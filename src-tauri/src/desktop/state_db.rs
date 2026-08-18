use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};

use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateDbSchema {
    pub(crate) columns: HashSet<String>,
    pub(crate) compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateThreadRecord {
    pub(crate) id: String,
    pub(crate) rollout_path: Option<String>,
    pub(crate) updated_at: i64,
    pub(crate) cwd: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) tokens_used: Option<i64>,
    pub(crate) cli_version: Option<String>,
}

pub(crate) struct StateDbReader {
    pool: SqlitePool,
    codex_home: PathBuf,
    pub(crate) schema: StateDbSchema,
}

impl StateDbReader {
    pub(crate) async fn open(path: &Path, codex_home: &Path) -> Result<Self, AppError> {
        let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
        let options = SqliteConnectOptions::from_str(&url)?
            .read_only(true)
            .create_if_missing(false)
            .busy_timeout(Duration::from_millis(400))
            .pragma("query_only", "ON");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let schema = inspect_schema(&pool).await?;
        Ok(Self {
            pool,
            codex_home: codex_home.to_path_buf(),
            schema,
        })
    }

    pub(crate) async fn threads(
        &self,
        max_rows: usize,
    ) -> Result<Vec<StateThreadRecord>, AppError> {
        if !self.schema.compatible {
            return Ok(Vec::new());
        }
        let columns = |name: &str| self.schema.columns.contains(name);
        let optional = |name: &str| {
            if columns(name) {
                name.to_owned()
            } else {
                "NULL".to_owned()
            }
        };
        let limit = i64::try_from(max_rows).unwrap_or(10_000);
        let sql = format!(
            "SELECT id, rollout_path, updated_at, {}, {}, {}, {} FROM threads ORDER BY updated_at DESC LIMIT ?",
            optional("cwd"), optional("model"), optional("tokens_used"), optional("cli_version")
        );
        let rows = sqlx::query(&sql).bind(limit).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StateThreadRecord {
                    id: row.try_get("id")?,
                    rollout_path: row.try_get("rollout_path")?,
                    updated_at: row.try_get("updated_at")?,
                    cwd: row.try_get(3)?,
                    model: row.try_get(4)?,
                    tokens_used: row.try_get(5)?,
                    cli_version: row.try_get(6)?,
                })
            })
            .collect()
    }

    pub(crate) fn resolve_rollout_path(&self, path: &str) -> PathBuf {
        let candidate = PathBuf::from(path);
        if candidate.is_absolute() {
            candidate
        } else {
            self.codex_home.join(path)
        }
    }
}

async fn inspect_schema(pool: &SqlitePool) -> Result<StateDbSchema, AppError> {
    let rows = sqlx::query("PRAGMA table_info(threads)")
        .fetch_all(pool)
        .await?;
    let columns = rows
        .into_iter()
        .filter_map(|row| row.try_get::<String, _>("name").ok())
        .collect::<HashSet<_>>();
    let compatible = ["id", "rollout_path", "updated_at"]
        .into_iter()
        .all(|required| columns.contains(required));
    Ok(StateDbSchema {
        columns,
        compatible,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn required_schema_is_detected_without_assuming_optional_columns() {
        let dir = tempfile::tempdir().expect("temp directory");
        let path = dir.path().join("state_6.sqlite");
        let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::from_str(&url)
                    .expect("sqlite options")
                    .create_if_missing(true),
            )
            .await
            .expect("sqlite should connect");
        sqlx::query("CREATE TABLE threads (id TEXT, rollout_path TEXT, updated_at INTEGER, cwd TEXT, model TEXT, tokens_used INTEGER, cli_version TEXT)")
            .execute(&pool)
            .await
            .expect("threads table should be created");
        sqlx::query("INSERT INTO threads VALUES ('thread-1', 'sessions/rollout-1.jsonl', 7, 'C:\\Demo', 'gpt-5.6-luna', 42, '1.0')")
            .execute(&pool)
            .await
            .expect("thread should be inserted");
        let reader = StateDbReader::open(&path, dir.path())
            .await
            .expect("reader should be read-only openable");
        assert!(reader.schema.compatible);
        assert_eq!(
            reader.threads(10).await.expect("threads should load")[0].cwd,
            Some("C:\\Demo".to_owned())
        );
    }

    #[tokio::test]
    async fn missing_required_schema_is_unavailable_not_a_panic() {
        let dir = tempfile::tempdir().expect("temp directory");
        let path = dir.path().join("state_5.sqlite");
        let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::from_str(&url)
                    .expect("sqlite options")
                    .create_if_missing(true),
            )
            .await
            .expect("sqlite should connect");
        sqlx::query("CREATE TABLE threads (id TEXT, updated_at INTEGER)")
            .execute(&pool)
            .await
            .expect("threads table should be created");
        let reader = StateDbReader::open(&path, dir.path())
            .await
            .expect("reader should open");
        assert!(!reader.schema.compatible);
        assert!(reader
            .threads(10)
            .await
            .expect("incompatible db should be skipped")
            .is_empty());
    }

    #[tokio::test]
    async fn external_database_is_read_only_and_query_only() {
        let dir = tempfile::tempdir().expect("temp directory");
        let path = dir.path().join("state_5.sqlite");
        let url = format!("sqlite://{}", path.to_string_lossy().replace('\\', "/"));
        let pool = SqlitePoolOptions::new()
            .connect_with(
                SqliteConnectOptions::from_str(&url)
                    .expect("sqlite options")
                    .create_if_missing(true),
            )
            .await
            .expect("sqlite should connect");
        sqlx::query("CREATE TABLE threads (id TEXT, rollout_path TEXT, updated_at INTEGER)")
            .execute(&pool)
            .await
            .expect("threads table should be created");
        pool.close().await;

        let reader = StateDbReader::open(&path, dir.path())
            .await
            .expect("reader should open");
        let write = sqlx::query("CREATE TABLE should_not_exist (id INTEGER)")
            .execute(&reader.pool)
            .await;
        assert!(write.is_err());
    }
}
