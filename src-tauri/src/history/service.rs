use std::sync::Arc;

use crate::{
    desktop::DesktopRepository, error::AppError, model_usage::ModelUsageService,
    project::ProjectService,
};

use super::model::{
    HistoryCoverage, MonitoringHistory, RateLimitHistoryPoint,
    TokenHistoryPoint as PublicTokenHistoryPoint,
};

const MAX_HISTORY_POINTS: usize = 2_000;
const COVERAGE: &str = "Codex 桌面版本地会话";

pub(crate) struct HistoryService {
    rate_limit_repository: Arc<DesktopRepository>,
    project_service: Arc<ProjectService>,
    model_usage_service: Arc<ModelUsageService>,
}

impl HistoryService {
    pub(crate) fn new(
        rate_limit_repository: Arc<DesktopRepository>,
        project_service: Arc<ProjectService>,
        model_usage_service: Arc<ModelUsageService>,
    ) -> Self {
        Self {
            rate_limit_repository,
            project_service,
            model_usage_service,
        }
    }

    pub(crate) async fn get_history(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<MonitoringHistory, AppError> {
        let rate_limit_series = self
            .rate_limit_repository
            .history_for_range(start_at, end_at, MAX_HISTORY_POINTS)
            .await?
            .into_iter()
            .map(|point| RateLimitHistoryPoint {
                captured_at: point.captured_at,
                limit_id: point.limit_id,
                kind: point.window_kind,
                duration: point.duration,
                used_percent: point.used_percent,
                resets_at: point.resets_at,
            })
            .collect();
        let token_series = self
            .rate_limit_repository
            .history_points(start_at, end_at, MAX_HISTORY_POINTS)
            .await?
            .into_iter()
            .map(public_token_point)
            .collect();
        let coverage_counts = self
            .rate_limit_repository
            .history_coverage(start_at, end_at)
            .await?;
        let projects = self.project_service.get_usage(start_at, end_at).await?;
        let models = self.model_usage_service.get_usage(start_at, end_at).await?;
        let coverage = HistoryCoverage {
            thread_usage: COVERAGE.to_owned(),
            observed_threads: coverage_counts.0,
            delta_events: coverage_counts.1,
            baseline_events: coverage_counts.2,
            unknown_project_events: coverage_counts.3,
            unknown_model_events: coverage_counts.4,
            pricing_coverage_percent: models.pricing_coverage_percent,
        };
        Ok(MonitoringHistory {
            rate_limit_series,
            token_series,
            project_summary: projects.projects,
            model_summary: models.models,
            coverage,
            start_at,
            end_at,
        })
    }
}

fn public_token_point(point: crate::desktop::DesktopTokenHistoryPoint) -> PublicTokenHistoryPoint {
    PublicTokenHistoryPoint {
        observed_at: point.observed_at,
        delta_total_tokens: point.delta_total_tokens,
        delta_input_tokens: point.delta_input_tokens,
        delta_cached_input_tokens: point.delta_cached_input_tokens,
        delta_cache_write_input_tokens: point.delta_cache_write_input_tokens,
        delta_output_tokens: point.delta_output_tokens,
        delta_reasoning_output_tokens: point.delta_reasoning_output_tokens,
        project_key: point.project_key,
        model_id: point.model_id,
    }
}
