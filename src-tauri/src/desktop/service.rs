use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::{broadcast, Mutex, RwLock};

use crate::{
    error::AppError,
    rate_limit::{RateLimitInfo, RateLimitStatus},
    usage::{CodexUsageInfo, DailyUsageBucket, UsageStatus, UsageSummary},
};

use super::{
    environment::{discover_environment, discover_paths},
    model::{
        DesktopDataStatus, DesktopEnvironmentInfo, DesktopMonitorStatus, DesktopThreadUsageInfo,
        DesktopThreadUsageStatus, DesktopUsageActivity,
    },
    repository::{CursorRecord, DesktopRepository},
    rollout::{read_rollout, RolloutEvent, SessionMeta, TurnContext},
    state_db::StateDbReader,
};

const MAX_ROLLOUT_FILES: usize = 10_000;
const ACTIVE_SCAN_INTERVAL: Duration = Duration::from_secs(3);
const FULL_DISCOVERY_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Default)]
struct SessionContext {
    thread_id: Option<String>,
    originator: Option<String>,
    cli_version: Option<String>,
    cwd: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct TurnState {
    turn_id: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
}

pub(crate) struct DesktopRateLimitService {
    repository: Arc<DesktopRepository>,
    cache: RwLock<Option<RateLimitInfo>>,
    updates: broadcast::Sender<RateLimitInfo>,
}

impl DesktopRateLimitService {
    pub(crate) fn new(repository: Arc<DesktopRepository>) -> Arc<Self> {
        Arc::new(Self {
            repository,
            cache: RwLock::new(None),
            updates: broadcast::channel(16).0,
        })
    }

    pub(crate) fn subscribe_updates(&self) -> broadcast::Receiver<RateLimitInfo> {
        self.updates.subscribe()
    }

    pub(crate) async fn get_rate_limits(&self, force: bool) -> RateLimitInfo {
        if !force {
            if let Some(info) = self.cache.read().await.clone() {
                return info;
            }
        }
        match self.repository.latest_rate_limits().await {
            Ok(Some(snapshot)) => {
                let info = snapshot.info;
                *self.cache.write().await = Some(info.clone());
                info
            }
            Ok(None) => unavailable_rate_limits("No Desktop rate-limit observation yet."),
            Err(error) => {
                error_rate_limits(&format!("Could not read Desktop rate limits: {error}"))
            }
        }
    }

    pub(crate) async fn refresh_from_store(&self) {
        let Ok(Some(snapshot)) = self.repository.latest_rate_limits().await else {
            return;
        };
        let info = snapshot.info;
        let changed = self.cache.read().await.as_ref() != Some(&info);
        *self.cache.write().await = Some(info.clone());
        if changed {
            let _ = self.updates.send(info);
        }
    }

    pub(crate) async fn shutdown(&self) {
        *self.cache.write().await = None;
    }
}

pub(crate) struct DesktopService {
    repository: Arc<DesktopRepository>,
    rate_limit_service: Arc<DesktopRateLimitService>,
    status: RwLock<DesktopMonitorStatus>,
    known_files: Mutex<HashMap<PathBuf, (u64, Option<i64>)>>,
    last_discovery: Mutex<Option<Instant>>,
    scan_lock: Mutex<()>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl DesktopService {
    pub(crate) fn new(
        repository: Arc<DesktopRepository>,
        rate_limit_service: Arc<DesktopRateLimitService>,
    ) -> Arc<Self> {
        let environment = unavailable_environment("Desktop data source has not been scanned yet.");
        Arc::new(Self {
            repository,
            rate_limit_service,
            status: RwLock::new(DesktopMonitorStatus {
                environment,
                indexed_desktop_sessions: 0,
                tracked_rollouts: 0,
                desktop_token_events: 0,
                delta_events: 0,
                baseline_only_events: 0,
                last_scan_at: None,
                last_desktop_event_at: None,
                backfill_complete: false,
                backfill_truncated: false,
                backfill_indexed: 0,
                backfill_total: 0,
                message: "Indexing Desktop history".to_owned(),
            }),
            known_files: Mutex::new(HashMap::new()),
            last_discovery: Mutex::new(None),
            scan_lock: Mutex::new(()),
            task: Mutex::new(None),
        })
    }

    pub(crate) fn start(self: &Arc<Self>) {
        let Ok(mut task_slot) = self.task.try_lock() else {
            return;
        };
        if task_slot
            .as_ref()
            .is_some_and(|task| !task.inner().is_finished())
        {
            return;
        }
        let service = Arc::clone(self);
        *task_slot = Some(tauri::async_runtime::spawn(async move {
            service.scan_once(true).await;
            let mut interval = tokio::time::interval(ACTIVE_SCAN_INTERVAL);
            loop {
                interval.tick().await;
                service.scan_once(false).await;
            }
        }));
    }

    pub(crate) async fn shutdown(&self) {
        if let Some(task) = self.task.lock().await.take() {
            task.abort();
            let _ = task.await;
        }
    }

    pub(crate) async fn refresh(&self) -> Result<DesktopMonitorStatus, AppError> {
        self.scan_once(true).await;
        Ok(self.status().await)
    }

    pub(crate) async fn environment(&self) -> DesktopEnvironmentInfo {
        self.status.read().await.environment.clone()
    }
    pub(crate) async fn status(&self) -> DesktopMonitorStatus {
        self.status.read().await.clone()
    }

    pub(crate) async fn usage(&self) -> CodexUsageInfo {
        match self.repository.activity().await {
            Ok(activity) => CodexUsageInfo {
                status: if activity.total_events == 0 {
                    UsageStatus::Unavailable
                } else {
                    UsageStatus::Available
                },
                summary: Some(UsageSummary {
                    lifetime_tokens: Some(activity.observed_tokens),
                    peak_daily_tokens: Some(activity.today_tokens),
                    longest_running_turn_sec: None,
                    current_streak_days: None,
                    longest_streak_days: None,
                }),
                daily_buckets: vec![DailyUsageBucket {
                    start_date: "Today".to_owned(),
                    tokens: activity.today_tokens,
                }],
                updated_at: self
                    .repository
                    .counts()
                    .await
                    .ok()
                    .and_then(|counts| counts.latest_event_at)
                    .unwrap_or_else(now),
                message: (activity.total_events == 0)
                    .then(|| "No Desktop token deltas observed yet.".to_owned()),
            },
            Err(error) => CodexUsageInfo {
                status: UsageStatus::Error,
                summary: None,
                daily_buckets: Vec::new(),
                updated_at: now(),
                message: Some(error.to_string()),
            },
        }
    }

    pub(crate) async fn activity(&self) -> Result<DesktopUsageActivity, AppError> {
        let totals = self.repository.activity().await?;
        let coverage = if totals.total_events == 0 {
            0.0
        } else {
            totals.priced_events as f64 / totals.total_events as f64 * 100.0
        };
        Ok(DesktopUsageActivity {
            status: if totals.total_events == 0 {
                "unavailable"
            } else {
                "available"
            }
            .to_owned(),
            observed_tokens: totals.observed_tokens,
            today_tokens: totals.today_tokens,
            observed_threads: totals.observed_threads,
            observed_turns: totals.observed_turns,
            input_tokens: totals.input_tokens,
            cached_input_tokens: totals.cached_input_tokens,
            cache_write_input_tokens: totals.cache_write_input_tokens,
            output_tokens: totals.output_tokens,
            reasoning_output_tokens: totals.reasoning_output_tokens,
            last_desktop_activity: self.repository.counts().await?.latest_event_at,
            pricing_coverage_percent: coverage,
            api_equivalent_cost_usd: totals.api_equivalent_cost_usd,
            message: (totals.total_events == 0)
                .then(|| "No Desktop token deltas observed yet.".to_owned()),
        })
    }

    pub(crate) async fn thread_usage(&self) -> DesktopThreadUsageInfo {
        match self.repository.counts().await {
            Ok(counts) => DesktopThreadUsageInfo {
                status: if counts.tracked_rollouts == 0 {
                    DesktopThreadUsageStatus::Unavailable
                } else {
                    DesktopThreadUsageStatus::Observing
                },
                coverage: "Codex Desktop local rollouts".to_owned(),
                inventory_thread_count: counts.indexed_sessions,
                inventory_truncated: self.status.read().await.backfill_truncated,
                observed_thread_count: counts.indexed_sessions,
                snapshot_count: counts.token_events,
                latest_observed_at: counts.latest_event_at,
                coverage_gap_detected: false,
                message: if counts.tracked_rollouts == 0 {
                    "No Desktop rollouts indexed yet.".to_owned()
                } else {
                    format!("Indexed {} Desktop rollout(s)", counts.tracked_rollouts)
                },
            },
            Err(error) => DesktopThreadUsageInfo {
                status: DesktopThreadUsageStatus::Error,
                coverage: "Codex Desktop local rollouts".to_owned(),
                inventory_thread_count: 0,
                inventory_truncated: false,
                observed_thread_count: 0,
                snapshot_count: 0,
                latest_observed_at: None,
                coverage_gap_detected: true,
                message: error.to_string(),
            },
        }
    }

    async fn scan_once(&self, force_discovery: bool) {
        let _scan_guard = self.scan_lock.lock().await;
        let mut environment = discover_environment().await;
        let Some(paths) = discover_paths() else {
            let mut state = self.status.write().await;
            state.environment = environment;
            state.message = "Codex Desktop local data not found.".to_owned();
            return;
        };

        let mut rollout_paths = Vec::new();
        let mut state_compatible = false;
        if let Some(state_path) = paths.state_database_path.as_ref() {
            match StateDbReader::open(state_path, &paths.codex_home).await {
                Ok(reader) => {
                    state_compatible = reader.schema.compatible;
                    if let Ok(threads) = reader.threads(MAX_ROLLOUT_FILES).await {
                        rollout_paths.extend(threads.into_iter().filter_map(|thread| {
                            thread
                                .rollout_path
                                .map(|path| reader.resolve_rollout_path(&path))
                        }));
                    }
                }
                Err(error) => {
                    log::debug!("Desktop state DB unavailable; using session files: {error}")
                }
            }
        }
        let should_discover = force_discovery
            || self
                .last_discovery
                .lock()
                .await
                .is_none_or(|value| value.elapsed() >= FULL_DISCOVERY_INTERVAL);
        let mut truncated = false;
        if should_discover {
            let (mut discovered, was_truncated) = discover_rollouts(&paths.sessions_path);
            rollout_paths.append(&mut discovered);
            truncated = was_truncated;
            *self.last_discovery.lock().await = Some(Instant::now());
        }
        rollout_paths.sort();
        rollout_paths.dedup();
        if rollout_paths.len() > MAX_ROLLOUT_FILES {
            rollout_paths.truncate(MAX_ROLLOUT_FILES);
            truncated = true;
        }
        {
            let mut known = self.known_files.lock().await;
            for path in &rollout_paths {
                if let Ok(metadata) = std::fs::metadata(path) {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                        .and_then(|value| i64::try_from(value.as_secs()).ok());
                    known
                        .entry(path.clone())
                        .or_insert((metadata.len(), modified));
                }
            }
        }
        let candidates = self
            .known_files
            .lock()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        {
            let mut state = self.status.write().await;
            state.environment.status = DesktopDataStatus::Indexing;
            state.environment.state_db_compatible = state_compatible;
            state.backfill_total = candidates.len();
            state.backfill_truncated = truncated;
            state.message = format!("Indexing Desktop history ({} sessions)", candidates.len());
        }
        let mut indexed = 0;
        for path in candidates {
            if let Err(error) = self.process_rollout(&path).await {
                log::debug!(
                    "Desktop rollout skipped (path only): {}: {error}",
                    path.display()
                );
            }
            indexed += 1;
            let mut state = self.status.write().await;
            state.backfill_indexed = indexed;
        }
        if let Ok(counts) = self.repository.counts().await {
            let mut state = self.status.write().await;
            state.environment.status = DesktopDataStatus::Ready;
            state.environment.last_activity_at = counts.latest_event_at;
            state.indexed_desktop_sessions = counts.indexed_sessions;
            state.tracked_rollouts = counts.tracked_rollouts;
            state.desktop_token_events = counts.token_events;
            state.delta_events = counts.delta_events;
            state.baseline_only_events = counts.baseline_only_events;
            state.last_scan_at = Some(now());
            state.last_desktop_event_at = counts.latest_event_at;
            state.backfill_complete = true;
            state.message = "Desktop data source ready.".to_owned();
        }
        let _ = self.rate_limit_service.refresh_from_store().await;
        environment.status = DesktopDataStatus::Ready;
    }

    async fn process_rollout(&self, path: &Path) -> Result<(), AppError> {
        let path_key = path.to_string_lossy().into_owned();
        let old = self.repository.cursor(&path_key).await?;
        let result = read_rollout(path, old.byte_offset)?;
        if result.next_offset == old.byte_offset && result.file_size == old.file_size {
            return Ok(());
        }
        let mut session = SessionContext {
            thread_id: old.thread_id.clone(),
            originator: old.originator.clone(),
            ..SessionContext::default()
        };
        let mut turn = TurnState::default();
        if let Some(thread_id) = session.thread_id.as_deref() {
            if let Some((cwd, model, cli_version)) =
                self.repository.thread_context(thread_id).await?
            {
                session.cwd = cwd;
                session.cli_version = cli_version;
                turn.model = model;
            }
        }
        let mut last_event_at = old.last_event_at;
        let mut rate_limit_changed = false;
        for event in &result.events {
            match event {
                RolloutEvent::SessionMeta(meta) => apply_session_meta(&mut session, meta),
                RolloutEvent::TurnContext(context) => apply_turn_context(&mut turn, context),
                RolloutEvent::TokenCount {
                    event_at,
                    turn_id,
                    total,
                    last,
                    model_context_window,
                    rate_limits,
                } => {
                    let is_desktop = session
                        .originator
                        .as_deref()
                        .is_some_and(is_desktop_originator);
                    if !is_desktop {
                        continue;
                    }
                    let Some(thread_id) = session.thread_id.as_deref() else {
                        continue;
                    };
                    let event_at = (*event_at > 0)
                        .then_some(*event_at)
                        .or(last_event_at)
                        .unwrap_or_else(now);
                    let turn_id = turn_id
                        .as_deref()
                        .or(turn.turn_id.as_deref())
                        .unwrap_or("unknown-turn");
                    let cwd = turn.cwd.as_deref().or(session.cwd.as_deref());
                    self.repository
                        .upsert_thread(
                            thread_id,
                            cwd,
                            turn.model.as_deref(),
                            session.cli_version.as_deref(),
                            session.originator.as_deref(),
                            event_at,
                        )
                        .await?;
                    self.repository
                        .persist_token_event(
                            &path_key,
                            thread_id,
                            turn_id,
                            event_at,
                            total,
                            last,
                            *model_context_window,
                            cwd,
                            turn.model.as_deref(),
                            session.originator.as_deref(),
                            result.next_offset,
                        )
                        .await?;
                    if !rate_limits.is_empty() {
                        rate_limit_changed |=
                            self.repository.persist_rate_limits(rate_limits).await?;
                    }
                    last_event_at = Some(event_at);
                }
            }
        }
        let is_desktop = session
            .originator
            .as_deref()
            .is_some_and(is_desktop_originator);
        let cursor = CursorRecord {
            thread_id: session.thread_id,
            byte_offset: result.next_offset,
            file_size: result.file_size,
            modified_at: result.modified_at,
            last_event_at,
            originator: session.originator,
            is_desktop,
        };
        self.repository.save_cursor(&path_key, &cursor).await?;
        if result.oversized_lines > 0 {
            log::warn!("oversized irrelevant rollout line skipped: path={path_key}");
        }
        if result.parse_errors > 0 {
            log::debug!(
                "rollout parse errors: path={path_key} count={}",
                result.parse_errors
            );
        }
        if rate_limit_changed {
            self.rate_limit_service.refresh_from_store().await;
        }
        Ok(())
    }
}

fn apply_session_meta(session: &mut SessionContext, meta: &SessionMeta) {
    session.thread_id = meta.thread_id.clone().or(session.thread_id.clone());
    session.originator = meta.originator.clone().or(session.originator.clone());
    session.cli_version = meta.cli_version.clone().or(session.cli_version.clone());
    session.cwd = meta.cwd.clone().or(session.cwd.clone());
}
fn apply_turn_context(turn: &mut TurnState, context: &TurnContext) {
    turn.turn_id = context.turn_id.clone().or(turn.turn_id.clone());
    turn.model = context.model.clone().or(turn.model.clone());
    turn.cwd = context.cwd.clone().or(turn.cwd.clone());
}
pub(crate) fn is_desktop_originator(originator: &str) -> bool {
    matches!(
        originator.to_ascii_lowercase().as_str(),
        "codex desktop" | "chatgpt desktop"
    )
}

fn discover_rollouts(sessions: &Path) -> (Vec<PathBuf>, bool) {
    let mut paths = Vec::new();
    collect_rollouts(sessions, &mut paths);
    paths.sort_by_key(|path| {
        std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
    });
    paths.reverse();
    let truncated = paths.len() > MAX_ROLLOUT_FILES;
    if truncated {
        paths.truncate(MAX_ROLLOUT_FILES);
    }
    (paths, truncated)
}
fn collect_rollouts(path: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rollouts(&path, paths);
        } else if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.starts_with("rollout-") && name.ends_with(".jsonl"))
        {
            paths.push(path);
        }
    }
}
fn unavailable_environment(message: &str) -> DesktopEnvironmentInfo {
    DesktopEnvironmentInfo {
        status: DesktopDataStatus::Unavailable,
        codex_home: None,
        sessions_path: None,
        state_database_path: None,
        state_db_compatible: false,
        desktop_data_available: false,
        desktop_running: None,
        desktop_process_pid: None,
        runtime_version: None,
        last_activity_at: None,
        message: message.to_owned(),
    }
}
fn unavailable_rate_limits(message: &str) -> RateLimitInfo {
    RateLimitInfo {
        status: RateLimitStatus::Unavailable,
        windows: Vec::new(),
        reset_credits_available: None,
        updated_at: now(),
        message: Some(message.to_owned()),
    }
}
fn error_rate_limits(message: &str) -> RateLimitInfo {
    RateLimitInfo {
        status: RateLimitStatus::Error,
        windows: Vec::new(),
        reset_credits_available: None,
        updated_at: now(),
        message: Some(message.to_owned()),
    }
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}
