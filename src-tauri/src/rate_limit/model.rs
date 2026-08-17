use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RateLimitStatus {
    Available,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RateLimitWindowKind {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitWindow {
    pub(crate) limit_id: Option<String>,
    pub(crate) limit_name: Option<String>,
    pub(crate) window_kind: RateLimitWindowKind,
    pub(crate) used_percent: f64,
    pub(crate) remaining_percent: f64,
    pub(crate) window_duration_mins: Option<i64>,
    pub(crate) resets_at: Option<i64>,
    pub(crate) plan_type: Option<String>,
    pub(crate) rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitInfo {
    pub(crate) status: RateLimitStatus,
    pub(crate) windows: Vec<RateLimitWindow>,
    pub(crate) reset_credits_available: Option<u64>,
    pub(crate) updated_at: i64,
    pub(crate) message: Option<String>,
}
