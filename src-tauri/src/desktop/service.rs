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
    repository::{CursorRecord, DesktopRepository, DesktopTokenEvent},
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

#[derive(Debug, Clone)]
struct RolloutCandidate {
    path: PathBuf,
    canonical_thread_id: Option<String>,
}

#[derive(Debug, Clone)]
struct KnownRollout {
    canonical_thread_id: Option<String>,
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
            Ok(None) => unavailable_rate_limits("尚未观测到桌面版额度数据。"),
            Err(error) => error_rate_limits(&format!("无法读取桌面版额度：{error}")),
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
    known_files: Mutex<HashMap<PathBuf, KnownRollout>>,
    last_discovery: Mutex<Option<Instant>>,
    raw_rate_limit_events: Mutex<usize>,
    scan_lock: Mutex<()>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

impl DesktopService {
    pub(crate) fn new(
        repository: Arc<DesktopRepository>,
        rate_limit_service: Arc<DesktopRateLimitService>,
    ) -> Arc<Self> {
        let environment = unavailable_environment("桌面版数据源尚未扫描。");
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
                raw_rate_limit_events: 0,
                parsed_rate_limit_observations: 0,
                reconciliation_checked: 0,
                reconciliation_matched: 0,
                reconciliation_mismatched: 0,
                index_revision: 0,
                last_scan_at: None,
                last_desktop_event_at: None,
                backfill_complete: false,
                backfill_truncated: false,
                backfill_indexed: 0,
                backfill_total: 0,
                message: "正在索引桌面版历史记录".to_owned(),
            }),
            known_files: Mutex::new(HashMap::new()),
            last_discovery: Mutex::new(None),
            raw_rate_limit_events: Mutex::new(0),
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

    pub(crate) async fn rebuild(&self) -> Result<DesktopMonitorStatus, AppError> {
        {
            let _scan_guard = self.scan_lock.lock().await;
            self.repository.rebuild_desktop_index().await?;
            self.known_files.lock().await.clear();
            *self.last_discovery.lock().await = None;
            *self.raw_rate_limit_events.lock().await = 0;
        }
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
                    start_date: "今天".to_owned(),
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
                    .then(|| "尚未观测到桌面版 Token 增量。".to_owned()),
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
            message: (totals.total_events == 0).then(|| "尚未观测到桌面版 Token 增量。".to_owned()),
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
                coverage: "Codex 桌面版本地会话".to_owned(),
                inventory_thread_count: counts.indexed_sessions,
                inventory_truncated: self.status.read().await.backfill_truncated,
                observed_thread_count: counts.indexed_sessions,
                snapshot_count: counts.token_events,
                latest_observed_at: counts.latest_event_at,
                coverage_gap_detected: false,
                message: if counts.tracked_rollouts == 0 {
                    "尚未索引桌面版会话。".to_owned()
                } else {
                    format!("已索引 {} 个桌面版会话", counts.tracked_rollouts)
                },
            },
            Err(error) => DesktopThreadUsageInfo {
                status: DesktopThreadUsageStatus::Error,
                coverage: "Codex 桌面版本地会话".to_owned(),
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
        match self.repository.desktop_index_revision().await {
            Ok(Some(revision)) if revision >= 2 => {}
            Ok(_) => {
                if let Err(error) = self.repository.rebuild_desktop_index().await {
                    log::warn!("Desktop derived index rebuild failed: {error}");
                    return;
                }
                self.known_files.lock().await.clear();
                *self.last_discovery.lock().await = None;
                *self.raw_rate_limit_events.lock().await = 0;
            }
            Err(error) => {
                log::warn!("Desktop index revision check failed: {error}");
                return;
            }
        }
        let mut environment = discover_environment().await;
        let Some(paths) = discover_paths() else {
            let mut state = self.status.write().await;
            state.environment = environment;
            state.message = "未找到 Codex 桌面版本地数据。".to_owned();
            return;
        };

        let mut rollout_candidates = Vec::new();
        let mut state_threads = Vec::new();
        let mut state_compatible = false;
        if let Some(state_path) = paths.state_database_path.as_ref() {
            match StateDbReader::open(state_path, &paths.codex_home).await {
                Ok(reader) => {
                    state_compatible = reader.schema.compatible;
                    if let Ok(threads) = reader.threads(MAX_ROLLOUT_FILES).await {
                        state_threads = threads
                            .iter()
                            .map(|thread| (thread.id.clone(), thread.tokens_used))
                            .collect();
                        rollout_candidates.extend(threads.into_iter().filter_map(|thread| {
                            thread.rollout_path.map(|path| RolloutCandidate {
                                path: reader.resolve_rollout_path(&path),
                                canonical_thread_id: Some(thread.id),
                            })
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
            rollout_candidates.append(&mut discovered);
            truncated = was_truncated;
            *self.last_discovery.lock().await = Some(Instant::now());
        }
        let mut candidates_by_path = HashMap::<PathBuf, Option<String>>::new();
        for candidate in rollout_candidates {
            candidates_by_path
                .entry(candidate.path)
                .and_modify(|thread_id| {
                    if thread_id.is_none() {
                        *thread_id = candidate.canonical_thread_id.clone();
                    }
                })
                .or_insert(candidate.canonical_thread_id);
        }
        let mut rollout_candidates = candidates_by_path
            .into_iter()
            .map(|(path, canonical_thread_id)| RolloutCandidate {
                path,
                canonical_thread_id,
            })
            .collect::<Vec<_>>();
        rollout_candidates.sort_by(|left, right| left.path.cmp(&right.path));
        if rollout_candidates.len() > MAX_ROLLOUT_FILES {
            rollout_candidates.truncate(MAX_ROLLOUT_FILES);
            truncated = true;
        }
        {
            let mut known = self.known_files.lock().await;
            for candidate in &rollout_candidates {
                if std::fs::metadata(&candidate.path).is_ok() {
                    known
                        .entry(candidate.path.clone())
                        .and_modify(|entry| {
                            if entry.canonical_thread_id.is_none() {
                                entry.canonical_thread_id = candidate.canonical_thread_id.clone();
                            }
                        })
                        .or_insert(KnownRollout {
                            canonical_thread_id: candidate.canonical_thread_id.clone(),
                        });
                }
            }
        }
        let candidates = self
            .known_files
            .lock()
            .await
            .iter()
            .map(|(path, known)| (path.clone(), known.canonical_thread_id.clone()))
            .collect::<Vec<_>>();
        environment.status = DesktopDataStatus::Indexing;
        environment.state_db_compatible = state_compatible;
        {
            let mut state = self.status.write().await;
            state.environment = environment.clone();
            state.backfill_total = candidates.len();
            state.backfill_truncated = truncated;
            state.message = format!("正在索引桌面版历史记录（{} 个会话）", candidates.len());
        }
        let mut indexed = 0;
        for (path, canonical_thread_id) in candidates {
            if let Err(error) = self
                .process_rollout(&path, canonical_thread_id.as_deref())
                .await
            {
                log::debug!(
                    "Desktop rollout skipped (path only): {}: {error}",
                    path.display()
                );
            }
            indexed += 1;
            let mut state = self.status.write().await;
            state.backfill_indexed = indexed;
        }
        let reconciliation = self
            .repository
            .reconcile_state_threads(&state_threads)
            .await
            .unwrap_or_default();
        if let Ok(counts) = self.repository.counts().await {
            environment.status = DesktopDataStatus::Ready;
            environment.last_activity_at = counts.latest_event_at;
            let index_revision = self
                .repository
                .desktop_index_revision()
                .await
                .ok()
                .flatten()
                .unwrap_or(2);
            let raw_rate_limit_events = counts.parsed_rate_limit_observations;
            let mut state = self.status.write().await;
            state.environment = environment.clone();
            state.indexed_desktop_sessions = counts.indexed_sessions;
            state.tracked_rollouts = counts.tracked_rollouts;
            state.desktop_token_events = counts.token_events;
            state.delta_events = counts.delta_events;
            state.baseline_only_events = counts.baseline_only_events;
            state.raw_rate_limit_events = raw_rate_limit_events;
            state.parsed_rate_limit_observations = counts.parsed_rate_limit_observations;
            state.reconciliation_checked = reconciliation.checked;
            state.reconciliation_matched = reconciliation.matched;
            state.reconciliation_mismatched = reconciliation.mismatched;
            state.index_revision = index_revision;
            state.last_scan_at = Some(now());
            state.last_desktop_event_at = counts.latest_event_at;
            state.backfill_complete = true;
            state.message = "桌面版数据源已就绪。".to_owned();
        }
        let _ = self.rate_limit_service.refresh_from_store().await;
    }

    async fn process_rollout(
        &self,
        path: &Path,
        canonical_thread_id: Option<&str>,
    ) -> Result<(), AppError> {
        let path_key = path.to_string_lossy().into_owned();
        let old = self.repository.cursor(&path_key).await?;
        let result = read_rollout(path, old.byte_offset)?;
        if result.next_offset == old.byte_offset && result.file_size == old.file_size {
            return Ok(());
        }
        let mut session = SessionContext {
            thread_id: canonical_thread_id
                .map(ToOwned::to_owned)
                .or(old.thread_id.clone()),
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
        let mut token_events = Vec::new();
        let mut rate_observations = Vec::new();
        for event in &result.events {
            match event {
                RolloutEvent::SessionMeta(meta) => {
                    apply_session_meta(&mut session, meta, canonical_thread_id)
                }
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
                    let event_at = (*event_at)
                        .or(last_event_at)
                        .or(result.modified_at)
                        .unwrap_or_else(now);
                    let turn_id = turn_id
                        .as_deref()
                        .or(turn.turn_id.as_deref())
                        .unwrap_or("unknown-turn");
                    let cwd = turn.cwd.as_deref().or(session.cwd.as_deref());
                    if let Some(thread_id) = session.thread_id.as_deref() {
                        if let Some(total) = total {
                            token_events.push(DesktopTokenEvent {
                                rollout_path: path_key.clone(),
                                thread_id: thread_id.to_owned(),
                                turn_id: turn_id.to_owned(),
                                observed_at: event_at,
                                total: total.clone(),
                                last: last.clone(),
                                model_context_window: *model_context_window,
                                cwd: cwd.map(ToOwned::to_owned),
                                model: turn.model.clone(),
                                originator: session.originator.clone(),
                                byte_offset: result.next_offset,
                            });
                        }
                    }
                    if !rate_limits.is_empty() {
                        *self.raw_rate_limit_events.lock().await += 1;
                        let mut observations = rate_limits.clone();
                        for observation in &mut observations {
                            if observation.event_at <= 0 {
                                observation.event_at = event_at;
                            }
                            if observation.thread_id.is_none() {
                                observation.thread_id = session.thread_id.clone();
                            }
                        }
                        rate_observations.extend(observations);
                    }
                    last_event_at = Some(event_at);
                }
            }
        }
        self.repository.persist_token_events(&token_events).await?;
        if !rate_observations.is_empty() {
            rate_limit_changed |= self
                .repository
                .persist_rate_limits(&rate_observations)
                .await?;
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
        if result.timestamp_errors > 0 {
            log::debug!(
                "rollout timestamp parse errors: path={path_key} count={}",
                result.timestamp_errors
            );
        }
        if rate_limit_changed {
            self.rate_limit_service.refresh_from_store().await;
        }
        Ok(())
    }
}

fn apply_session_meta(
    session: &mut SessionContext,
    meta: &SessionMeta,
    canonical_thread_id: Option<&str>,
) {
    if canonical_thread_id.is_none() {
        session.thread_id = meta.thread_id.clone().or(session.thread_id.clone());
    }
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

fn discover_rollouts(sessions: &Path) -> (Vec<RolloutCandidate>, bool) {
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
    (
        paths
            .into_iter()
            .map(|path| RolloutCandidate {
                canonical_thread_id: extract_thread_id_from_filename(&path),
                path,
            })
            .collect(),
        truncated,
    )
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

fn extract_thread_id_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let parts = stem.split('-').collect::<Vec<_>>();
    parts.windows(5).find_map(|parts| {
        let lengths = [8, 4, 4, 4, 12];
        (parts.iter().zip(lengths).all(|(part, length)| {
            part.len() == length && part.chars().all(|character| character.is_ascii_hexdigit())
        }))
        .then(|| parts.join("-"))
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_rollout_filename_extracts_uuid_identity() {
        let path = Path::new(
            "rollout-2026-08-18T04-46-07-000000Z-12345678-1234-abcd-ef01-1234567890ab.jsonl",
        );
        assert_eq!(
            extract_thread_id_from_filename(path).as_deref(),
            Some("12345678-1234-abcd-ef01-1234567890ab")
        );
    }

    #[test]
    fn canonical_thread_id_protects_against_fork_session_metadata() {
        let mut session = SessionContext {
            thread_id: Some("thread-B".to_owned()),
            ..SessionContext::default()
        };
        apply_session_meta(
            &mut session,
            &SessionMeta {
                thread_id: Some("thread-A".to_owned()),
                cwd: Some("C:\\Projects\\Demo".to_owned()),
                ..SessionMeta::default()
            },
            Some("thread-B"),
        );
        assert_eq!(session.thread_id.as_deref(), Some("thread-B"));
        assert_eq!(session.cwd.as_deref(), Some("C:\\Projects\\Demo"));
    }

    #[test]
    fn session_metadata_can_define_identity_when_no_canonical_thread_exists() {
        let mut session = SessionContext::default();
        apply_session_meta(
            &mut session,
            &SessionMeta {
                thread_id: Some("thread-A".to_owned()),
                ..SessionMeta::default()
            },
            None,
        );
        assert_eq!(session.thread_id.as_deref(), Some("thread-A"));
    }

    #[test]
    fn desktop_originator_matching_is_case_insensitive_and_narrow() {
        assert!(is_desktop_originator("Codex Desktop"));
        assert!(is_desktop_originator("CHATGPT DESKTOP"));
        assert!(!is_desktop_originator("Codex CLI"));
    }
}
