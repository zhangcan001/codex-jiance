use std::sync::{Arc, Weak};

use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
    account::{unix_timestamp, AccountService, AccountStatus, CodexAccountInfo},
    codex::app_server::{AppServerManager, JsonRpcClient, SchemaCompatibilityService},
    error::AppError,
    models::codex::{SchemaCompatibilityReport, SchemaCompatibilityStatus},
};

use super::{
    model::{RateLimitInfo, RateLimitStatus, RateLimitWindow, RateLimitWindowKind},
    repository::RateLimitRepository,
    wire::{RateLimitBucketWire, RateLimitReadResponse, RateLimitWindowWire},
};

pub(crate) const RATE_LIMIT_READ_METHOD: &str = "account/rateLimits/read";
const RATE_LIMIT_UPDATED_METHOD: &str = "account/rateLimits/updated";
const RATE_LIMIT_RESPONSE_PARSE_ERROR: &str = "Rate limit response could not be parsed.";
const RATE_LIMIT_CACHE_TTL_SECONDS: i64 = 60;

#[derive(Debug, Clone)]
struct RateLimitCache {
    current: Option<RateLimitInfo>,
    stale: bool,
    fetched_at: Option<i64>,
}

struct RateLimitWatcher {
    client: Weak<JsonRpcClient>,
    task: tokio::task::JoinHandle<()>,
}

pub(crate) struct RateLimitService {
    app_server_manager: Arc<AppServerManager>,
    compatibility_service: Arc<SchemaCompatibilityService>,
    account_service: Arc<AccountService>,
    repository: Arc<RateLimitRepository>,
    cache: Arc<RwLock<RateLimitCache>>,
    watcher: Mutex<Option<RateLimitWatcher>>,
}

impl RateLimitService {
    pub(crate) fn new(
        app_server_manager: Arc<AppServerManager>,
        compatibility_service: Arc<SchemaCompatibilityService>,
        account_service: Arc<AccountService>,
        repository: Arc<RateLimitRepository>,
    ) -> Self {
        Self {
            app_server_manager,
            compatibility_service,
            account_service,
            repository,
            cache: Arc::new(RwLock::new(RateLimitCache {
                current: None,
                stale: true,
                fetched_at: None,
            })),
            watcher: Mutex::new(None),
        }
    }

    pub(crate) async fn get_rate_limits(&self, force: bool) -> RateLimitInfo {
        let compatibility = self.compatibility_service.check(false).await;
        if !supports_rate_limit_read(&compatibility) {
            self.mark_stale().await;
            return if compatibility.status == SchemaCompatibilityStatus::Error {
                error_info(
                    compatibility
                        .message
                        .as_deref()
                        .unwrap_or("Rate limit compatibility verification failed."),
                )
            } else {
                unavailable_info("Installed Codex schema does not expose account/rateLimits/read.")
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
                    "Start and initialize the Codex App Server to read rate limits.",
                )
            }
        };

        self.ensure_watcher(&client).await;

        if !force {
            let now = unix_timestamp();
            let cache = self.cache.read().await;
            if let (Some(current), Some(fetched_at)) = (&cache.current, cache.fetched_at) {
                if !cache.stale && now.saturating_sub(fetched_at) <= RATE_LIMIT_CACHE_TTL_SECONDS {
                    return current.clone();
                }
            }
        }

        match Self::read_rate_limits_with_client(&client).await {
            Ok(rate_limits) => {
                self.set_cache(rate_limits.clone()).await;
                self.persist_snapshot(&rate_limits).await;
                log::info!("Rate limit cache refreshed");
                rate_limits
            }
            Err(error) => {
                self.mark_stale().await;
                error_info(&format!("Could not read Codex rate limits: {error}"))
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        let watcher = self.watcher.lock().await.take();
        if let Some(watcher) = watcher {
            watcher.task.abort();
            let _ = watcher.task.await;
        }

        let mut cache = self.cache.write().await;
        cache.current = None;
        cache.stale = true;
        cache.fetched_at = None;
    }

    async fn ensure_watcher(&self, client: &Arc<JsonRpcClient>) {
        let mut watcher = self.watcher.lock().await;
        if watcher
            .as_ref()
            .is_some_and(|watcher| watcher_is_current(watcher, client))
        {
            return;
        }

        if let Some(previous) = watcher.take() {
            previous.task.abort();
            let _ = previous.task.await;
            self.clear_cache().await;
        } else {
            self.mark_stale().await;
        }

        let receiver = client.subscribe_notifications();
        let client_weak = Arc::downgrade(client);
        let task = tokio::spawn(watch_notifications(
            Arc::clone(&self.cache),
            Weak::clone(&client_weak),
            Some(Arc::clone(&self.repository)),
            receiver,
        ));
        *watcher = Some(RateLimitWatcher {
            client: client_weak,
            task,
        });
        log::info!("Rate limit notification watcher attached");
    }

    pub(crate) async fn read_rate_limits_with_client(
        client: &JsonRpcClient,
    ) -> Result<RateLimitInfo, AppError> {
        let response = client.request_no_params(RATE_LIMIT_READ_METHOD).await?;
        let response: RateLimitReadResponse = serde_json::from_value(response)
            .map_err(|_| AppError::RpcProtocol(RATE_LIMIT_RESPONSE_PARSE_ERROR.to_owned()))?;

        Ok(normalize_rate_limits(response))
    }

    async fn set_cache(&self, rate_limits: RateLimitInfo) {
        let mut cache = self.cache.write().await;
        cache.current = Some(rate_limits);
        cache.stale = false;
        cache.fetched_at = Some(unix_timestamp());
    }

    async fn persist_snapshot(&self, rate_limits: &RateLimitInfo) {
        if let Err(error) = self
            .repository
            .persist_snapshot_if_changed(rate_limits)
            .await
        {
            log::warn!("Rate limit snapshot persistence failed: {error}");
        }
    }

    async fn mark_stale(&self) {
        self.cache.write().await.stale = true;
    }

    async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.current = None;
        cache.stale = true;
        cache.fetched_at = None;
    }
}

async fn watch_notifications(
    cache: Arc<RwLock<RateLimitCache>>,
    client: Weak<JsonRpcClient>,
    repository: Option<Arc<RateLimitRepository>>,
    mut receiver: broadcast::Receiver<crate::codex::app_server::json_rpc::RpcNotification>,
) {
    loop {
        match receiver.recv().await {
            Ok(notification) if notification.method == RATE_LIMIT_UPDATED_METHOD => {
                log::info!("account/rateLimits/updated received");
                mark_cache_stale(&cache).await;
                if !refresh_from_notification(&cache, &client, repository.as_deref()).await {
                    log::warn!("Rate limit cache refresh after notification failed");
                }
            }
            Ok(_) => {}
            Err(broadcast::error::RecvError::Lagged(_)) => {
                mark_cache_stale(&cache).await;
                if !refresh_from_notification(&cache, &client, repository.as_deref()).await {
                    log::warn!("Rate limit cache refresh after notification lag failed");
                }
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("Rate limit notification watcher closed");
                return;
            }
        }
    }
}

async fn refresh_from_notification(
    cache: &Arc<RwLock<RateLimitCache>>,
    client: &Weak<JsonRpcClient>,
    repository: Option<&RateLimitRepository>,
) -> bool {
    let Some(client) = client.upgrade() else {
        return false;
    };

    match RateLimitService::read_rate_limits_with_client(&client).await {
        Ok(rate_limits) => {
            {
                let mut cache = cache.write().await;
                cache.current = Some(rate_limits.clone());
                cache.stale = false;
                cache.fetched_at = Some(unix_timestamp());
            }
            if let Some(repository) = repository {
                if let Err(error) = repository.persist_snapshot_if_changed(&rate_limits).await {
                    log::warn!("Rate limit snapshot persistence failed: {error}");
                }
            }
            log::info!("Rate limit cache refreshed");
            true
        }
        Err(_) => {
            mark_cache_stale(cache).await;
            false
        }
    }
}

async fn mark_cache_stale(cache: &RwLock<RateLimitCache>) {
    cache.write().await.stale = true;
}

fn watcher_is_current(watcher: &RateLimitWatcher, client: &Arc<JsonRpcClient>) -> bool {
    !watcher.task.is_finished() && same_client(&watcher.client, client)
}

fn same_client(existing: &Weak<JsonRpcClient>, current: &Arc<JsonRpcClient>) -> bool {
    existing
        .upgrade()
        .is_some_and(|existing| Arc::ptr_eq(&existing, current))
}

fn account_gate(account: &CodexAccountInfo) -> Option<RateLimitInfo> {
    if account.status != AccountStatus::Connected {
        return Some(unavailable_info(
            account
                .message
                .as_deref()
                .unwrap_or("Codex account is unavailable for rate limits."),
        ));
    }

    match account.account_type.as_deref() {
        Some("apiKey") => Some(unavailable_info(
            "Rate limits are unavailable for API key accounts.",
        )),
        Some("amazonBedrock") => Some(unavailable_info(
            "Rate limits are unavailable for Amazon Bedrock accounts.",
        )),
        _ => None,
    }
}

fn supports_rate_limit_read(report: &SchemaCompatibilityReport) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.key == RATE_LIMIT_READ_METHOD && check.present)
}

fn normalize_rate_limits(response: RateLimitReadResponse) -> RateLimitInfo {
    let mut windows = Vec::new();
    if let Some(buckets) = response
        .rate_limits_by_limit_id
        .as_ref()
        .filter(|buckets| !buckets.is_empty())
    {
        let mut entries = buckets.iter().collect::<Vec<_>>();
        entries.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (limit_id, bucket) in entries {
            append_bucket_windows(&mut windows, Some(limit_id.as_str()), bucket);
        }
    } else if let Some(bucket) = response.rate_limits.as_ref() {
        append_bucket_windows(&mut windows, None, bucket);
    }

    RateLimitInfo {
        status: RateLimitStatus::Available,
        windows,
        reset_credits_available: response
            .rate_limit_reset_credits
            .and_then(|credits| credits.available_count),
        updated_at: unix_timestamp(),
        message: None,
    }
}

fn append_bucket_windows(
    windows: &mut Vec<RateLimitWindow>,
    map_limit_id: Option<&str>,
    bucket: &RateLimitBucketWire,
) {
    let limit_id = bucket
        .limit_id
        .clone()
        .or_else(|| map_limit_id.map(str::to_owned));
    if let Some(window) = bucket.primary.as_ref() {
        append_window(
            windows,
            bucket,
            limit_id.clone(),
            RateLimitWindowKind::Primary,
            window,
        );
    }
    if let Some(window) = bucket.secondary.as_ref() {
        append_window(
            windows,
            bucket,
            limit_id,
            RateLimitWindowKind::Secondary,
            window,
        );
    }
}

fn append_window(
    windows: &mut Vec<RateLimitWindow>,
    bucket: &RateLimitBucketWire,
    limit_id: Option<String>,
    window_kind: RateLimitWindowKind,
    window: &RateLimitWindowWire,
) {
    let Some(raw_used_percent) = window.used_percent else {
        return;
    };
    if !raw_used_percent.is_finite() {
        return;
    }

    let used_percent = raw_used_percent.clamp(0.0, 100.0);
    let remaining_percent = (100.0 - used_percent).clamp(0.0, 100.0);
    windows.push(RateLimitWindow {
        limit_id,
        limit_name: bucket.limit_name.clone(),
        window_kind,
        used_percent,
        remaining_percent,
        window_duration_mins: window.window_duration_mins,
        resets_at: window.resets_at,
        plan_type: bucket.plan_type.clone(),
        rate_limit_reached_type: bucket.rate_limit_reached_type.clone(),
    });
}

fn unavailable_info(message: &str) -> RateLimitInfo {
    RateLimitInfo {
        status: RateLimitStatus::Unavailable,
        windows: Vec::new(),
        reset_credits_available: None,
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

fn error_info(message: &str) -> RateLimitInfo {
    RateLimitInfo {
        status: RateLimitStatus::Error,
        windows: Vec::new(),
        reset_credits_available: None,
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Weak};

    use serde_json::{json, Value};
    use tokio::{
        io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader},
        sync::broadcast,
        time::{timeout, Duration},
    };

    use super::{account_gate, normalize_rate_limits, RateLimitService, RATE_LIMIT_READ_METHOD};
    use crate::rate_limit::model::{RateLimitStatus, RateLimitWindowKind};
    use crate::rate_limit::wire::{
        RateLimitBucketWire, RateLimitReadResponse, RateLimitResetCreditsWire, RateLimitWindowWire,
    };
    use crate::{
        account::{AccountStatus, CodexAccountInfo},
        codex::app_server::JsonRpcClient,
        models::codex::SchemaCompatibilityStatus,
    };

    fn bucket(
        limit_id: Option<&str>,
        primary: Option<RateLimitWindowWire>,
        secondary: Option<RateLimitWindowWire>,
    ) -> RateLimitBucketWire {
        RateLimitBucketWire {
            limit_id: limit_id.map(str::to_owned),
            limit_name: Some("ChatGPT".to_owned()),
            primary,
            secondary,
            plan_type: Some("plus".to_owned()),
            rate_limit_reached_type: None,
        }
    }

    fn window(used_percent: f64, duration: Option<i64>) -> RateLimitWindowWire {
        RateLimitWindowWire {
            used_percent: Some(used_percent),
            window_duration_mins: duration,
            resets_at: Some(1_700_000_000),
        }
    }

    fn chatgpt_account() -> CodexAccountInfo {
        CodexAccountInfo {
            status: AccountStatus::Connected,
            account_type: Some("chatgpt".to_owned()),
            email_masked: None,
            plan_type: Some("plus".to_owned()),
            credential_source: None,
            requires_openai_auth: Some(true),
            auth_mode: None,
            updated_at: 0,
            message: None,
        }
    }

    #[test]
    fn normalizes_single_bucket_primary_secondary_and_clamps_derived_remaining() {
        let info = normalize_rate_limits(RateLimitReadResponse {
            rate_limits: Some(bucket(
                Some("chatgpt"),
                Some(window(12.5, Some(300))),
                Some(window(120.0, Some(10080))),
            )),
            rate_limits_by_limit_id: None,
            rate_limit_reset_credits: Some(RateLimitResetCreditsWire {
                available_count: Some(2),
            }),
        });

        assert_eq!(info.status, RateLimitStatus::Available);
        assert_eq!(info.windows.len(), 2);
        assert_eq!(info.windows[0].window_kind, RateLimitWindowKind::Primary);
        assert_eq!(info.windows[0].used_percent, 12.5);
        assert_eq!(info.windows[0].remaining_percent, 87.5);
        assert_eq!(info.windows[1].used_percent, 100.0);
        assert_eq!(info.windows[1].remaining_percent, 0.0);
        assert_eq!(info.reset_credits_available, Some(2));
    }

    #[test]
    fn rate_limits_by_limit_id_is_preferred_over_fallback_bucket() {
        let mut by_limit_id = std::collections::HashMap::new();
        by_limit_id.insert(
            "multi".to_owned(),
            bucket(Some("multi"), Some(window(33.0, Some(300))), None),
        );
        let info = normalize_rate_limits(RateLimitReadResponse {
            rate_limits: Some(bucket(
                Some("fallback"),
                Some(window(99.0, Some(10080))),
                None,
            )),
            rate_limits_by_limit_id: Some(by_limit_id),
            rate_limit_reset_credits: None,
        });

        assert_eq!(info.windows.len(), 1);
        assert_eq!(info.windows[0].limit_id.as_deref(), Some("multi"));
        assert_eq!(info.windows[0].used_percent, 33.0);
    }

    #[test]
    fn account_gate_allows_chatgpt_and_rejects_api_key_and_bedrock() {
        assert!(account_gate(&chatgpt_account()).is_none());

        for account_type in ["apiKey", "amazonBedrock"] {
            let mut account = chatgpt_account();
            account.account_type = Some(account_type.to_owned());
            let unavailable = account_gate(&account).expect("account should be gated");
            assert_eq!(unavailable.status, RateLimitStatus::Unavailable);
        }
    }

    #[test]
    fn compatibility_status_does_not_replace_specific_method_check() {
        let report = crate::models::codex::SchemaCompatibilityReport {
            status: SchemaCompatibilityStatus::Compatible,
            codex_version: None,
            checked_at: 0,
            schema_generated: true,
            stable_surface: true,
            schema_file_count: 1,
            schema_total_bytes: 1,
            required_passed: 13,
            required_total: 13,
            optional_passed: 9,
            optional_total: 9,
            core_monitoring_compatible: true,
            advanced_thread_usage_supported: true,
            checks: Vec::new(),
            warnings: Vec::new(),
            message: None,
        };
        assert!(!super::supports_rate_limit_read(&report));
    }

    #[tokio::test]
    async fn rate_limit_read_omits_params_and_jsonrpc() {
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
                .expect("request should arrive");
            let request: Value = serde_json::from_str(&line).expect("request should be JSON");
            assert_eq!(request["method"], RATE_LIMIT_READ_METHOD);
            assert!(request.get("params").is_none());
            assert!(request.get("jsonrpc").is_none());
            let response = json!({
                "id": request["id"],
                "result": {
                    "rateLimits": {
                        "limitId": "chatgpt",
                        "primary": {"usedPercent": 37, "windowDurationMins": 300, "resetsAt": 1700000000}
                    },
                    "futureField": true
                }
            });
            let mut bytes = serde_json::to_vec(&response).expect("response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("response should send");
        });

        let info = RateLimitService::read_rate_limits_with_client(&client)
            .await
            .expect("rate limit response should parse");
        assert_eq!(info.windows.len(), 1);
        assert_eq!(info.windows[0].window_duration_mins, Some(300));
        server.await.expect("server should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn rate_limit_updated_triggers_one_full_read_and_ignores_partial_payload() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let cache = Arc::new(tokio::sync::RwLock::new(super::RateLimitCache {
            current: None,
            stale: true,
            fetched_at: None,
        }));
        let receiver = client.subscribe_notifications();
        let watcher = tokio::spawn(super::watch_notifications(
            Arc::clone(&cache),
            Arc::downgrade(&client),
            None,
            receiver,
        ));

        server_writer
            .write_all(b"{\"method\":\"account/updated\",\"params\":{\"ignored\":true}}\n")
            .await
            .expect("unrelated notification should send");
        server_writer
            .write_all(
                b"{\"method\":\"account/rateLimits/updated\",\"params\":{\"rateLimits\":{\"primary\":{\"usedPercent\":99}}}}\n",
            )
            .await
            .expect("rate limit notification should send");

        let mut reader = BufReader::new(server_reader);
        let mut line = String::new();
        timeout(Duration::from_secs(1), reader.read_line(&mut line))
            .await
            .expect("full refresh request should arrive")
            .expect("request should be readable");
        let request: Value = serde_json::from_str(&line).expect("request should be JSON");
        assert_eq!(request["method"], RATE_LIMIT_READ_METHOD);
        assert!(request.get("params").is_none());
        let response = json!({
            "id": request["id"],
            "result": {
                "rateLimits": {
                    "limitId": "chatgpt",
                    "primary": {"usedPercent": 37, "windowDurationMins": 300}
                }
            }
        });
        let mut response_bytes = serde_json::to_vec(&response).expect("response should serialize");
        response_bytes.push(b'\n');
        server_writer
            .write_all(&response_bytes)
            .await
            .expect("response should send");

        timeout(Duration::from_secs(1), async {
            loop {
                if !cache.read().await.stale {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("cache should refresh");
        let cached = cache.read().await.current.clone().expect("cached snapshot");
        assert_eq!(cached.windows[0].used_percent, 37.0);

        watcher.abort();
        let _ = watcher.await;
        client.shutdown().await;
    }

    #[tokio::test]
    async fn lagged_notification_runs_one_refresh_and_closed_watcher_exits() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let cache = Arc::new(tokio::sync::RwLock::new(super::RateLimitCache {
            current: None,
            stale: true,
            fetched_at: None,
        }));
        let (sender, receiver) = broadcast::channel(1);
        sender
            .send(crate::codex::app_server::json_rpc::RpcNotification {
                method: "ignored/one".to_owned(),
                params: Value::Null,
            })
            .expect("first notification should send");
        sender
            .send(crate::codex::app_server::json_rpc::RpcNotification {
                method: "ignored/two".to_owned(),
                params: Value::Null,
            })
            .expect("second notification should send");
        let watcher = tokio::spawn(super::watch_notifications(
            Arc::clone(&cache),
            Arc::downgrade(&client),
            None,
            receiver,
        ));

        let mut reader = BufReader::new(server_reader);
        let mut line = String::new();
        timeout(Duration::from_secs(1), reader.read_line(&mut line))
            .await
            .expect("lagged refresh request should arrive")
            .expect("request should be readable");
        let request: Value = serde_json::from_str(&line).expect("request should be JSON");
        assert_eq!(request["method"], RATE_LIMIT_READ_METHOD);
        assert!(request.get("params").is_none());
        let response = json!({
            "id": request["id"],
            "result": {"rateLimits": {"primary": {"usedPercent": 1}}}
        });
        let mut response_bytes = serde_json::to_vec(&response).expect("response should serialize");
        response_bytes.push(b'\n');
        server_writer
            .write_all(&response_bytes)
            .await
            .expect("response should send");
        timeout(Duration::from_secs(1), async {
            loop {
                if !cache.read().await.current.is_some() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    continue;
                }
                break;
            }
        })
        .await
        .expect("lagged cache should refresh");

        drop(sender);
        watcher.abort();
        let _ = watcher.await;
        client.shutdown().await;

        let (closed_sender, closed_receiver) = broadcast::channel(1);
        drop(closed_sender);
        let closed_watcher = tokio::spawn(super::watch_notifications(
            Arc::clone(&cache),
            Weak::new(),
            None,
            closed_receiver,
        ));
        timeout(Duration::from_secs(1), closed_watcher)
            .await
            .expect("closed watcher should exit")
            .expect("closed watcher should join");
    }

    #[tokio::test]
    async fn same_client_identity_is_reused_but_new_client_is_not() {
        let (first_io, _) = tokio::io::duplex(64);
        let (first_reader, first_writer) = split(first_io);
        let first = JsonRpcClient::from_io(first_reader, first_writer).await;
        let first_weak = Arc::downgrade(&first);
        assert!(super::same_client(&first_weak, &first));

        let (second_io, _) = tokio::io::duplex(64);
        let (second_reader, second_writer) = split(second_io);
        let second = JsonRpcClient::from_io(second_reader, second_writer).await;
        assert!(!super::same_client(&first_weak, &second));

        first.shutdown().await;
        second.shutdown().await;
    }

    #[test]
    fn unknown_fields_and_decimal_used_percent_are_tolerated() {
        let response: RateLimitReadResponse = serde_json::from_value(json!({
            "rateLimits": {
                "primary": {"usedPercent": 41.25, "windowDurationMins": 15, "futureWindow": true}
            },
            "rateLimitResetCredits": {"availableCount": 4, "future": "ignored"},
            "future": "ignored"
        }))
        .expect("future fields should be ignored");
        let info = normalize_rate_limits(response);
        assert_eq!(info.windows[0].used_percent, 41.25);
        assert_eq!(info.reset_credits_available, Some(4));
    }
}
