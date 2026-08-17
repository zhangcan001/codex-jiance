use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    account::AccountService,
    burn_rate::BurnRateService,
    codex::app_server::{AppServerManager, SchemaCompatibilityService},
    database::Database,
    prediction::QuotaPredictionService,
    rate_limit::{RateLimitRepository, RateLimitService},
    usage::UsageService,
};

pub struct AppState {
    pub db_pool: SqlitePool,
    pub database_path: PathBuf,
    pub app_server_manager: Arc<AppServerManager>,
    pub schema_compatibility_service: Arc<SchemaCompatibilityService>,
    pub account_service: Arc<AccountService>,
    #[allow(dead_code)]
    pub rate_limit_repository: Arc<RateLimitRepository>,
    pub rate_limit_service: Arc<RateLimitService>,
    pub burn_rate_service: Arc<BurnRateService>,
    pub quota_prediction_service: Arc<QuotaPredictionService>,
    pub usage_service: Arc<UsageService>,
}

impl AppState {
    pub fn from_database(database: Database) -> Self {
        let app_server_manager = Arc::new(AppServerManager::new());
        let schema_compatibility_service = Arc::new(SchemaCompatibilityService::new());
        let account_service = Arc::new(AccountService::new(
            Arc::clone(&app_server_manager),
            Arc::clone(&schema_compatibility_service),
        ));
        let rate_limit_repository = Arc::new(RateLimitRepository::new(database.pool.clone()));
        let rate_limit_service = Arc::new(RateLimitService::new(
            Arc::clone(&app_server_manager),
            Arc::clone(&schema_compatibility_service),
            Arc::clone(&account_service),
            Arc::clone(&rate_limit_repository),
        ));
        let burn_rate_service = Arc::new(BurnRateService::new(
            Arc::clone(&rate_limit_service),
            Arc::clone(&rate_limit_repository),
        ));
        let quota_prediction_service = Arc::new(QuotaPredictionService::new(
            Arc::clone(&rate_limit_service),
            Arc::clone(&burn_rate_service),
        ));
        let usage_service = Arc::new(UsageService::new(
            Arc::clone(&app_server_manager),
            Arc::clone(&schema_compatibility_service),
            Arc::clone(&account_service),
        ));

        Self {
            db_pool: database.pool,
            database_path: database.path,
            app_server_manager,
            schema_compatibility_service,
            account_service,
            rate_limit_repository,
            rate_limit_service,
            burn_rate_service,
            quota_prediction_service,
            usage_service,
        }
    }
}
