use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitReadResponse {
    #[serde(default)]
    pub(crate) rate_limits: Option<RateLimitBucketWire>,
    #[serde(default)]
    pub(crate) rate_limits_by_limit_id: Option<HashMap<String, RateLimitBucketWire>>,
    #[serde(default)]
    pub(crate) rate_limit_reset_credits: Option<RateLimitResetCreditsWire>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitBucketWire {
    #[serde(default)]
    pub(crate) limit_id: Option<String>,
    #[serde(default)]
    pub(crate) limit_name: Option<String>,
    #[serde(default)]
    pub(crate) primary: Option<RateLimitWindowWire>,
    #[serde(default)]
    pub(crate) secondary: Option<RateLimitWindowWire>,
    #[serde(default)]
    pub(crate) plan_type: Option<String>,
    #[serde(default)]
    pub(crate) rate_limit_reached_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitWindowWire {
    #[serde(default)]
    pub(crate) used_percent: Option<f64>,
    #[serde(default)]
    pub(crate) window_duration_mins: Option<i64>,
    #[serde(default)]
    pub(crate) resets_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RateLimitResetCreditsWire {
    #[serde(default)]
    pub(crate) available_count: Option<u64>,
}
