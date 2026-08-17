use super::catalog::{resolve_profile, PRICING_EFFECTIVE_DATE};
use super::model::{CostBreakdown, PricingError, TokenCostInput};

pub fn calculate_api_equivalent_cost(input: TokenCostInput) -> Result<CostBreakdown, PricingError> {
    let profile = resolve_profile(&input.model).ok_or(PricingError::PricingUnavailable)?;

    let cache_write_rate = match (
        input.cache_write_input_tokens,
        profile.cache_write_usd_per_million,
    ) {
        (0, _) => 0.0,
        (_, Some(rate)) => rate,
        (_, None) => return Err(PricingError::UnsupportedPricingComponent),
    };

    let total_input_tokens = input
        .uncached_input_tokens
        .saturating_add(input.cached_input_tokens)
        .saturating_add(input.cache_write_input_tokens);
    let long_context_rule = profile
        .long_context
        .filter(|rule| total_input_tokens > rule.threshold_tokens);
    let input_multiplier = long_context_rule.map_or(1.0, |rule| rule.input_multiplier);
    let output_multiplier = long_context_rule.map_or(1.0, |rule| rule.output_multiplier);

    let uncached_input_usd = cost(
        input.uncached_input_tokens,
        profile.uncached_input_usd_per_million,
        input_multiplier,
    );
    let cached_input_usd = cost(
        input.cached_input_tokens,
        profile.cached_input_usd_per_million,
        input_multiplier,
    );
    let cache_write_usd = cost(
        input.cache_write_input_tokens,
        cache_write_rate,
        input_multiplier,
    );
    let output_usd = cost(
        input.output_tokens,
        profile.output_usd_per_million,
        output_multiplier,
    );
    let total_usd = uncached_input_usd + cached_input_usd + cache_write_usd + output_usd;

    Ok(CostBreakdown {
        model: input.model,
        profile_id: profile.id.to_owned(),
        uncached_input_usd,
        cached_input_usd,
        cache_write_usd,
        output_usd,
        total_usd,
        long_context_applied: long_context_rule.is_some(),
        pricing_effective_date: PRICING_EFFECTIVE_DATE,
    })
}

fn cost(tokens: u64, usd_per_million: f64, multiplier: f64) -> f64 {
    tokens as f64 / 1_000_000.0 * usd_per_million * multiplier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::catalog::CATALOG;
    use crate::pricing::model::ApiEquivalentCostAvailability;

    fn input(model: &str) -> TokenCostInput {
        TokenCostInput {
            model: model.to_owned(),
            uncached_input_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            output_tokens: 0,
        }
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 0.000_000_001,
            "{actual} != {expected}"
        );
    }

    #[test]
    fn catalog_contains_every_current_profile() {
        let ids: Vec<_> = CATALOG.iter().map(|profile| profile.id).collect();
        assert_eq!(
            ids,
            vec![
                "gpt-5.6-sol",
                "gpt-5.6-terra",
                "gpt-5.6-luna",
                "gpt-5.5",
                "gpt-5.4",
                "gpt-5.4-mini",
                "gpt-5.3-codex",
                "gpt-5.2"
            ]
        );
    }

    #[test]
    fn catalog_prices_and_long_context_rules_match_the_current_snapshot() {
        let expected = [
            ("gpt-5.6-sol", 5.0, 0.5, Some(6.25), 30.0, true),
            ("gpt-5.6-terra", 2.5, 0.25, Some(3.125), 15.0, true),
            ("gpt-5.6-luna", 1.0, 0.1, Some(1.25), 6.0, true),
            ("gpt-5.5", 5.0, 0.5, None, 30.0, false),
            ("gpt-5.4", 2.5, 0.25, None, 15.0, false),
            ("gpt-5.4-mini", 0.75, 0.075, None, 4.5, false),
            ("gpt-5.3-codex", 1.75, 0.175, None, 14.0, false),
            ("gpt-5.2", 1.75, 0.175, None, 14.0, false),
        ];

        for (id, input, cached, cache_write, output, has_long_context) in expected {
            let profile = CATALOG.iter().find(|profile| profile.id == id).unwrap();
            assert_eq!(profile.uncached_input_usd_per_million, input);
            assert_eq!(profile.cached_input_usd_per_million, cached);
            assert_eq!(profile.cache_write_usd_per_million, cache_write);
            assert_eq!(profile.output_usd_per_million, output);
            assert_eq!(profile.long_context.is_some(), has_long_context);
            if let Some(rule) = profile.long_context {
                assert_eq!(rule.threshold_tokens, 272_000);
                assert_eq!(rule.input_multiplier, 2.0);
                assert_eq!(rule.output_multiplier, 1.5);
            }
        }
    }

    #[test]
    fn exact_and_snapshot_aliases_resolve_without_substring_matching() {
        assert_eq!(
            crate::pricing::catalog::resolve_profile("gpt-5.6"),
            crate::pricing::catalog::resolve_profile("gpt-5.6-sol")
        );
        assert_eq!(
            crate::pricing::catalog::resolve_profile("gpt-5.6-sol-2026-08-17"),
            crate::pricing::catalog::resolve_profile("gpt-5.6-sol")
        );
        assert_eq!(
            crate::pricing::catalog::resolve_profile("gpt-5.4-mini-2026-08-17"),
            crate::pricing::catalog::resolve_profile("gpt-5.4-mini")
        );
        assert!(crate::pricing::catalog::resolve_profile("my-gpt-5.6-copy").is_none());
        assert!(crate::pricing::catalog::resolve_profile("gpt-5.6-sol-2026-8-17").is_none());
    }

    #[test]
    fn prices_all_token_categories_and_sums_them() {
        let mut input = input("gpt-5.6-sol");
        input.uncached_input_tokens = 1_000;
        input.cached_input_tokens = 2_000;
        input.cache_write_input_tokens = 3_000;
        input.output_tokens = 4_000;

        let result = calculate_api_equivalent_cost(input).unwrap();
        close(result.uncached_input_usd, 0.005);
        close(result.cached_input_usd, 0.001);
        close(result.cache_write_usd, 0.01875);
        close(result.output_usd, 0.12);
        close(result.total_usd, 0.14475);
        assert!(!result.long_context_applied);
        assert_eq!(result.pricing_effective_date, "2026-08-17");
    }

    #[test]
    fn zero_tokens_are_free() {
        let result = calculate_api_equivalent_cost(input("gpt-5.6-luna")).unwrap();
        close(result.total_usd, 0.0);
        assert!(!result.long_context_applied);
    }

    #[test]
    fn unknown_models_are_unavailable_without_fallback() {
        let error = calculate_api_equivalent_cost(input("gpt-5.6-unknown")).unwrap_err();
        assert_eq!(error, PricingError::PricingUnavailable);
    }

    #[test]
    fn cache_write_is_supported_for_5_6_profiles() {
        let mut input = input("gpt-5.6-terra");
        input.cache_write_input_tokens = 1_000;

        let result = calculate_api_equivalent_cost(input).unwrap();
        close(result.cache_write_usd, 0.003125);
    }

    #[test]
    fn unsupported_cache_write_fails_only_when_non_zero() {
        let mut input = input("gpt-5.5");
        assert!(calculate_api_equivalent_cost(input.clone()).is_ok());

        input.cache_write_input_tokens = 1;
        assert_eq!(
            calculate_api_equivalent_cost(input).unwrap_err(),
            PricingError::UnsupportedPricingComponent
        );
    }

    #[test]
    fn long_context_boundary_is_strict_and_applies_to_all_categories() {
        let mut boundary = input("gpt-5.6-sol");
        boundary.uncached_input_tokens = 272_000;
        boundary.output_tokens = 1_000_000;
        let boundary_result = calculate_api_equivalent_cost(boundary.clone()).unwrap();
        assert!(!boundary_result.long_context_applied);
        close(boundary_result.uncached_input_usd, 1.36);
        close(boundary_result.output_usd, 30.0);

        let mut over = boundary.clone();
        over.uncached_input_tokens = 272_001;
        over.cached_input_tokens = 1;
        over.cache_write_input_tokens = 1;
        let over_result = calculate_api_equivalent_cost(over).unwrap();
        assert!(over_result.long_context_applied);
        close(over_result.uncached_input_usd, 2.72001);
        close(over_result.cached_input_usd, 0.000001);
        close(over_result.cache_write_usd, 0.0000125);
        close(over_result.output_usd, 45.0);
    }

    #[test]
    fn large_values_remain_finite() {
        let mut input = input("gpt-5.6-luna");
        input.uncached_input_tokens = 9_000_000_000;
        input.output_tokens = 2_000_000_000;

        let result = calculate_api_equivalent_cost(input).unwrap();
        assert!(result.total_usd.is_finite());
        close(result.total_usd, 36_000.0);
    }

    #[test]
    fn availability_has_a_non_cost_state_for_live_usage() {
        let availability = ApiEquivalentCostAvailability::MissingTokenBreakdown;
        assert_eq!(
            availability,
            ApiEquivalentCostAvailability::MissingTokenBreakdown
        );
    }
}
