use serde::Serialize;

use crate::rate_limit::RateLimitWindowKind;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BurnRateStatus {
    Available,
    InsufficientData,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BurnRateEstimate {
    pub(crate) status: BurnRateStatus,
    pub(crate) limit_id: Option<String>,
    pub(crate) limit_name: Option<String>,
    pub(crate) window_kind: Option<RateLimitWindowKind>,
    pub(crate) window_duration_mins: Option<i64>,
    pub(crate) resets_at: Option<i64>,
    pub(crate) latest_used_percent: Option<f64>,
    pub(crate) burn_rate_percent_points_per_hour: Option<f64>,
    pub(crate) sample_count: usize,
    pub(crate) observed_span_sec: Option<i64>,
    pub(crate) used_delta_percent: Option<f64>,
    pub(crate) first_observed_at: Option<i64>,
    pub(crate) last_observed_at: Option<i64>,
    pub(crate) trust_class: String,
    pub(crate) message: Option<String>,
}
