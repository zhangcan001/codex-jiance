use sqlx::{migrate::Migrator, SqlitePool};

use crate::error::AppError;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &SqlitePool) -> Result<(), AppError> {
    MIGRATOR.run(pool).await?;
    Ok(())
}

pub async fn get_schema_version(pool: &SqlitePool) -> Result<i64, AppError> {
    let version: Option<i64> = sqlx::query_scalar("SELECT MAX(version) FROM schema_info")
        .fetch_one(pool)
        .await?;

    version.ok_or_else(|| AppError::InvalidState("Database schema version is missing.".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::connection::create_pool;

    #[tokio::test]
    async fn migration_runs_against_a_temporary_database() {
        let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
        let database_path = temp_dir.path().join("database.db");
        let database_url = format!("sqlite://{}", database_path.display());

        let pool = create_pool(&database_url)
            .await
            .expect("temporary database should connect");
        run(&pool).await.expect("migration should complete");

        assert!(database_path.exists());
    }

    #[tokio::test]
    async fn schema_version_is_four_after_migration() {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        run(&pool).await.expect("migration should complete");

        assert_eq!(get_schema_version(&pool).await.expect("schema exists"), 4);
    }

    #[tokio::test]
    async fn health_query_succeeds_after_migration() {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        run(&pool).await.expect("migration should complete");

        let value: i64 = sqlx::query_scalar("SELECT 1")
            .fetch_one(&pool)
            .await
            .expect("health query should succeed");

        assert_eq!(value, 1);
    }

    #[tokio::test]
    async fn migration_is_idempotent_on_second_initialization() {
        let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
        let database_path = temp_dir.path().join("database.db");
        let database_url = format!("sqlite://{}", database_path.display());

        let first_pool = create_pool(&database_url)
            .await
            .expect("first connection should succeed");
        run(&first_pool)
            .await
            .expect("first migration should complete");
        first_pool.close().await;

        let second_pool = create_pool(&database_url)
            .await
            .expect("second connection should succeed");
        run(&second_pool)
            .await
            .expect("second migration should remain valid");

        assert_eq!(
            get_schema_version(&second_pool)
                .await
                .expect("schema exists"),
            4
        );
    }
}
