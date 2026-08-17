use std::sync::Arc;

use crate::{
    account::unix_timestamp,
    burn_rate::{BurnRateEstimate, BurnRateService, BurnRateStatus},
    rate_limit::{RateLimitService, RateLimitStatus, RateLimitWindow},
};

use super::model::{PredictionConfidence, QuotaPrediction, QuotaPredictionOutcome};

const ESTIMATED_TRUST: &str = "estimated";
const STABLE_BURN_RATE: f64 = 0.01;

pub(crate) struct QuotaPredictionService {
    rate_limit_service: Arc<RateLimitService>,
    burn_rate_service: Arc<BurnRateService>,
}

impl QuotaPredictionService {
    pub(crate) fn new(
        rate_limit_service: Arc<RateLimitService>,
        burn_rate_service: Arc<BurnRateService>,
    ) -> Self {
        Self {
            rate_limit_service,
            burn_rate_service,
        }
    }

    pub(crate) async fn get_predictions(
        &self,
        force_rate_limit_refresh: bool,
    ) -> Vec<QuotaPrediction> {
        let info = self
            .rate_limit_service
            .get_rate_limits(force_rate_limit_refresh)
            .await;
        if info.status != RateLimitStatus::Available {
            return vec![service_status_prediction(
                match info.status {
                    RateLimitStatus::Unavailable => QuotaPredictionOutcome::Unavailable,
                    RateLimitStatus::Error => QuotaPredictionOutcome::Error,
                    RateLimitStatus::Available => QuotaPredictionOutcome::InsufficientData,
                },
                info.message,
            )];
        }

        let burn_rates = self.burn_rate_service.get_burn_rates(false).await;
        info.windows
            .iter()
            .map(|window| {
                let burn_rate = burn_rates.iter().find(|estimate| {
                    estimate.limit_id == window.limit_id
                        && estimate.window_kind == Some(window.window_kind)
                        && estimate.window_duration_mins == window.window_duration_mins
                        && estimate.resets_at == window.resets_at
                });
                calculate_prediction(window, burn_rate, info.updated_at)
            })
            .collect()
    }
}

pub(crate) fn calculate_prediction(
    window: &RateLimitWindow,
    burn_rate: Option<&BurnRateEstimate>,
    calculated_at: i64,
) -> QuotaPrediction {
    let mut prediction = base_prediction(window, calculated_at);
    if window.limit_id.is_none() || !window.used_percent.is_finite() {
        prediction.message =
            Some("Official rate-limit identity or usage is unavailable.".to_owned());
        return prediction;
    }

    prediction.used_percent = Some(window.used_percent);
    if window.used_percent >= 100.0 {
        prediction.outcome = QuotaPredictionOutcome::AlreadyDepleted;
        prediction.message = Some("Quota currently exhausted".to_owned());
        return prediction;
    }

    let Some(burn_rate) = burn_rate else {
        prediction.message = Some("Insufficient data".to_owned());
        return prediction;
    };
    if burn_rate.status != BurnRateStatus::Available {
        prediction.outcome = match burn_rate.status {
            BurnRateStatus::Unavailable => QuotaPredictionOutcome::Unavailable,
            BurnRateStatus::Error => QuotaPredictionOutcome::Error,
            BurnRateStatus::InsufficientData => QuotaPredictionOutcome::InsufficientData,
            BurnRateStatus::Available => QuotaPredictionOutcome::InsufficientData,
        };
        prediction.message = Some("Insufficient data".to_owned());
        return prediction;
    }

    let Some(burn_rate_value) = burn_rate.burn_rate_percent_points_per_hour else {
        prediction.message = Some("Insufficient data".to_owned());
        return prediction;
    };
    if !burn_rate_value.is_finite() {
        prediction.outcome = QuotaPredictionOutcome::Error;
        prediction.message = Some("Burn rate is not finite.".to_owned());
        return prediction;
    }

    prediction.burn_rate_percent_points_per_hour = Some(burn_rate_value);
    prediction.confidence = confidence_for(burn_rate);
    if burn_rate_value <= STABLE_BURN_RATE {
        prediction.outcome = QuotaPredictionOutcome::Stable;
        prediction.message = Some("No depletion projected from current burn".to_owned());
        return prediction;
    }

    let remaining = (100.0 - window.used_percent).max(0.0);
    let seconds_to_depletion = remaining / burn_rate_value * 3600.0;
    if !seconds_to_depletion.is_finite() || seconds_to_depletion < 0.0 {
        prediction.outcome = QuotaPredictionOutcome::Error;
        prediction.message = Some("Depletion estimate is not finite.".to_owned());
        return prediction;
    }

    let estimated_depletion_at = calculated_at.saturating_add(seconds_to_depletion.round() as i64);
    prediction.seconds_to_depletion = Some(seconds_to_depletion);
    prediction.estimated_depletion_at = Some(estimated_depletion_at);
    prediction.outcome = match window.resets_at {
        Some(resets_at) if estimated_depletion_at < resets_at => {
            QuotaPredictionOutcome::DepletionBeforeReset
        }
        Some(_) => QuotaPredictionOutcome::ResetBeforeDepletion,
        None => QuotaPredictionOutcome::ResetUnknown,
    };
    prediction.message = Some(
        match prediction.outcome {
            QuotaPredictionOutcome::DepletionBeforeReset => "Estimated depletion before reset",
            QuotaPredictionOutcome::ResetBeforeDepletion => "Reset likely before depletion",
            QuotaPredictionOutcome::ResetUnknown => "Reset time is unknown",
            _ => "Insufficient data",
        }
        .to_owned(),
    );
    prediction
}

fn confidence_for(burn_rate: &BurnRateEstimate) -> PredictionConfidence {
    let Some(duration_mins) = burn_rate.window_duration_mins else {
        return PredictionConfidence::Low;
    };
    if duration_mins <= 0 {
        return PredictionConfidence::Low;
    }
    let observed_fraction = burn_rate
        .observed_span_sec
        .map(|span| span as f64 / (duration_mins as f64 * 60.0))
        .unwrap_or(0.0);
    if burn_rate.sample_count >= 6 && observed_fraction >= 0.20 {
        PredictionConfidence::High
    } else if burn_rate.sample_count >= 3 && observed_fraction >= 0.05 {
        PredictionConfidence::Medium
    } else {
        PredictionConfidence::Low
    }
}

fn base_prediction(window: &RateLimitWindow, calculated_at: i64) -> QuotaPrediction {
    QuotaPrediction {
        outcome: QuotaPredictionOutcome::InsufficientData,
        limit_id: window.limit_id.clone(),
        limit_name: window.limit_name.clone(),
        window_kind: Some(window.window_kind),
        window_duration_mins: window.window_duration_mins,
        used_percent: None,
        burn_rate_percent_points_per_hour: None,
        estimated_depletion_at: None,
        seconds_to_depletion: None,
        resets_at: window.resets_at,
        confidence: PredictionConfidence::Low,
        trust_class: ESTIMATED_TRUST.to_owned(),
        calculated_at,
        message: None,
    }
}

fn service_status_prediction(
    outcome: QuotaPredictionOutcome,
    message: Option<String>,
) -> QuotaPrediction {
    QuotaPrediction {
        outcome,
        limit_id: None,
        limit_name: None,
        window_kind: None,
        window_duration_mins: None,
        used_percent: None,
        burn_rate_percent_points_per_hour: None,
        estimated_depletion_at: None,
        seconds_to_depletion: None,
        resets_at: None,
        confidence: PredictionConfidence::Low,
        trust_class: ESTIMATED_TRUST.to_owned(),
        calculated_at: unix_timestamp(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        burn_rate::{BurnRateEstimate, BurnRateStatus},
        rate_limit::RateLimitWindowKind,
    };

    fn window(used_percent: f64, resets_at: Option<i64>) -> RateLimitWindow {
        RateLimitWindow {
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: RateLimitWindowKind::Primary,
            used_percent,
            remaining_percent: 100.0 - used_percent,
            window_duration_mins: Some(300),
            resets_at,
            plan_type: None,
            rate_limit_reached_type: None,
        }
    }

    fn burn(rate: f64, sample_count: usize, observed_span_sec: i64) -> BurnRateEstimate {
        BurnRateEstimate {
            status: BurnRateStatus::Available,
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: Some(RateLimitWindowKind::Primary),
            window_duration_mins: Some(300),
            resets_at: Some(10_000),
            latest_used_percent: Some(80.0),
            burn_rate_percent_points_per_hour: Some(rate),
            sample_count,
            observed_span_sec: Some(observed_span_sec),
            used_delta_percent: Some(10.0),
            first_observed_at: Some(1),
            last_observed_at: Some(1 + observed_span_sec),
            trust_class: "estimated".to_owned(),
            message: None,
        }
    }

    #[test]
    fn predicts_two_hours_from_twenty_percent_remaining() {
        let prediction = calculate_prediction(
            &window(80.0, Some(10_000)),
            Some(&burn(10.0, 3, 600)),
            1_000,
        );
        assert_eq!(
            prediction.outcome,
            QuotaPredictionOutcome::DepletionBeforeReset
        );
        assert_eq!(prediction.seconds_to_depletion, Some(7_200.0));
        assert_eq!(prediction.estimated_depletion_at, Some(8_200));
        assert_eq!(prediction.confidence, PredictionConfidence::Low);
    }

    #[test]
    fn covers_reset_stable_exhausted_and_unknown_cases() {
        assert_eq!(
            calculate_prediction(&window(80.0, Some(7_200)), Some(&burn(10.0, 3, 900)), 0).outcome,
            QuotaPredictionOutcome::ResetBeforeDepletion
        );
        assert_eq!(
            calculate_prediction(&window(80.0, Some(7_201)), Some(&burn(10.0, 3, 900)), 0).outcome,
            QuotaPredictionOutcome::DepletionBeforeReset
        );
        assert_eq!(
            calculate_prediction(&window(100.0, Some(10_000)), None, 0).outcome,
            QuotaPredictionOutcome::AlreadyDepleted
        );
        assert_eq!(
            calculate_prediction(&window(80.0, None), Some(&burn(10.0, 3, 900)), 0).outcome,
            QuotaPredictionOutcome::ResetUnknown
        );
        assert_eq!(
            calculate_prediction(&window(80.0, Some(10_000)), Some(&burn(0.01, 3, 900)), 0).outcome,
            QuotaPredictionOutcome::Stable
        );
    }

    #[test]
    fn calculates_confidence_without_calling_it_accurate() {
        assert_eq!(
            confidence_for(&burn(1.0, 3, 600)),
            PredictionConfidence::Low
        );
        assert_eq!(
            confidence_for(&burn(1.0, 3, 1_000)),
            PredictionConfidence::Medium
        );
        assert_eq!(
            confidence_for(&burn(1.0, 6, 3_600)),
            PredictionConfidence::High
        );
    }
}
