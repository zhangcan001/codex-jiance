use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    alerts::AlertService,
    burn_rate::BurnRateService,
    database::Database,
    desktop::{DesktopRateLimitService, DesktopRepository, DesktopService},
    history::HistoryService,
    model_usage::ModelUsageService,
    prediction::QuotaPredictionService,
    project::ProjectService,
    settings::SettingsService,
};

pub struct AppState {
    pub db_pool: SqlitePool,
    pub database_path: PathBuf,
    pub desktop_service: Arc<DesktopService>,
    pub rate_limit_service: Arc<DesktopRateLimitService>,
    pub burn_rate_service: Arc<BurnRateService>,
    pub quota_prediction_service: Arc<QuotaPredictionService>,
    pub alert_service: Arc<AlertService>,
    pub project_service: Arc<ProjectService>,
    pub model_usage_service: Arc<ModelUsageService>,
    pub history_service: Arc<HistoryService>,
    pub settings_service: Arc<SettingsService>,
}

impl AppState {
    pub fn from_database(
        database: Database,
        app_handle: tauri::AppHandle,
        settings_service: Arc<SettingsService>,
    ) -> Self {
        let desktop_repository = Arc::new(DesktopRepository::new(database.pool.clone()));
        let rate_limit_service = DesktopRateLimitService::new(Arc::clone(&desktop_repository));
        let burn_rate_service = Arc::new(BurnRateService::new(
            Arc::clone(&rate_limit_service),
            Arc::clone(&desktop_repository),
        ));
        let quota_prediction_service = Arc::new(QuotaPredictionService::new(
            Arc::clone(&rate_limit_service),
            Arc::clone(&burn_rate_service),
        ));
        let alert_service = AlertService::new(
            app_handle,
            Arc::clone(&rate_limit_service),
            Arc::clone(&quota_prediction_service),
            Arc::clone(&settings_service),
        );
        let desktop_service = DesktopService::new(
            Arc::clone(&desktop_repository),
            Arc::clone(&rate_limit_service),
        );
        let project_service = Arc::new(ProjectService::new(Arc::clone(&desktop_repository)));
        let model_usage_service = Arc::new(ModelUsageService::new(Arc::clone(&desktop_repository)));
        let history_service = Arc::new(HistoryService::new(
            Arc::clone(&desktop_repository),
            Arc::clone(&project_service),
            Arc::clone(&model_usage_service),
        ));

        Self {
            db_pool: database.pool,
            database_path: database.path,
            desktop_service,
            rate_limit_service,
            burn_rate_service,
            quota_prediction_service,
            alert_service,
            project_service,
            model_usage_service,
            history_service,
            settings_service,
        }
    }
}
