use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UsageStatus {
    Available,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageSummary {
    pub(crate) lifetime_tokens: Option<u64>,
    pub(crate) peak_daily_tokens: Option<u64>,
    pub(crate) longest_running_turn_sec: Option<u64>,
    pub(crate) current_streak_days: Option<u64>,
    pub(crate) longest_streak_days: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageBucket {
    pub(crate) start_date: String,
    pub(crate) tokens: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodexUsageInfo {
    pub(crate) status: UsageStatus,
    pub(crate) summary: Option<UsageSummary>,
    pub(crate) daily_buckets: Vec<DailyUsageBucket>,
    pub(crate) updated_at: i64,
    pub(crate) message: Option<String>,
}
