use serde::Serialize;

use crate::{model_usage::ModelUsageAggregate, project::ProjectUsageAggregate};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitHistoryPoint {
    pub(crate) captured_at: i64,
    pub(crate) limit_id: Option<String>,
    pub(crate) kind: String,
    pub(crate) duration: Option<i64>,
    pub(crate) used_percent: f64,
    pub(crate) resets_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TokenHistoryPoint {
    pub(crate) observed_at: i64,
    pub(crate) delta_total_tokens: u64,
    pub(crate) delta_input_tokens: u64,
    pub(crate) delta_cached_input_tokens: u64,
    pub(crate) delta_cache_write_input_tokens: u64,
    pub(crate) delta_output_tokens: u64,
    pub(crate) delta_reasoning_output_tokens: u64,
    pub(crate) project_key: Option<String>,
    pub(crate) model_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryCoverage {
    pub(crate) thread_usage: String,
    pub(crate) observed_threads: usize,
    pub(crate) delta_events: usize,
    pub(crate) baseline_events: usize,
    pub(crate) unknown_project_events: usize,
    pub(crate) unknown_model_events: usize,
    pub(crate) pricing_coverage_percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitoringHistory {
    pub(crate) rate_limit_series: Vec<RateLimitHistoryPoint>,
    pub(crate) token_series: Vec<TokenHistoryPoint>,
    pub(crate) project_summary: Vec<ProjectUsageAggregate>,
    pub(crate) model_summary: Vec<ModelUsageAggregate>,
    pub(crate) coverage: HistoryCoverage,
    pub(crate) start_at: Option<i64>,
    pub(crate) end_at: Option<i64>,
}
