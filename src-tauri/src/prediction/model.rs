use serde::Serialize;

use crate::rate_limit::RateLimitWindowKind;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum QuotaPredictionOutcome {
    DepletionBeforeReset,
    ResetBeforeDepletion,
    AlreadyDepleted,
    Stable,
    InsufficientData,
    ResetUnknown,
    Unavailable,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PredictionConfidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaPrediction {
    pub(crate) outcome: QuotaPredictionOutcome,
    pub(crate) limit_id: Option<String>,
    pub(crate) limit_name: Option<String>,
    pub(crate) window_kind: Option<RateLimitWindowKind>,
    pub(crate) window_duration_mins: Option<i64>,
    pub(crate) used_percent: Option<f64>,
    pub(crate) burn_rate_percent_points_per_hour: Option<f64>,
    pub(crate) estimated_depletion_at: Option<i64>,
    pub(crate) seconds_to_depletion: Option<f64>,
    pub(crate) resets_at: Option<i64>,
    pub(crate) confidence: PredictionConfidence,
    pub(crate) trust_class: String,
    pub(crate) calculated_at: i64,
    pub(crate) message: Option<String>,
}
