use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageReadResponse {
    #[serde(default)]
    pub(crate) summary: Option<UsageSummaryWire>,
    #[serde(default)]
    pub(crate) daily_usage_buckets: Option<Vec<DailyUsageBucketWire>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageSummaryWire {
    #[serde(default)]
    pub(crate) lifetime_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) peak_daily_tokens: Option<u64>,
    #[serde(default)]
    pub(crate) longest_running_turn_sec: Option<u64>,
    #[serde(default)]
    pub(crate) current_streak_days: Option<u64>,
    #[serde(default)]
    pub(crate) longest_streak_days: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DailyUsageBucketWire {
    pub(crate) start_date: String,
    pub(crate) tokens: u64,
}
