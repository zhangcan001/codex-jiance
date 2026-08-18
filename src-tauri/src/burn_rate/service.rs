use std::sync::Arc;

use crate::{
    desktop::{DesktopRateLimitService, DesktopRepository},
    rate_limit::{RateLimitHistorySample, RateLimitStatus, RateLimitWindow},
};

use super::model::{BurnRateEstimate, BurnRateStatus};

const MAX_HISTORY_SAMPLES: usize = 64;
const MIN_OBSERVED_SPAN_SEC: i64 = 60;
const ESTIMATED_TRUST: &str = "estimated";

pub(crate) struct BurnRateService {
    rate_limit_service: Arc<DesktopRateLimitService>,
    repository: Arc<DesktopRepository>,
}

impl BurnRateService {
    pub(crate) fn new(
        rate_limit_service: Arc<DesktopRateLimitService>,
        repository: Arc<DesktopRepository>,
    ) -> Self {
        Self {
            rate_limit_service,
            repository,
        }
    }

    pub(crate) async fn get_burn_rates(
        &self,
        force_rate_limit_refresh: bool,
    ) -> Vec<BurnRateEstimate> {
        let info = self
            .rate_limit_service
            .get_rate_limits(force_rate_limit_refresh)
            .await;
        if info.status != RateLimitStatus::Available {
            return vec![service_status_estimate(
                match info.status {
                    RateLimitStatus::Unavailable => BurnRateStatus::Unavailable,
                    RateLimitStatus::Error => BurnRateStatus::Error,
                    RateLimitStatus::Available => BurnRateStatus::InsufficientData,
                },
                info.message,
            )];
        }

        let mut estimates = Vec::with_capacity(info.windows.len());
        for window in info.windows {
            if window.limit_id.is_none() || window.resets_at.is_none() {
                estimates.push(calculate_burn_rate(&window, Vec::new(), info.updated_at));
                continue;
            }

            match self
                .repository
                .history_for_window(
                    window.limit_id.as_deref(),
                    window.window_kind,
                    window.window_duration_mins,
                    window.resets_at,
                    MAX_HISTORY_SAMPLES,
                )
                .await
            {
                Ok(history) => {
                    estimates.push(calculate_burn_rate(&window, history, info.updated_at))
                }
                Err(error) => estimates.push(error_estimate(&window, error.to_string())),
            }
        }
        estimates
    }
}

pub(crate) fn calculate_burn_rate(
    window: &RateLimitWindow,
    mut history: Vec<RateLimitHistorySample>,
    current_observed_at: i64,
) -> BurnRateEstimate {
    let mut estimate = base_estimate(window);
    if window.limit_id.is_none() {
        estimate.message = Some("额度窗口缺少官方 limit id。".to_owned());
        return estimate;
    }
    if window.resets_at.is_none() {
        estimate.message = Some("额度窗口缺少重置时间戳。".to_owned());
        return estimate;
    }
    if !window.used_percent.is_finite() {
        estimate.message = Some("官方已用百分比不是有限值。".to_owned());
        return estimate;
    }

    history.push(RateLimitHistorySample {
        captured_at: current_observed_at,
        used_percent: window.used_percent,
    });
    history.sort_by_key(|sample| sample.captured_at);

    let mut samples: Vec<RateLimitHistorySample> = Vec::with_capacity(history.len());
    for sample in history {
        if !sample.used_percent.is_finite() {
            continue;
        }
        if let Some(previous) = samples.last_mut() {
            if previous.captured_at == sample.captured_at {
                *previous = sample;
                continue;
            }
        }
        samples.push(sample);
    }

    estimate.sample_count = samples.len();
    estimate.latest_used_percent = samples.last().map(|sample| sample.used_percent);
    estimate.first_observed_at = samples.first().map(|sample| sample.captured_at);
    estimate.last_observed_at = samples.last().map(|sample| sample.captured_at);

    if samples.len() < 2 {
        estimate.message = Some("至少需要两条观测记录。".to_owned());
        return estimate;
    }

    let first = samples[0];
    let latest = samples[samples.len() - 1];
    let span = latest.captured_at - first.captured_at;
    estimate.observed_span_sec = Some(span);
    if span < MIN_OBSERVED_SPAN_SEC {
        estimate.message = Some("观测记录至少需要覆盖 60 秒。".to_owned());
        return estimate;
    }
    if span < 0 {
        estimate.message = Some("观测时间戳顺序无效。".to_owned());
        return estimate;
    }

    let used_delta = latest.used_percent - first.used_percent;
    estimate.used_delta_percent = Some(used_delta);
    if !used_delta.is_finite() {
        estimate.message = Some("已用百分比增量不是有限值。".to_owned());
        return estimate;
    }
    if used_delta < 0.0 {
        estimate.message = Some("当前重置周期内已用百分比出现下降。".to_owned());
        return estimate;
    }

    let burn_rate = used_delta / (span as f64 / 3600.0);
    if !burn_rate.is_finite() {
        estimate.message = Some("消耗速率不是有限值。".to_owned());
        return estimate;
    }

    estimate.status = BurnRateStatus::Available;
    estimate.burn_rate_percent_points_per_hour = Some(burn_rate);
    estimate.message = None;
    estimate
}

fn base_estimate(window: &RateLimitWindow) -> BurnRateEstimate {
    BurnRateEstimate {
        status: BurnRateStatus::InsufficientData,
        limit_id: window.limit_id.clone(),
        limit_name: window.limit_name.clone(),
        window_kind: Some(window.window_kind),
        window_duration_mins: window.window_duration_mins,
        resets_at: window.resets_at,
        latest_used_percent: Some(window.used_percent),
        burn_rate_percent_points_per_hour: None,
        sample_count: 0,
        observed_span_sec: None,
        used_delta_percent: None,
        first_observed_at: None,
        last_observed_at: None,
        trust_class: ESTIMATED_TRUST.to_owned(),
        message: None,
    }
}

fn service_status_estimate(status: BurnRateStatus, message: Option<String>) -> BurnRateEstimate {
    BurnRateEstimate {
        status,
        limit_id: None,
        limit_name: None,
        window_kind: None,
        window_duration_mins: None,
        resets_at: None,
        latest_used_percent: None,
        burn_rate_percent_points_per_hour: None,
        sample_count: 0,
        observed_span_sec: None,
        used_delta_percent: None,
        first_observed_at: None,
        last_observed_at: None,
        trust_class: ESTIMATED_TRUST.to_owned(),
        message,
    }
}

fn error_estimate(window: &RateLimitWindow, message: String) -> BurnRateEstimate {
    let mut estimate = base_estimate(window);
    estimate.status = BurnRateStatus::Error;
    estimate.message = Some(message);
    estimate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rate_limit::RateLimitWindowKind;

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

    fn sample(captured_at: i64, used_percent: f64) -> RateLimitHistorySample {
        RateLimitHistorySample {
            captured_at,
            used_percent,
        }
    }

    #[test]
    fn calculates_positive_and_zero_burn_rates() {
        let positive =
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 10.0)], 1_600);
        assert_eq!(positive.status, BurnRateStatus::Available);
        assert_eq!(positive.burn_rate_percent_points_per_hour, Some(60.0));

        let zero =
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 20.0)], 1_600);
        assert_eq!(zero.status, BurnRateStatus::Available);
        assert_eq!(zero.burn_rate_percent_points_per_hour, Some(0.0));
    }

    #[test]
    fn rejects_insufficient_decreasing_and_unknown_cycle_data() {
        assert_eq!(
            calculate_burn_rate(&window(20.0, Some(2_000)), Vec::new(), 1_600).status,
            BurnRateStatus::InsufficientData
        );
        assert_eq!(
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 30.0)], 1_600,)
                .status,
            BurnRateStatus::InsufficientData
        );
        assert_eq!(
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 30.0)], 1_030,)
                .status,
            BurnRateStatus::InsufficientData
        );
        assert_eq!(
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 30.0)], 1_600,)
                .status,
            BurnRateStatus::InsufficientData
        );
        assert_eq!(
            calculate_burn_rate(&window(20.0, Some(2_000)), vec![sample(1_000, 30.0)], 1_600,)
                .message
                .as_deref(),
            Some("当前重置周期内已用百分比出现下降。")
        );
        assert_eq!(
            calculate_burn_rate(&window(20.0, None), vec![sample(1_000, 10.0)], 1_600).status,
            BurnRateStatus::InsufficientData
        );
    }

    #[test]
    fn deduplicates_timestamp_and_keeps_current_observation() {
        let estimate = calculate_burn_rate(
            &window(30.0, Some(2_000)),
            vec![sample(1_000, 10.0), sample(1_000, 20.0)],
            1_600,
        );
        assert_eq!(estimate.status, BurnRateStatus::Available);
        assert_eq!(estimate.sample_count, 2);
        assert_eq!(estimate.used_delta_percent, Some(10.0));
        assert!(estimate
            .burn_rate_percent_points_per_hour
            .expect("burn rate should exist")
            .is_finite());
    }
}
