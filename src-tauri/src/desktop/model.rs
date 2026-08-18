use serde::Serialize;

use crate::rate_limit::RateLimitInfo;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DesktopDataStatus {
    Ready,
    Indexing,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopEnvironmentInfo {
    pub(crate) status: DesktopDataStatus,
    pub(crate) codex_home: Option<String>,
    pub(crate) sessions_path: Option<String>,
    pub(crate) state_database_path: Option<String>,
    pub(crate) state_db_compatible: bool,
    pub(crate) desktop_data_available: bool,
    pub(crate) desktop_running: Option<bool>,
    pub(crate) desktop_process_pid: Option<u32>,
    pub(crate) runtime_version: Option<String>,
    pub(crate) last_activity_at: Option<i64>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopMonitorStatus {
    pub(crate) environment: DesktopEnvironmentInfo,
    pub(crate) indexed_desktop_sessions: usize,
    pub(crate) tracked_rollouts: usize,
    pub(crate) desktop_token_events: usize,
    pub(crate) delta_events: usize,
    pub(crate) baseline_only_events: usize,
    pub(crate) raw_rate_limit_events: usize,
    pub(crate) parsed_rate_limit_observations: usize,
    pub(crate) reconciliation_checked: usize,
    pub(crate) reconciliation_matched: usize,
    pub(crate) reconciliation_mismatched: usize,
    pub(crate) index_revision: i64,
    pub(crate) last_scan_at: Option<i64>,
    pub(crate) last_desktop_event_at: Option<i64>,
    pub(crate) backfill_complete: bool,
    pub(crate) backfill_truncated: bool,
    pub(crate) backfill_indexed: usize,
    pub(crate) backfill_total: usize,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopUsageActivity {
    pub(crate) status: String,
    pub(crate) observed_tokens: u64,
    pub(crate) today_tokens: u64,
    pub(crate) observed_threads: usize,
    pub(crate) observed_turns: usize,
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) uncached_input_tokens: u64,
    pub(crate) cached_input_ratio_percent: f64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) last_desktop_activity: Option<i64>,
    pub(crate) pricing_coverage_percent: f64,
    pub(crate) api_equivalent_cost_usd: Option<f64>,
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DesktopThreadUsageStatus {
    Observing,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopThreadUsageInfo {
    pub(crate) status: DesktopThreadUsageStatus,
    pub(crate) coverage: String,
    pub(crate) inventory_thread_count: usize,
    pub(crate) inventory_truncated: bool,
    pub(crate) observed_thread_count: usize,
    pub(crate) snapshot_count: usize,
    pub(crate) latest_observed_at: Option<i64>,
    pub(crate) coverage_gap_detected: bool,
    pub(crate) message: String,
}

#[derive(Debug, Clone)]
pub(crate) struct DesktopRateLimitSnapshot {
    pub(crate) info: RateLimitInfo,
}
