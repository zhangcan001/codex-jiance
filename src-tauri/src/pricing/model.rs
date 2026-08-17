use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenCostInput {
    pub model: String,
    pub uncached_input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CostBreakdown {
    pub model: String,
    pub profile_id: String,
    pub uncached_input_usd: f64,
    pub cached_input_usd: f64,
    pub cache_write_usd: f64,
    pub output_usd: f64,
    pub total_usd: f64,
    pub long_context_applied: bool,
    pub pricing_effective_date: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiEquivalentCostAvailability {
    Available,
    MissingTokenBreakdown,
    UnsupportedModel,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PricingError {
    #[error("pricing is unavailable for this model")]
    PricingUnavailable,
    #[error("cache-write pricing is not supported for this model")]
    UnsupportedPricingComponent,
}
