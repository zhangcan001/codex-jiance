use std::sync::Arc;

use crate::{
    account::{unix_timestamp, AccountService, AccountStatus, CodexAccountInfo},
    codex::app_server::{AppServerManager, JsonRpcClient, SchemaCompatibilityService},
    error::AppError,
    models::codex::{SchemaCompatibilityReport, SchemaCompatibilityStatus},
};

use super::{
    model::{CodexUsageInfo, DailyUsageBucket, UsageStatus, UsageSummary},
    wire::{UsageReadResponse, UsageSummaryWire},
};

pub(crate) const USAGE_READ_METHOD: &str = "account/usage/read";
const USAGE_RESPONSE_PARSE_ERROR: &str = "Usage response could not be parsed.";
const USAGE_CACHE_TTL_SECONDS: i64 = 300;

#[derive(Debug, Clone)]
struct UsageCache {
    current: Option<CodexUsageInfo>,
    stale: bool,
    fetched_at: Option<i64>,
}

pub(crate) struct UsageService {
    app_server_manager: Arc<AppServerManager>,
    compatibility_service: Arc<SchemaCompatibilityService>,
    account_service: Arc<AccountService>,
    cache: tokio::sync::RwLock<UsageCache>,
}

impl UsageService {
    pub(crate) fn new(
        app_server_manager: Arc<AppServerManager>,
        compatibility_service: Arc<SchemaCompatibilityService>,
        account_service: Arc<AccountService>,
    ) -> Self {
        Self {
            app_server_manager,
            compatibility_service,
            account_service,
            cache: tokio::sync::RwLock::new(UsageCache {
                current: None,
                stale: true,
                fetched_at: None,
            }),
        }
    }

    pub(crate) async fn get_usage(&self, force: bool) -> CodexUsageInfo {
        let compatibility = self.compatibility_service.check(false).await;
        if !supports_usage_read(&compatibility) {
            self.mark_stale().await;
            return if compatibility.status == SchemaCompatibilityStatus::Error {
                error_info(
                    compatibility
                        .message
                        .as_deref()
                        .unwrap_or("Usage compatibility verification failed."),
                )
            } else {
                unavailable_info("Installed Codex schema does not expose account/usage/read.")
            };
        }

        let account = self.account_service.get_account(false).await;
        if let Some(gated) = account_gate(&account) {
            return gated;
        }

        let client = match self.app_server_manager.initialized_client().await {
            Ok(client) => client,
            Err(_) => {
                return unavailable_info(
                    "Start and initialize the Codex App Server to read usage information.",
                )
            }
        };

        let now = unix_timestamp();
        let cached = {
            let cache = self.cache.read().await;
            cache
                .current
                .clone()
                .filter(|_| cache_is_usable(force, cache.stale, cache.fetched_at, now))
        };
        if let Some(current) = cached {
            return current;
        }

        match Self::read_usage_with_client(&client).await {
            Ok(usage) => {
                self.set_cache(usage.clone()).await;
                log::info!("Usage cache refreshed");
                usage
            }
            Err(error) => {
                self.mark_stale().await;
                error_info(&format!("Could not read Codex usage: {error}"))
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        let mut cache = self.cache.write().await;
        cache.current = None;
        cache.stale = true;
        cache.fetched_at = None;
    }

    pub(crate) async fn read_usage_with_client(
        client: &JsonRpcClient,
    ) -> Result<CodexUsageInfo, AppError> {
        let response = client.request_no_params(USAGE_READ_METHOD).await?;
        let response: UsageReadResponse = serde_json::from_value(response)
            .map_err(|_| AppError::RpcProtocol(USAGE_RESPONSE_PARSE_ERROR.to_owned()))?;

        Ok(normalize_usage(response))
    }

    async fn set_cache(&self, usage: CodexUsageInfo) {
        let mut cache = self.cache.write().await;
        cache.current = Some(usage);
        cache.stale = false;
        cache.fetched_at = Some(unix_timestamp());
    }

    async fn mark_stale(&self) {
        self.cache.write().await.stale = true;
    }
}

fn normalize_usage(response: UsageReadResponse) -> CodexUsageInfo {
    CodexUsageInfo {
        status: UsageStatus::Available,
        summary: response.summary.map(normalize_summary),
        daily_buckets: response
            .daily_usage_buckets
            .unwrap_or_default()
            .into_iter()
            .map(|bucket| DailyUsageBucket {
                start_date: bucket.start_date,
                tokens: bucket.tokens,
            })
            .collect(),
        updated_at: unix_timestamp(),
        message: None,
    }
}

fn normalize_summary(summary: UsageSummaryWire) -> UsageSummary {
    UsageSummary {
        lifetime_tokens: summary.lifetime_tokens,
        peak_daily_tokens: summary.peak_daily_tokens,
        longest_running_turn_sec: summary.longest_running_turn_sec,
        current_streak_days: summary.current_streak_days,
        longest_streak_days: summary.longest_streak_days,
    }
}

fn account_gate(account: &CodexAccountInfo) -> Option<CodexUsageInfo> {
    if account.status != AccountStatus::Connected {
        return Some(unavailable_info(
            account
                .message
                .as_deref()
                .unwrap_or("Codex account is unavailable for usage."),
        ));
    }

    match account.account_type.as_deref() {
        Some("apiKey") => Some(unavailable_info(
            "Usage is unavailable for API key accounts.",
        )),
        Some("amazonBedrock") => Some(unavailable_info(
            "Usage is unavailable for Amazon Bedrock accounts.",
        )),
        _ => None,
    }
}

fn supports_usage_read(report: &SchemaCompatibilityReport) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.key == USAGE_READ_METHOD && check.present)
}

fn cache_is_fresh(fetched_at: i64, now: i64) -> bool {
    now.saturating_sub(fetched_at) <= USAGE_CACHE_TTL_SECONDS
}

fn cache_is_usable(force: bool, stale: bool, fetched_at: Option<i64>, now: i64) -> bool {
    !force && !stale && fetched_at.is_some_and(|fetched_at| cache_is_fresh(fetched_at, now))
}

fn unavailable_info(message: &str) -> CodexUsageInfo {
    CodexUsageInfo {
        status: UsageStatus::Unavailable,
        summary: None,
        daily_buckets: Vec::new(),
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

fn error_info(message: &str) -> CodexUsageInfo {
    CodexUsageInfo {
        status: UsageStatus::Error,
        summary: None,
        daily_buckets: Vec::new(),
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use tokio::io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader};

    use super::{
        account_gate, cache_is_fresh, cache_is_usable, normalize_usage, supports_usage_read,
        UsageService, USAGE_READ_METHOD,
    };
    use crate::account::{AccountStatus, CodexAccountInfo};
    use crate::codex::app_server::JsonRpcClient;
    use crate::models::codex::{
        CompatibilityCheck, CompatibilityCheckCategory, SchemaCompatibilityReport,
        SchemaCompatibilityStatus,
    };
    use crate::usage::model::{UsageStatus, UsageSummary};
    use crate::usage::wire::UsageReadResponse;

    fn connected_account(account_type: Option<&str>) -> CodexAccountInfo {
        CodexAccountInfo {
            status: AccountStatus::Connected,
            account_type: account_type.map(str::to_owned),
            email_masked: Some("us***@example.com".to_owned()),
            plan_type: Some("plus".to_owned()),
            credential_source: Some("appServer".to_owned()),
            requires_openai_auth: Some(true),
            auth_mode: Some("chatgpt".to_owned()),
            updated_at: 1_700_000_000,
            message: None,
        }
    }

    fn report_with_usage(
        present: bool,
        status: SchemaCompatibilityStatus,
    ) -> SchemaCompatibilityReport {
        SchemaCompatibilityReport {
            status,
            codex_version: None,
            checked_at: 0,
            schema_generated: true,
            stable_surface: true,
            schema_file_count: 1,
            schema_total_bytes: 1,
            required_passed: if present { 1 } else { 0 },
            required_total: 1,
            optional_passed: 0,
            optional_total: 0,
            core_monitoring_compatible: present,
            advanced_thread_usage_supported: false,
            checks: vec![CompatibilityCheck {
                key: USAGE_READ_METHOD.to_owned(),
                category: CompatibilityCheckCategory::Method,
                required: true,
                present,
            }],
            warnings: Vec::new(),
            message: None,
        }
    }

    #[test]
    fn normalizes_complete_summary_and_multiple_buckets() {
        let response: UsageReadResponse = serde_json::from_value(json!({
            "summary": {
                "lifetimeTokens": 1000,
                "peakDailyTokens": 400,
                "longestRunningTurnSec": 90,
                "currentStreakDays": 3,
                "longestStreakDays": 8
            },
            "dailyUsageBuckets": [
                {"startDate": "2026-08-16", "tokens": 123},
                {"startDate": "2026-08-17", "tokens": 456}
            ],
            "futureField": true
        }))
        .expect("complete usage response should parse");
        let usage = normalize_usage(response);
        assert_eq!(usage.status, UsageStatus::Available);
        assert_eq!(
            usage.summary,
            Some(UsageSummary {
                lifetime_tokens: Some(1000),
                peak_daily_tokens: Some(400),
                longest_running_turn_sec: Some(90),
                current_streak_days: Some(3),
                longest_streak_days: Some(8),
            })
        );
        assert_eq!(usage.daily_buckets.len(), 2);
    }

    #[test]
    fn tolerates_partial_null_and_missing_usage_fields() {
        let partial: UsageReadResponse = serde_json::from_value(json!({
            "summary": {
                "lifetimeTokens": null,
                "peakDailyTokens": 400,
                "longestRunningTurnSec": null,
                "currentStreakDays": 3,
                "longestStreakDays": null
            },
            "dailyUsageBuckets": null
        }))
        .expect("partial usage response should parse");
        let usage = normalize_usage(partial);
        assert_eq!(usage.summary.as_ref().and_then(|s| s.lifetime_tokens), None);
        assert_eq!(
            usage.summary.as_ref().and_then(|s| s.peak_daily_tokens),
            Some(400)
        );
        assert!(usage.daily_buckets.is_empty());

        let no_summary: UsageReadResponse = serde_json::from_value(json!({
            "summary": null,
            "dailyUsageBuckets": null
        }))
        .expect("null summary should parse");
        assert!(normalize_usage(no_summary).summary.is_none());
    }

    #[test]
    fn compatibility_requires_the_specific_method_and_allows_limited_status() {
        assert!(supports_usage_read(&report_with_usage(
            true,
            SchemaCompatibilityStatus::Limited
        )));
        assert!(!supports_usage_read(&report_with_usage(
            false,
            SchemaCompatibilityStatus::Compatible
        )));
    }

    #[test]
    fn account_gate_rejects_api_key_and_bedrock_but_allows_unknown_provider() {
        assert!(account_gate(&connected_account(None)).is_none());
        assert!(account_gate(&connected_account(Some("futureProvider"))).is_none());
        for account_type in ["apiKey", "amazonBedrock"] {
            let unavailable = account_gate(&connected_account(Some(account_type)))
                .expect("account should be unavailable");
            assert_eq!(unavailable.status, UsageStatus::Unavailable);
        }
    }

    #[test]
    fn cache_ttl_is_five_minutes() {
        assert!(cache_is_fresh(1_000, 1_300));
        assert!(!cache_is_fresh(1_000, 1_301));
        assert!(cache_is_usable(false, false, Some(1_000), 1_300));
        assert!(!cache_is_usable(true, false, Some(1_000), 1_300));
    }

    #[tokio::test]
    async fn usage_read_omits_params_and_jsonrpc() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("usage request should arrive");
            let request: Value = serde_json::from_str(&line).expect("request should be JSON");
            assert_eq!(request["method"], USAGE_READ_METHOD);
            assert!(request.get("params").is_none());
            assert!(request.get("jsonrpc").is_none());
            let response = json!({
                "id": request["id"],
                "result": {"summary": null, "dailyUsageBuckets": null}
            });
            let mut bytes = serde_json::to_vec(&response).expect("response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("response should send");
        });

        let usage = UsageService::read_usage_with_client(&client)
            .await
            .expect("usage response should parse");
        assert_eq!(usage.status, UsageStatus::Available);
        server.await.expect("server should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn parse_failure_and_remote_error_are_returned_safely() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let server = tokio::spawn(async move {
            let mut reader = BufReader::new(server_reader);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .await
                .expect("first request should arrive");
            let request: Value = serde_json::from_str(&line).expect("request should be JSON");
            let mut bytes = serde_json::to_vec(
                &json!({"id": request["id"], "result": {"summary": {"lifetimeTokens": "bad"}}}),
            )
            .expect("response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("parse response should send");
            line.clear();
            reader
                .read_line(&mut line)
                .await
                .expect("second request should arrive");
            let request: Value = serde_json::from_str(&line).expect("request should be JSON");
            let mut bytes = serde_json::to_vec(
                &json!({"id": request["id"], "error": {"code": -1, "message": "denied"}}),
            )
            .expect("error response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("error response should send");
        });

        assert!(UsageService::read_usage_with_client(&client).await.is_err());
        assert!(UsageService::read_usage_with_client(&client).await.is_err());
        server.await.expect("server should finish");
        client.shutdown().await;
    }
}
