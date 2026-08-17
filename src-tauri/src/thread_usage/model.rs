use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThreadUsageStatus {
    Observing,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadUsageInfo {
    pub(crate) status: ThreadUsageStatus,
    pub(crate) coverage: String,
    pub(crate) inventory_thread_count: usize,
    pub(crate) inventory_truncated: bool,
    pub(crate) observed_thread_count: usize,
    pub(crate) snapshot_count: usize,
    pub(crate) latest_observed_at: Option<i64>,
    pub(crate) coverage_gap_detected: bool,
    pub(crate) message: String,
}
