use std::path::PathBuf;

use sqlx::SqlitePool;

use crate::database::Database;

pub struct AppState {
    pub db_pool: SqlitePool,
    pub database_path: PathBuf,
}

impl AppState {
    pub fn from_database(database: Database) -> Self {
        Self {
            db_pool: database.pool,
            database_path: database.path,
        }
    }
}
