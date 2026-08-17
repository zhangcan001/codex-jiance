use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelUsageAggregate {
    pub(crate) model_id: String,
    pub(crate) model_source: String,
    pub(crate) event_count: usize,
    pub(crate) thread_count: usize,
    pub(crate) total_tokens: u64,
    pub(crate) input_tokens: u64,
    pub(crate) uncached_input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) cache_hit_percent: Option<f64>,
    pub(crate) api_equivalent_cost_usd: Option<f64>,
    pub(crate) pricing_available: bool,
    pub(crate) pricing_effective_date: Option<String>,
    pub(crate) priced_event_count: usize,
    pub(crate) unpriced_event_count: usize,
    pub(crate) pricing_coverage_percent: f64,
    pub(crate) first_observed_at: Option<i64>,
    pub(crate) last_observed_at: Option<i64>,
    pub(crate) trust_class: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelUsageReport {
    pub(crate) models: Vec<ModelUsageAggregate>,
    pub(crate) observed_delta_events: usize,
    pub(crate) priced_delta_events: usize,
    pub(crate) pricing_coverage_percent: f64,
    pub(crate) total_api_equivalent_cost_usd: Option<f64>,
    pub(crate) start_at: Option<i64>,
    pub(crate) end_at: Option<i64>,
}
