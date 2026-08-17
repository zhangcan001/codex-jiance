pub mod connection;
pub mod migrations;

use std::path::{Path, PathBuf};

use log::info;
use sqlx::SqlitePool;

use crate::error::AppError;

pub struct Database {
    pub pool: SqlitePool,
    pub path: PathBuf,
}

pub async fn initialize(app_data_dir: &Path) -> Result<Database, AppError> {
    std::fs::create_dir_all(app_data_dir)?;

    let path = app_data_dir.join("codex-usage-monitor.db");
    info!("Database path resolved: {}", path.display());
    info!("Database initialization started");

    let database_url = sqlite_url(&path);
    let pool = connection::create_pool(&database_url).await?;
    migrations::run(&pool).await?;

    let schema_version = migrations::get_schema_version(&pool).await?;
    info!("Database migrations completed: schema v{schema_version}");
    info!("Database ready");

    Ok(Database { pool, path })
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"))
}
