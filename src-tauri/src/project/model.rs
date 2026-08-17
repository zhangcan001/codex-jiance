use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectUsageAggregate {
    pub(crate) project_key: String,
    pub(crate) project_name: String,
    pub(crate) thread_count: usize,
    pub(crate) observed_event_count: usize,
    pub(crate) attributed_delta_event_count: usize,
    pub(crate) total_tokens: u64,
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) cache_hit_percent: Option<f64>,
    pub(crate) api_equivalent_cost_usd: Option<f64>,
    pub(crate) priced_event_count: usize,
    pub(crate) unpriced_event_count: usize,
    pub(crate) pricing_coverage_percent: f64,
    pub(crate) first_observed_at: Option<i64>,
    pub(crate) last_observed_at: Option<i64>,
    pub(crate) trust_class: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectUsageReport {
    pub(crate) projects: Vec<ProjectUsageAggregate>,
    pub(crate) observed_delta_events: usize,
    pub(crate) unknown_project_events: usize,
    pub(crate) pricing_coverage_percent: f64,
    pub(crate) start_at: Option<i64>,
    pub(crate) end_at: Option<i64>,
}
