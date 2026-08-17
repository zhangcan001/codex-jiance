pub(crate) const PRICING_EFFECTIVE_DATE: &str = "2026-08-17";

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LongContextRule {
    pub(crate) threshold_tokens: u64,
    pub(crate) input_multiplier: f64,
    pub(crate) output_multiplier: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PricingProfile {
    pub(crate) id: &'static str,
    pub(crate) uncached_input_usd_per_million: f64,
    pub(crate) cached_input_usd_per_million: f64,
    pub(crate) cache_write_usd_per_million: Option<f64>,
    pub(crate) output_usd_per_million: f64,
    pub(crate) long_context: Option<LongContextRule>,
}

const LONG_CONTEXT: LongContextRule = LongContextRule {
    threshold_tokens: 272_000,
    input_multiplier: 2.0,
    output_multiplier: 1.5,
};

const GPT_56_SOL: PricingProfile = PricingProfile {
    id: "gpt-5.6-sol",
    uncached_input_usd_per_million: 5.00,
    cached_input_usd_per_million: 0.50,
    cache_write_usd_per_million: Some(6.25),
    output_usd_per_million: 30.00,
    long_context: Some(LONG_CONTEXT),
};

const GPT_56_TERRA: PricingProfile = PricingProfile {
    id: "gpt-5.6-terra",
    uncached_input_usd_per_million: 2.50,
    cached_input_usd_per_million: 0.25,
    cache_write_usd_per_million: Some(3.125),
    output_usd_per_million: 15.00,
    long_context: Some(LONG_CONTEXT),
};

const GPT_56_LUNA: PricingProfile = PricingProfile {
    id: "gpt-5.6-luna",
    uncached_input_usd_per_million: 1.00,
    cached_input_usd_per_million: 0.10,
    cache_write_usd_per_million: Some(1.25),
    output_usd_per_million: 6.00,
    long_context: Some(LONG_CONTEXT),
};

const GPT_55: PricingProfile = PricingProfile {
    id: "gpt-5.5",
    uncached_input_usd_per_million: 5.00,
    cached_input_usd_per_million: 0.50,
    cache_write_usd_per_million: None,
    output_usd_per_million: 30.00,
    long_context: None,
};

const GPT_54: PricingProfile = PricingProfile {
    id: "gpt-5.4",
    uncached_input_usd_per_million: 2.50,
    cached_input_usd_per_million: 0.25,
    cache_write_usd_per_million: None,
    output_usd_per_million: 15.00,
    long_context: None,
};

const GPT_54_MINI: PricingProfile = PricingProfile {
    id: "gpt-5.4-mini",
    uncached_input_usd_per_million: 0.75,
    cached_input_usd_per_million: 0.075,
    cache_write_usd_per_million: None,
    output_usd_per_million: 4.50,
    long_context: None,
};

const GPT_53_CODEX: PricingProfile = PricingProfile {
    id: "gpt-5.3-codex",
    uncached_input_usd_per_million: 1.75,
    cached_input_usd_per_million: 0.175,
    cache_write_usd_per_million: None,
    output_usd_per_million: 14.00,
    long_context: None,
};

const GPT_52: PricingProfile = PricingProfile {
    id: "gpt-5.2",
    uncached_input_usd_per_million: 1.75,
    cached_input_usd_per_million: 0.175,
    cache_write_usd_per_million: None,
    output_usd_per_million: 14.00,
    long_context: None,
};

pub(crate) const CATALOG: &[PricingProfile] = &[
    GPT_56_SOL,
    GPT_56_TERRA,
    GPT_56_LUNA,
    GPT_55,
    GPT_54,
    GPT_54_MINI,
    GPT_53_CODEX,
    GPT_52,
];

pub(crate) fn resolve_profile(model: &str) -> Option<&'static PricingProfile> {
    exact_profile(model).or_else(|| snapshot_alias_base(model).and_then(exact_profile))
}

fn exact_profile(model: &str) -> Option<&'static PricingProfile> {
    let profile_id = match model {
        "gpt-5.6-sol" | "gpt-5.6" => "gpt-5.6-sol",
        "gpt-5.6-terra" => "gpt-5.6-terra",
        "gpt-5.6-luna" => "gpt-5.6-luna",
        "gpt-5.5" => "gpt-5.5",
        "gpt-5.4" => "gpt-5.4",
        "gpt-5.4-mini" => "gpt-5.4-mini",
        "gpt-5.3-codex" => "gpt-5.3-codex",
        "gpt-5.2" => "gpt-5.2",
        _ => return None,
    };

    CATALOG.iter().find(|profile| profile.id == profile_id)
}

fn snapshot_alias_base(model: &str) -> Option<&str> {
    let (prefix, day) = model.rsplit_once('-')?;
    let (base, month) = prefix.rsplit_once('-')?;
    let (base, year) = base.rsplit_once('-')?;

    if year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && year.bytes().all(|byte| byte.is_ascii_digit())
        && month.bytes().all(|byte| byte.is_ascii_digit())
        && day.bytes().all(|byte| byte.is_ascii_digit())
    {
        Some(base)
    } else {
        None
    }
}
