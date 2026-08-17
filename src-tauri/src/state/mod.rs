use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    codex::app_server::{AppServerManager, SchemaCompatibilityService},
    database::Database,
};

pub struct AppState {
    pub db_pool: SqlitePool,
    pub database_path: PathBuf,
    pub app_server_manager: Arc<AppServerManager>,
    pub schema_compatibility_service: Arc<SchemaCompatibilityService>,
}

impl AppState {
    pub fn from_database(database: Database) -> Self {
        Self {
            db_pool: database.pool,
            database_path: database.path,
            app_server_manager: Arc::new(AppServerManager::new()),
            schema_compatibility_service: Arc::new(SchemaCompatibilityService::new()),
        }
    }
}
