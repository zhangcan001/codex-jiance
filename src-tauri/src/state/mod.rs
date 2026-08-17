use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    account::AccountService,
    codex::app_server::{AppServerManager, SchemaCompatibilityService},
    database::Database,
    rate_limit::{RateLimitRepository, RateLimitService},
};

pub struct AppState {
    pub db_pool: SqlitePool,
    pub database_path: PathBuf,
    pub app_server_manager: Arc<AppServerManager>,
    pub schema_compatibility_service: Arc<SchemaCompatibilityService>,
    pub account_service: Arc<AccountService>,
    pub rate_limit_service: Arc<RateLimitService>,
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
            rate_limit_repository,
        ));

        Self {
            db_pool: database.pool,
            database_path: database.path,
            app_server_manager,
            schema_compatibility_service,
            account_service,
            rate_limit_service,
        }
    }
}
