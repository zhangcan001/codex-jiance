use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
};

use serde_json::json;
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
    account::unix_timestamp,
    codex::app_server::{AppServerManager, JsonRpcClient, SchemaCompatibilityService},
    error::AppError,
    models::codex::SchemaCompatibilityStatus,
};

use super::{
    model::{ThreadUsageInfo, ThreadUsageStatus},
    repository::{normalize_project, ThreadMetadataRecord, ThreadUsageRepository},
    wire::{
        source_string, ModelReroutedParams, ThreadListResponseWire, ThreadSettingsUpdatedParams,
        ThreadTokenUsageUpdatedParams,
    },
};

const THREAD_LIST_METHOD: &str = "thread/list";
const TOKEN_USAGE_UPDATED_METHOD: &str = "thread/tokenUsage/updated";
const THREAD_SETTINGS_UPDATED_METHOD: &str = "thread/settings/updated";
const MODEL_REROUTED_METHOD: &str = "model/rerouted";
const MAX_THREADS: usize = 5_000;
const MAX_PAGES: usize = 50;
const COVERAGE: &str = "Current App Server connection";

#[derive(Default)]
struct ThreadInventoryCache {
    thread_count: usize,
    truncated: bool,
    loaded: bool,
}

struct ThreadUsageWatcher {
    client: Weak<JsonRpcClient>,
    task: tokio::task::JoinHandle<()>,
}

pub(crate) struct ThreadUsageService {
    app_server_manager: Arc<AppServerManager>,
    compatibility_service: Arc<SchemaCompatibilityService>,
    repository: Arc<ThreadUsageRepository>,
    inventory: Arc<RwLock<ThreadInventoryCache>>,
    watcher: Mutex<Option<ThreadUsageWatcher>>,
    model_overrides: Arc<RwLock<HashMap<(String, String), String>>>,
    coverage_gap_detected: Arc<AtomicBool>,
}

impl ThreadUsageService {
    pub(crate) fn new(
        app_server_manager: Arc<AppServerManager>,
        compatibility_service: Arc<SchemaCompatibilityService>,
        repository: Arc<ThreadUsageRepository>,
    ) -> Self {
        Self {
            app_server_manager,
            compatibility_service,
            repository,
            inventory: Arc::new(RwLock::new(ThreadInventoryCache::default())),
            watcher: Mutex::new(None),
            model_overrides: Arc::new(RwLock::new(HashMap::new())),
            coverage_gap_detected: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) async fn get_status(&self, force_inventory: bool) -> ThreadUsageInfo {
        let compatibility = self.compatibility_service.check(false).await;
        if compatibility.status == SchemaCompatibilityStatus::Error {
            return unavailable_info(
                ThreadUsageStatus::Error,
                compatibility
                    .message
                    .as_deref()
                    .unwrap_or("Schema compatibility check failed."),
            );
        }
        if !compatibility.advanced_thread_usage_supported {
            return unavailable_info(
                ThreadUsageStatus::Unavailable,
                "Installed Codex schema does not expose passive thread token usage.",
            );
        }

        let client = match self.app_server_manager.initialized_client().await {
            Ok(client) => client,
            Err(_) => {
                return unavailable_info(
                    ThreadUsageStatus::Unavailable,
                    "Start and initialize the Codex App Server to observe thread token usage.",
                )
            }
        };
        self.ensure_watcher(&client).await;

        let should_inventory = force_inventory || !self.inventory.read().await.loaded;
        if should_inventory {
            if let Err(error) = self.refresh_inventory(&client).await {
                log::warn!("Thread inventory refresh failed: {error}");
            }
        }

        let inventory = self.inventory.read().await;
        let (observed_thread_count, snapshot_count, delta_events, latest_observed_at) =
            match self.repository.usage_counts().await {
                Ok(counts) => counts,
                Err(error) => {
                    return unavailable_info(
                        ThreadUsageStatus::Error,
                        &format!("Thread usage database query failed: {error}"),
                    )
                }
            };
        let message = if snapshot_count == 0 {
            "No token-usage events observed on this App Server connection".to_owned()
        } else {
            format!("Observed {delta_events} derived token delta event(s) on this App Server connection")
        };
        ThreadUsageInfo {
            status: ThreadUsageStatus::Observing,
            coverage: COVERAGE.to_owned(),
            inventory_thread_count: inventory.thread_count,
            inventory_truncated: inventory.truncated,
            observed_thread_count,
            snapshot_count,
            latest_observed_at,
            coverage_gap_detected: self.coverage_gap_detected.load(Ordering::Acquire),
            message,
        }
    }

    pub(crate) async fn shutdown(&self) {
        let watcher = self.watcher.lock().await.take();
        if let Some(watcher) = watcher {
            watcher.task.abort();
            let _ = watcher.task.await;
        }
    }

    async fn ensure_watcher(&self, client: &Arc<JsonRpcClient>) {
        let mut watcher = self.watcher.lock().await;
        if watcher.as_ref().is_some_and(|watcher| {
            !watcher.task.is_finished() && same_client(&watcher.client, client)
        }) {
            return;
        }
        if let Some(previous) = watcher.take() {
            previous.task.abort();
            let _ = previous.task.await;
        }
        let receiver = client.subscribe_notifications();
        let client_weak = Arc::downgrade(client);
        let task = tokio::spawn(watch_notifications(
            Arc::clone(&self.repository),
            self.model_overrides.clone(),
            self.coverage_gap_detected.clone(),
            receiver,
        ));
        *watcher = Some(ThreadUsageWatcher {
            client: client_weak,
            task,
        });
        log::info!("Thread usage notification watcher attached");
    }

    async fn refresh_inventory(&self, client: &Arc<JsonRpcClient>) -> Result<(), AppError> {
        let mut cursor = None;
        let mut count = 0usize;
        let mut truncated = false;
        for page in 0..MAX_PAGES {
            let mut params = json!({
                "archived": false,
                "limit": 100,
                "sortKey": "updated_at"
            });
            if let Some(cursor) = cursor.as_ref() {
                params["cursor"] = json!(cursor);
            }
            let response = client.request(THREAD_LIST_METHOD, params).await?;
            let response: ThreadListResponseWire =
                serde_json::from_value(response).map_err(|_| {
                    AppError::RpcProtocol("Thread list response could not be parsed.".to_owned())
                })?;
            for thread in response.data.items {
                if count >= MAX_THREADS {
                    truncated = true;
                    break;
                }
                self.repository
                    .upsert_metadata(&metadata_from_thread(thread))
                    .await?;
                count += 1;
            }
            if truncated || response.next_cursor.is_none() {
                break;
            }
            if page + 1 == MAX_PAGES {
                truncated = true;
                break;
            }
            cursor = response.next_cursor;
        }
        let mut inventory = self.inventory.write().await;
        inventory.thread_count = count;
        inventory.truncated = truncated;
        inventory.loaded = true;
        Ok(())
    }
}

async fn watch_notifications(
    repository: Arc<ThreadUsageRepository>,
    model_overrides: Arc<RwLock<HashMap<(String, String), String>>>,
    coverage_gap_detected: Arc<AtomicBool>,
    mut receiver: broadcast::Receiver<crate::codex::app_server::json_rpc::RpcNotification>,
) {
    loop {
        let notification = match receiver.recv().await {
            Ok(notification) => notification,
            Err(broadcast::error::RecvError::Lagged(_)) => {
                coverage_gap_detected.store(true, Ordering::Release);
                log::warn!("Thread usage notification coverage gap detected");
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => return,
        };
        match notification.method.as_str() {
            TOKEN_USAGE_UPDATED_METHOD => {
                let event = match serde_json::from_value::<ThreadTokenUsageUpdatedParams>(
                    notification.params,
                ) {
                    Ok(event) => event,
                    Err(error) => {
                        log::warn!("Ignoring invalid thread token usage notification: {error}");
                        continue;
                    }
                };
                let model = model_overrides
                    .read()
                    .await
                    .get(&(event.thread_id.clone(), event.turn_id.clone()))
                    .cloned();
                let model = model.as_deref().map(|model| (model, "rerouted"));
                if let Err(error) = repository
                    .persist_token_event(&event, unix_timestamp(), model)
                    .await
                {
                    log::warn!("Ignoring invalid thread token usage notification: {error}");
                }
            }
            THREAD_SETTINGS_UPDATED_METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<ThreadSettingsUpdatedParams>(notification.params)
                {
                    if let Err(error) = repository
                        .update_settings(
                            &params.thread_id,
                            params.thread_settings.cwd.as_deref(),
                            params.thread_settings.model.as_deref(),
                            params.thread_settings.model_provider.as_deref(),
                        )
                        .await
                    {
                        log::warn!("Thread settings metadata update failed: {error}");
                    }
                }
            }
            MODEL_REROUTED_METHOD => {
                if let Ok(params) =
                    serde_json::from_value::<ModelReroutedParams>(notification.params)
                {
                    model_overrides.write().await.insert(
                        (params.thread_id.clone(), params.turn_id),
                        params.to_model.clone(),
                    );
                    if let Err(error) = repository
                        .update_rerouted_model(&params.thread_id, &params.to_model)
                        .await
                    {
                        log::warn!("Thread reroute metadata update failed: {error}");
                    }
                }
            }
            _ => {}
        }
    }
}

fn metadata_from_thread(thread: super::wire::ThreadWire) -> ThreadMetadataRecord {
    let (project_key, project_name) = normalize_project(&thread.cwd);
    ThreadMetadataRecord {
        thread_id: thread.id,
        session_id: thread.session_id,
        forked_from_id: thread.forked_from_id,
        parent_thread_id: thread.parent_thread_id,
        cwd: thread.cwd,
        project_key,
        project_name,
        model_provider: Some(thread.model_provider),
        model_id: None,
        model_source: None,
        cli_version: Some(thread.cli_version),
        source: source_string(thread.source),
        thread_source: thread.thread_source,
        git_sha: thread.git_info.as_ref().and_then(|git| git.sha.clone()),
        git_branch: thread.git_info.and_then(|git| git.branch),
        thread_name: thread.name,
        created_at: thread.created_at,
        updated_at: thread.updated_at,
        recency_at: thread.recency_at,
    }
}

fn same_client(existing: &Weak<JsonRpcClient>, current: &Arc<JsonRpcClient>) -> bool {
    existing
        .upgrade()
        .is_some_and(|existing| Arc::ptr_eq(&existing, current))
}

fn unavailable_info(status: ThreadUsageStatus, message: &str) -> ThreadUsageInfo {
    ThreadUsageInfo {
        status,
        coverage: COVERAGE.to_owned(),
        inventory_thread_count: 0,
        inventory_truncated: false,
        observed_thread_count: 0,
        snapshot_count: 0,
        latest_observed_at: None,
        coverage_gap_detected: false,
        message: message.to_owned(),
    }
}
