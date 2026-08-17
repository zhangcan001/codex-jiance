use serde::Serialize;

use crate::{prediction::QuotaPredictionOutcome, rate_limit::RateLimitWindowKind};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum QuotaAlertType {
    UsageThreshold,
    PredictedDepletion,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum QuotaAlertSeverity {
    Warning,
    High,
    Critical,
    Exhausted,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuotaAlert {
    pub(crate) id: String,
    #[serde(rename = "type")]
    pub(crate) alert_type: QuotaAlertType,
    pub(crate) severity: QuotaAlertSeverity,
    pub(crate) limit_id: Option<String>,
    pub(crate) limit_name: Option<String>,
    pub(crate) window_kind: Option<RateLimitWindowKind>,
    pub(crate) window_duration_mins: Option<i64>,
    pub(crate) used: Option<f64>,
    pub(crate) threshold: Option<f64>,
    pub(crate) prediction_outcome: Option<QuotaPredictionOutcome>,
    pub(crate) seconds_to_depletion: Option<f64>,
    pub(crate) resets_at: Option<i64>,
    pub(crate) trust_class: String,
    pub(crate) created_at: i64,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AlertServiceStatus {
    pub(crate) running: bool,
    pub(crate) notification_permission: String,
    pub(crate) notification_available: bool,
    pub(crate) active_worker: bool,
    pub(crate) alert_count: usize,
    pub(crate) latest_alerts: Vec<QuotaAlert>,
}
