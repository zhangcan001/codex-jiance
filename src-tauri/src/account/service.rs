use std::sync::{Arc, Weak};

use serde_json::json;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
    codex::app_server::{AppServerManager, JsonRpcClient, SchemaCompatibilityService},
    error::AppError,
    models::codex::{SchemaCompatibilityReport, SchemaCompatibilityStatus},
};

use super::{
    model::{mask_email, unix_timestamp, AccountStatus, CodexAccountInfo},
    wire::{AccountReadResponse, AccountUpdatedParams},
};

const ACCOUNT_READ_METHOD: &str = "account/read";
const ACCOUNT_UPDATED_METHOD: &str = "account/updated";
const ACCOUNT_RESPONSE_PARSE_ERROR: &str = "Account response could not be parsed.";

#[derive(Debug, Clone)]
struct AccountCache {
    current: Option<CodexAccountInfo>,
    stale: bool,
}

struct AccountWatcher {
    client: Weak<JsonRpcClient>,
    task: tokio::task::JoinHandle<()>,
}

pub(crate) struct AccountService {
    app_server_manager: Arc<AppServerManager>,
    compatibility_service: Arc<SchemaCompatibilityService>,
    cache: Arc<RwLock<AccountCache>>,
    watcher: Mutex<Option<AccountWatcher>>,
}

impl AccountService {
    pub(crate) fn new(
        app_server_manager: Arc<AppServerManager>,
        compatibility_service: Arc<SchemaCompatibilityService>,
    ) -> Self {
        Self {
            app_server_manager,
            compatibility_service,
            cache: Arc::new(RwLock::new(AccountCache {
                current: None,
                stale: true,
            })),
            watcher: Mutex::new(None),
        }
    }

    pub(crate) async fn get_account(&self, force: bool) -> CodexAccountInfo {
        let client = match self.app_server_manager.initialized_client().await {
            Ok(client) => client,
            Err(_) => {
                return unavailable_info(
                    "Start and initialize the Codex App Server to read account information.",
                )
            }
        };

        let compatibility = self.compatibility_service.check(false).await;
        if !supports_account_read(&compatibility) {
            self.mark_stale().await;
            return if compatibility.status == SchemaCompatibilityStatus::Error {
                error_info(
                    compatibility
                        .message
                        .as_deref()
                        .unwrap_or("Account compatibility verification failed."),
                )
            } else {
                unavailable_info("Installed Codex schema does not expose account/read.")
            };
        }

        self.ensure_watcher(&client).await;

        if !force {
            let cache = self.cache.read().await;
            if let Some(current) = cache.current.as_ref().filter(|_| !cache.stale) {
                return current.clone();
            }
        }

        let auth_mode = self
            .cache
            .read()
            .await
            .current
            .as_ref()
            .and_then(|account| account.auth_mode.clone());
        match Self::read_account_with_client(&client, auth_mode).await {
            Ok(account) => {
                self.set_cache(account.clone()).await;
                log::info!("Account cache refreshed");
                account
            }
            Err(error) => {
                self.mark_stale().await;
                error_info(&account_read_error_message(error))
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        let watcher = self.watcher.lock().await.take();
        if let Some(watcher) = watcher {
            watcher.task.abort();
            let _ = watcher.task.await;
        }

        self.clear_cache().await;
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
            receiver,
        ));
        *watcher = Some(AccountWatcher {
            client: client_weak,
            task,
        });
        log::info!("Account notification watcher attached");
    }

    async fn set_cache(&self, account: CodexAccountInfo) {
        let mut cache = self.cache.write().await;
        cache.current = Some(account);
        cache.stale = false;
    }

    async fn mark_stale(&self) {
        mark_cache_stale(&self.cache).await;
    }

    async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.current = None;
        cache.stale = true;
    }

    pub(crate) async fn read_account_with_client(
        client: &JsonRpcClient,
        auth_mode: Option<String>,
    ) -> Result<CodexAccountInfo, AppError> {
        let response = client
            .request(ACCOUNT_READ_METHOD, json!({ "refreshToken": false }))
            .await?;
        let response: AccountReadResponse = serde_json::from_value(response)
            .map_err(|_| AppError::AccountService(ACCOUNT_RESPONSE_PARSE_ERROR.to_owned()))?;

        Ok(normalize_account(response, auth_mode))
    }
}

async fn watch_notifications(
    cache: Arc<RwLock<AccountCache>>,
    client: Weak<JsonRpcClient>,
    mut receiver: broadcast::Receiver<crate::codex::app_server::json_rpc::RpcNotification>,
) {
    loop {
        let notification = match receiver.recv().await {
            Ok(notification) => notification,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                mark_cache_stale(&cache).await;
                if !refresh_from_notification(&cache, &client, None).await {
                    log::warn!("Account cache refresh after notification lag failed");
                }
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => {
                log::info!("Account notification watcher closed");
                return;
            }
        };

        if notification.method != ACCOUNT_UPDATED_METHOD {
            continue;
        }

        log::info!("account/updated received");
        let auth_mode = serde_json::from_value::<AccountUpdatedParams>(notification.params)
            .ok()
            .and_then(|params| {
                let _notification_plan_type = params.plan_type;
                params.auth_mode
            });
        mark_cache_stale(&cache).await;
        if !refresh_from_notification(&cache, &client, auth_mode).await {
            log::warn!("Account cache refresh after account/updated failed");
        }
    }
}

async fn refresh_from_notification(
    cache: &Arc<RwLock<AccountCache>>,
    client: &Weak<JsonRpcClient>,
    auth_mode: Option<String>,
) -> bool {
    let Some(client) = client.upgrade() else {
        return false;
    };

    let auth_mode = if auth_mode.is_some() {
        auth_mode
    } else {
        cache
            .read()
            .await
            .current
            .as_ref()
            .and_then(|account| account.auth_mode.clone())
    };

    match AccountService::read_account_with_client(&client, auth_mode).await {
        Ok(account) => {
            let mut cache = cache.write().await;
            cache.current = Some(account);
            cache.stale = false;
            log::info!("Account cache refreshed");
            true
        }
        Err(_) => {
            mark_cache_stale(cache).await;
            false
        }
    }
}

async fn mark_cache_stale(cache: &RwLock<AccountCache>) {
    cache.write().await.stale = true;
}

fn watcher_is_current(watcher: &AccountWatcher, client: &Arc<JsonRpcClient>) -> bool {
    !watcher.task.is_finished() && same_client(&watcher.client, client)
}

fn same_client(existing: &Weak<JsonRpcClient>, current: &Arc<JsonRpcClient>) -> bool {
    existing
        .upgrade()
        .is_some_and(|existing| Arc::ptr_eq(&existing, current))
}

fn supports_account_read(report: &SchemaCompatibilityReport) -> bool {
    report
        .checks
        .iter()
        .any(|check| check.key == ACCOUNT_READ_METHOD && check.present)
}

fn normalize_account(response: AccountReadResponse, auth_mode: Option<String>) -> CodexAccountInfo {
    let updated_at = unix_timestamp();
    let Some(account) = response.account else {
        let message = if response.requires_openai_auth {
            "Codex does not currently have an OpenAI account session."
        } else {
            "No account is currently reported by Codex."
        };
        return CodexAccountInfo {
            status: AccountStatus::NoAccount,
            account_type: None,
            email_masked: None,
            plan_type: None,
            credential_source: None,
            requires_openai_auth: Some(response.requires_openai_auth),
            auth_mode,
            updated_at,
            message: Some(message.to_owned()),
        };
    };

    let credential_source = account.credential_source.or_else(|| {
        account.uses_codex_managed_credentials.map(|managed| {
            if managed {
                "codexManaged".to_owned()
            } else {
                "awsManaged".to_owned()
            }
        })
    });
    CodexAccountInfo {
        status: AccountStatus::Connected,
        account_type: Some(account.account_type),
        email_masked: account.email.as_deref().map(mask_email),
        plan_type: account.plan_type,
        credential_source,
        requires_openai_auth: Some(response.requires_openai_auth),
        auth_mode,
        updated_at,
        message: None,
    }
}

fn account_read_error_message(error: AppError) -> String {
    match error {
        AppError::AccountService(message) => message,
        error => format!("Could not read Codex account: {error}"),
    }
}

fn unavailable_info(message: &str) -> CodexAccountInfo {
    CodexAccountInfo {
        status: AccountStatus::Unavailable,
        account_type: None,
        email_masked: None,
        plan_type: None,
        credential_source: None,
        requires_openai_auth: None,
        auth_mode: None,
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

fn error_info(message: &str) -> CodexAccountInfo {
    CodexAccountInfo {
        status: AccountStatus::Error,
        account_type: None,
        email_masked: None,
        plan_type: None,
        credential_source: None,
        requires_openai_auth: None,
        auth_mode: None,
        updated_at: unix_timestamp(),
        message: Some(message.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{json, Value};
    use tokio::{
        io::{split, AsyncBufReadExt, AsyncWriteExt, BufReader},
        time::{timeout, Duration},
    };

    use super::{
        normalize_account, same_client, watch_notifications, AccountCache, ACCOUNT_READ_METHOD,
        ACCOUNT_RESPONSE_PARSE_ERROR,
    };
    use crate::{
        account::wire::{AccountReadResponse, AccountWire},
        codex::app_server::JsonRpcClient,
        error::AppError,
        models::codex::SchemaCompatibilityStatus,
    };

    fn response(account: Option<AccountWire>, requires_openai_auth: bool) -> AccountReadResponse {
        AccountReadResponse {
            account,
            requires_openai_auth,
        }
    }

    #[test]
    fn normalizes_chatgpt_and_masks_email() {
        let account = normalize_account(
            response(
                Some(AccountWire {
                    account_type: "chatgpt".to_owned(),
                    email: Some("user@example.com".to_owned()),
                    plan_type: Some("plus".to_owned()),
                    credential_source: None,
                    uses_codex_managed_credentials: None,
                }),
                true,
            ),
            Some("chatgpt".to_owned()),
        );

        assert_eq!(account.status, super::AccountStatus::Connected);
        assert_eq!(account.account_type.as_deref(), Some("chatgpt"));
        assert_eq!(account.email_masked.as_deref(), Some("us***@example.com"));
        assert_eq!(account.plan_type.as_deref(), Some("plus"));
        assert_eq!(account.requires_openai_auth, Some(true));
        assert_eq!(account.auth_mode.as_deref(), Some("chatgpt"));
    }

    #[test]
    fn normalizes_null_api_key_bedrock_and_future_accounts() {
        let no_account = normalize_account(response(None, true), None);
        assert_eq!(no_account.status, super::AccountStatus::NoAccount);
        assert_eq!(no_account.requires_openai_auth, Some(true));

        let api_key = normalize_account(
            response(
                Some(AccountWire {
                    account_type: "apiKey".to_owned(),
                    email: None,
                    plan_type: None,
                    credential_source: None,
                    uses_codex_managed_credentials: None,
                }),
                true,
            ),
            None,
        );
        assert_eq!(api_key.status, super::AccountStatus::Connected);
        assert_eq!(api_key.email_masked, None);
        assert_eq!(api_key.plan_type, None);

        for managed in [true, false] {
            let bedrock = normalize_account(
                response(
                    Some(AccountWire {
                        account_type: "amazonBedrock".to_owned(),
                        email: None,
                        plan_type: None,
                        credential_source: None,
                        uses_codex_managed_credentials: Some(managed),
                    }),
                    false,
                ),
                None,
            );
            assert_eq!(bedrock.status, super::AccountStatus::Connected);
            assert_eq!(
                bedrock.credential_source.as_deref(),
                Some(if managed {
                    "codexManaged"
                } else {
                    "awsManaged"
                })
            );
        }

        let future = normalize_account(
            response(
                Some(AccountWire {
                    account_type: "futureProvider".to_owned(),
                    email: None,
                    plan_type: Some("business".to_owned()),
                    credential_source: Some("future".to_owned()),
                    uses_codex_managed_credentials: None,
                }),
                false,
            ),
            None,
        );
        assert_eq!(future.account_type.as_deref(), Some("futureProvider"));
        assert_eq!(future.plan_type.as_deref(), Some("business"));
    }

    #[tokio::test]
    async fn account_read_uses_false_refresh_token_and_parses_response() {
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
            assert_eq!(request["method"], ACCOUNT_READ_METHOD);
            assert_eq!(request["params"]["refreshToken"], false);
            assert!(request["params"].get("refreshToken").is_some());
            let response = json!({
                "id": request["id"],
                "result": {
                    "account": {"type": "chatgpt", "email": "ab@example.com", "planType": "pro"},
                    "requiresOpenaiAuth": true,
                    "futureField": "ignored"
                }
            });
            let mut bytes = serde_json::to_vec(&response).expect("response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("response should send");
        });

        let account = super::AccountService::read_account_with_client(&client, None)
            .await
            .expect("account response should parse");
        assert_eq!(account.status, super::AccountStatus::Connected);
        assert_eq!(account.email_masked.as_deref(), Some("a***@example.com"));
        server.await.expect("server should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn malformed_account_response_returns_a_safe_parse_error() {
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
            let response = json!({
                "id": request["id"],
                "result": {
                    "account": {"type": "chatgpt", "email": 7},
                    "requiresOpenaiAuth": true
                }
            });
            let mut bytes = serde_json::to_vec(&response).expect("response should serialize");
            bytes.push(b'\n');
            server_writer
                .write_all(&bytes)
                .await
                .expect("response should send");
        });

        let error = super::AccountService::read_account_with_client(&client, None)
            .await
            .expect_err("malformed account response should fail");
        assert!(matches!(
            error,
            AppError::AccountService(message) if message == ACCOUNT_RESPONSE_PARSE_ERROR
        ));

        server.await.expect("server should finish");
        client.shutdown().await;
    }

    #[tokio::test]
    async fn account_updated_triggers_a_safe_refresh_and_updates_cache() {
        let (client_io, server_io) = tokio::io::duplex(4096);
        let (client_reader, client_writer) = split(client_io);
        let (server_reader, mut server_writer) = split(server_io);
        let client = JsonRpcClient::from_io(client_reader, client_writer).await;
        let cache = Arc::new(tokio::sync::RwLock::new(AccountCache {
            current: None,
            stale: true,
        }));
        let receiver = client.subscribe_notifications();
        let watcher = tokio::spawn(watch_notifications(
            Arc::clone(&cache),
            Arc::downgrade(&client),
            receiver,
        ));

        let notification = json!({
            "method": "account/updated",
            "params": {"authMode": "chatgpt", "planType": "plus"}
        });
        let mut notification_bytes =
            serde_json::to_vec(&notification).expect("notification should serialize");
        notification_bytes.push(b'\n');
        server_writer
            .write_all(&notification_bytes)
            .await
            .expect("notification should send");

        let mut reader = BufReader::new(server_reader);
        let mut line = String::new();
        timeout(Duration::from_secs(1), reader.read_line(&mut line))
            .await
            .expect("refresh request should arrive")
            .expect("request should be readable");
        let request: Value = serde_json::from_str(&line).expect("request should be JSON");
        assert_eq!(request["method"], ACCOUNT_READ_METHOD);
        assert_eq!(request["params"]["refreshToken"], false);
        let response = json!({
            "id": request["id"],
            "result": {
                "account": {"type": "chatgpt", "email": "user@example.com", "planType": "pro"},
                "requiresOpenaiAuth": true
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
        let cached = cache.read().await.current.clone().expect("account cache");
        assert_eq!(cached.auth_mode.as_deref(), Some("chatgpt"));
        assert_eq!(cached.plan_type.as_deref(), Some("pro"));

        watcher.abort();
        let _ = watcher.await;
        client.shutdown().await;
    }

    #[tokio::test]
    async fn same_client_identity_is_reused_but_new_client_is_not() {
        let (first_io, _) = tokio::io::duplex(64);
        let (first_reader, first_writer) = split(first_io);
        let first = JsonRpcClient::from_io(first_reader, first_writer).await;
        let first_weak = Arc::downgrade(&first);
        assert!(same_client(&first_weak, &first));

        let (second_io, _) = tokio::io::duplex(64);
        let (second_reader, second_writer) = split(second_io);
        let second = JsonRpcClient::from_io(second_reader, second_writer).await;
        assert!(!same_client(&first_weak, &second));

        first.shutdown().await;
        second.shutdown().await;
    }

    #[test]
    fn compatibility_status_is_not_enough_without_account_read_check() {
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

        assert!(!super::supports_account_read(&report));
    }
}
