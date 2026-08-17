use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::{account::unix_timestamp, error::AppError};

use super::wire::{ThreadTokenUsageUpdatedParams, TokenUsageBreakdownWire};

#[derive(Debug, Clone)]
pub(crate) struct ThreadMetadataRecord {
    pub(crate) thread_id: String,
    pub(crate) session_id: String,
    pub(crate) forked_from_id: Option<String>,
    pub(crate) parent_thread_id: Option<String>,
    pub(crate) cwd: String,
    pub(crate) project_key: String,
    pub(crate) project_name: String,
    pub(crate) model_provider: Option<String>,
    pub(crate) model_id: Option<String>,
    pub(crate) model_source: Option<String>,
    pub(crate) cli_version: Option<String>,
    pub(crate) source: Option<String>,
    pub(crate) thread_source: Option<String>,
    pub(crate) git_sha: Option<String>,
    pub(crate) git_branch: Option<String>,
    pub(crate) thread_name: Option<String>,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
    pub(crate) recency_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PersistedTokenSnapshot {
    pub(crate) inserted: bool,
    pub(crate) baseline_only: bool,
    pub(crate) delta_event: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenHistoryPoint {
    pub(crate) observed_at: i64,
    pub(crate) delta_total_tokens: u64,
    pub(crate) delta_input_tokens: u64,
    pub(crate) delta_cached_input_tokens: u64,
    pub(crate) delta_cache_write_input_tokens: u64,
    pub(crate) delta_output_tokens: u64,
    pub(crate) delta_reasoning_output_tokens: u64,
    pub(crate) project_key: Option<String>,
    pub(crate) model_id: Option<String>,
}

#[derive(Clone)]
pub(crate) struct ThreadUsageRepository {
    pub(crate) pool: SqlitePool,
}

impl ThreadUsageRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn upsert_metadata(
        &self,
        metadata: &ThreadMetadataRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO thread_metadata
             (thread_id, session_id, forked_from_id, parent_thread_id, cwd, project_key,
              project_name, model_provider, model_id, model_source, cli_version, source,
              thread_source, git_sha, git_branch, thread_name, created_at, updated_at,
              recency_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(thread_id) DO UPDATE SET
               session_id=excluded.session_id, forked_from_id=excluded.forked_from_id,
               parent_thread_id=excluded.parent_thread_id, cwd=excluded.cwd,
               project_key=excluded.project_key, project_name=excluded.project_name,
               model_provider=excluded.model_provider, model_id=COALESCE(excluded.model_id, thread_metadata.model_id),
               model_source=COALESCE(excluded.model_source, thread_metadata.model_source),
               cli_version=excluded.cli_version, source=excluded.source,
               thread_source=excluded.thread_source, git_sha=excluded.git_sha,
               git_branch=excluded.git_branch, thread_name=excluded.thread_name,
               created_at=excluded.created_at, updated_at=excluded.updated_at,
               recency_at=excluded.recency_at, last_seen_at=excluded.last_seen_at",
        )
        .bind(&metadata.thread_id)
        .bind(&metadata.session_id)
        .bind(&metadata.forked_from_id)
        .bind(&metadata.parent_thread_id)
        .bind(&metadata.cwd)
        .bind(&metadata.project_key)
        .bind(&metadata.project_name)
        .bind(&metadata.model_provider)
        .bind(&metadata.model_id)
        .bind(&metadata.model_source)
        .bind(&metadata.cli_version)
        .bind(&metadata.source)
        .bind(&metadata.thread_source)
        .bind(&metadata.git_sha)
        .bind(&metadata.git_branch)
        .bind(&metadata.thread_name)
        .bind(metadata.created_at)
        .bind(metadata.updated_at)
        .bind(metadata.recency_at)
        .bind(unix_timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn ensure_minimal_metadata(&self, thread_id: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT OR IGNORE INTO thread_metadata
             (thread_id, session_id, cwd, project_key, project_name, created_at,
              updated_at, last_seen_at)
             VALUES (?, ?, '', 'unknown', 'Unknown', ?, ?, ?)",
        )
        .bind(thread_id)
        .bind(thread_id)
        .bind(unix_timestamp())
        .bind(unix_timestamp())
        .bind(unix_timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn update_settings(
        &self,
        thread_id: &str,
        cwd: Option<&str>,
        model: Option<&str>,
        model_provider: Option<&str>,
    ) -> Result<(), AppError> {
        self.ensure_minimal_metadata(thread_id).await?;
        let (project_key, project_name) = cwd
            .map(normalize_project)
            .unwrap_or_else(|| ("unknown".to_owned(), "Unknown".to_owned()));
        sqlx::query(
            "UPDATE thread_metadata
             SET cwd=COALESCE(?, cwd), project_key=CASE WHEN ? IS NULL THEN project_key ELSE ? END,
                 project_name=CASE WHEN ? IS NULL THEN project_name ELSE ? END,
                 model_id=COALESCE(?, model_id), model_provider=COALESCE(?, model_provider),
                 model_source=CASE WHEN ? IS NULL THEN model_source ELSE 'threadSettings' END,
                 last_seen_at=? WHERE thread_id=?",
        )
        .bind(cwd)
        .bind(cwd)
        .bind(&project_key)
        .bind(cwd)
        .bind(&project_name)
        .bind(model)
        .bind(model_provider)
        .bind(model)
        .bind(unix_timestamp())
        .bind(thread_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn update_rerouted_model(
        &self,
        thread_id: &str,
        model: &str,
    ) -> Result<(), AppError> {
        self.ensure_minimal_metadata(thread_id).await?;
        sqlx::query(
            "UPDATE thread_metadata SET model_id=?, model_source='rerouted', last_seen_at=?
             WHERE thread_id=?",
        )
        .bind(model)
        .bind(unix_timestamp())
        .bind(thread_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn persist_token_event(
        &self,
        event: &ThreadTokenUsageUpdatedParams,
        observed_at: i64,
        model_override: Option<(&str, &str)>,
    ) -> Result<PersistedTokenSnapshot, AppError> {
        validate_breakdown(&event.token_usage.total)?;
        validate_breakdown(&event.token_usage.last)?;
        let context_window = event
            .token_usage
            .model_context_window
            .map(|value| non_negative(value, "model context window"))
            .transpose()?;

        let mut transaction = self.pool.begin().await?;
        sqlx::query(
            "INSERT OR IGNORE INTO thread_metadata
             (thread_id, session_id, cwd, project_key, project_name, created_at,
              updated_at, last_seen_at)
             VALUES (?, ?, '', 'unknown', 'Unknown', ?, ?, ?)",
        )
        .bind(&event.thread_id)
        .bind(&event.thread_id)
        .bind(observed_at)
        .bind(observed_at)
        .bind(observed_at)
        .execute(&mut *transaction)
        .await?;

        let metadata = sqlx::query(
            "SELECT project_key, project_name, model_id, model_source
             FROM thread_metadata WHERE thread_id=?",
        )
        .bind(&event.thread_id)
        .fetch_one(&mut *transaction)
        .await?;
        let project_key: String = metadata.try_get("project_key")?;
        let project_name: String = metadata.try_get("project_name")?;
        let metadata_model: Option<String> = metadata.try_get("model_id")?;
        let metadata_model_source: Option<String> = metadata.try_get("model_source")?;
        let model_id = model_override
            .map(|value| value.0)
            .or(metadata_model.as_deref());
        let model_source = model_override
            .map(|value| value.1)
            .or(metadata_model_source.as_deref());
        let fingerprint = fingerprint(event, &project_key, model_id, context_window)?;

        let duplicate: Option<i64> =
            sqlx::query_scalar("SELECT id FROM thread_token_snapshots WHERE fingerprint=? LIMIT 1")
                .bind(&fingerprint)
                .fetch_optional(&mut *transaction)
                .await?;
        if duplicate.is_some() {
            transaction.commit().await?;
            return Ok(PersistedTokenSnapshot {
                inserted: false,
                baseline_only: false,
                delta_event: false,
            });
        }

        let previous = sqlx::query(
            "SELECT total_tokens, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    output_tokens, reasoning_output_tokens
             FROM thread_token_snapshots WHERE thread_id=?
             ORDER BY observed_at DESC, id DESC LIMIT 1",
        )
        .bind(&event.thread_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let current = counts(&event.token_usage.total)?;
        let previous = previous
            .map(|row| {
                Ok::<_, AppError>([
                    non_negative(row.try_get("total_tokens")?, "stored total tokens")?,
                    non_negative(row.try_get("input_tokens")?, "stored input tokens")?,
                    non_negative(
                        row.try_get("cached_input_tokens")?,
                        "stored cached input tokens",
                    )?,
                    non_negative(
                        row.try_get("cache_write_input_tokens")?,
                        "stored cache write input tokens",
                    )?,
                    non_negative(row.try_get("output_tokens")?, "stored output tokens")?,
                    non_negative(
                        row.try_get("reasoning_output_tokens")?,
                        "stored reasoning output tokens",
                    )?,
                ])
            })
            .transpose()?;
        let (baseline_only, reset_detected, delta) = match previous {
            None => (true, false, None),
            Some(previous)
                if current
                    .iter()
                    .zip(previous)
                    .any(|(now, before)| *now < before) =>
            {
                (true, true, None)
            }
            Some(previous) => (
                false,
                false,
                Some(
                    current
                        .map(|now| now)
                        .into_iter()
                        .zip(previous)
                        .map(|(now, before)| now - before)
                        .collect::<Vec<_>>(),
                ),
            ),
        };

        let total = to_sql_counts(counts(&event.token_usage.total)?)?;
        let last = to_sql_counts(counts(&event.token_usage.last)?)?;
        let delta = delta
            .map(|values| {
                values
                    .into_iter()
                    .map(i64::try_from)
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()
            .map_err(|_| {
                AppError::InvalidState("Token delta exceeds SQLite integer range.".to_owned())
            })?;
        sqlx::query(
            "INSERT INTO thread_token_snapshots
             (thread_id, turn_id, observed_at, total_tokens, input_tokens, cached_input_tokens,
              cache_write_input_tokens, output_tokens, reasoning_output_tokens, last_total_tokens,
              last_input_tokens, last_cached_input_tokens, last_cache_write_input_tokens,
              last_output_tokens, last_reasoning_output_tokens, model_context_window, project_key,
              project_name, model_id, model_source, delta_total_tokens, delta_input_tokens,
              delta_cached_input_tokens, delta_cache_write_input_tokens, delta_output_tokens,
              delta_reasoning_output_tokens, baseline_only, reset_detected, fingerprint)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event.thread_id)
        .bind(&event.turn_id)
        .bind(observed_at)
        .bind(total[0]).bind(total[1]).bind(total[2]).bind(total[3]).bind(total[4]).bind(total[5])
        .bind(last[0]).bind(last[1]).bind(last[2]).bind(last[3]).bind(last[4]).bind(last[5])
        .bind(
            context_window
                .map(i64::try_from)
                .transpose()
                .map_err(|_| {
                    AppError::InvalidState(
                        "Model context window exceeds SQLite integer range.".to_owned(),
                    )
                })?,
        )
        .bind(&project_key).bind(&project_name).bind(model_id).bind(model_source)
        .bind(delta.as_ref().and_then(|v| v.first().copied()))
        .bind(delta.as_ref().and_then(|v| v.get(1).copied()))
        .bind(delta.as_ref().and_then(|v| v.get(2).copied()))
        .bind(delta.as_ref().and_then(|v| v.get(3).copied()))
        .bind(delta.as_ref().and_then(|v| v.get(4).copied()))
        .bind(delta.as_ref().and_then(|v| v.get(5).copied()))
        .bind(baseline_only as i64)
        .bind(reset_detected as i64)
        .bind(&fingerprint)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(PersistedTokenSnapshot {
            inserted: true,
            baseline_only,
            delta_event: delta.is_some(),
        })
    }

    pub(crate) async fn usage_counts(
        &self,
    ) -> Result<(usize, usize, usize, Option<i64>), AppError> {
        let snapshots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM thread_token_snapshots")
            .fetch_one(&self.pool)
            .await?;
        let threads: i64 =
            sqlx::query_scalar("SELECT COUNT(DISTINCT thread_id) FROM thread_token_snapshots")
                .fetch_one(&self.pool)
                .await?;
        let deltas: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM thread_token_snapshots WHERE delta_total_tokens IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        let latest: Option<i64> =
            sqlx::query_scalar("SELECT MAX(observed_at) FROM thread_token_snapshots")
                .fetch_one(&self.pool)
                .await?;
        Ok((
            threads as usize,
            snapshots as usize,
            deltas as usize,
            latest,
        ))
    }

    pub(crate) async fn history_points(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
        max_points: usize,
    ) -> Result<Vec<TokenHistoryPoint>, AppError> {
        let limit = i64::try_from(max_points.min(2_000)).unwrap_or(2_000);
        let rows = sqlx::query(
            "SELECT observed_at, delta_total_tokens, delta_input_tokens,
                    delta_cached_input_tokens, delta_cache_write_input_tokens,
                    delta_output_tokens, delta_reasoning_output_tokens, project_key, model_id
             FROM (
                 SELECT observed_at, id, delta_total_tokens, delta_input_tokens,
                        delta_cached_input_tokens, delta_cache_write_input_tokens,
                        delta_output_tokens, delta_reasoning_output_tokens,
                        project_key, model_id
                 FROM thread_token_snapshots
                 WHERE delta_total_tokens IS NOT NULL
                   AND (? IS NULL OR observed_at >= ?)
                   AND (? IS NULL OR observed_at < ?)
                 ORDER BY observed_at DESC, id DESC
                 LIMIT ?
             )
             ORDER BY observed_at ASC, id ASC",
        )
        .bind(start_at)
        .bind(start_at)
        .bind(end_at)
        .bind(end_at)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(TokenHistoryPoint {
                    observed_at: row.try_get("observed_at")?,
                    delta_total_tokens: non_negative(
                        row.try_get("delta_total_tokens")?,
                        "delta total tokens",
                    )?,
                    delta_input_tokens: non_negative(
                        row.try_get("delta_input_tokens")?,
                        "delta input tokens",
                    )?,
                    delta_cached_input_tokens: non_negative(
                        row.try_get("delta_cached_input_tokens")?,
                        "delta cached input tokens",
                    )?,
                    delta_cache_write_input_tokens: non_negative(
                        row.try_get("delta_cache_write_input_tokens")?,
                        "delta cache write input tokens",
                    )?,
                    delta_output_tokens: non_negative(
                        row.try_get("delta_output_tokens")?,
                        "delta output tokens",
                    )?,
                    delta_reasoning_output_tokens: non_negative(
                        row.try_get("delta_reasoning_output_tokens")?,
                        "delta reasoning output tokens",
                    )?,
                    project_key: row.try_get("project_key")?,
                    model_id: row.try_get("model_id")?,
                })
            })
            .collect()
    }

    pub(crate) async fn history_coverage(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
    ) -> Result<(usize, usize, usize, usize, usize), AppError> {
        let row = sqlx::query(
            "SELECT COUNT(DISTINCT thread_id) AS observed_threads,
                    SUM(CASE WHEN delta_total_tokens IS NOT NULL THEN 1 ELSE 0 END) AS delta_events,
                    SUM(CASE WHEN baseline_only=1 THEN 1 ELSE 0 END) AS baseline_events,
                    SUM(CASE WHEN delta_total_tokens IS NOT NULL AND (project_key IS NULL OR project_key='unknown') THEN 1 ELSE 0 END) AS unknown_projects,
                    SUM(CASE WHEN delta_total_tokens IS NOT NULL AND model_id IS NULL THEN 1 ELSE 0 END) AS unknown_models
             FROM thread_token_snapshots
             WHERE (? IS NULL OR observed_at >= ?)
               AND (? IS NULL OR observed_at < ?)",
        )
        .bind(start_at)
        .bind(start_at)
        .bind(end_at)
        .bind(end_at)
        .fetch_one(&self.pool)
        .await?;
        Ok((
            row.try_get::<i64, _>("observed_threads")? as usize,
            row.try_get::<Option<i64>, _>("delta_events")?.unwrap_or(0) as usize,
            row.try_get::<Option<i64>, _>("baseline_events")?
                .unwrap_or(0) as usize,
            row.try_get::<Option<i64>, _>("unknown_projects")?
                .unwrap_or(0) as usize,
            row.try_get::<Option<i64>, _>("unknown_models")?
                .unwrap_or(0) as usize,
        ))
    }
}

fn validate_breakdown(breakdown: &TokenUsageBreakdownWire) -> Result<(), AppError> {
    for (value, name) in [
        (breakdown.total_tokens, "total tokens"),
        (breakdown.input_tokens, "input tokens"),
        (breakdown.cached_input_tokens, "cached input tokens"),
        (
            breakdown.cache_write_input_tokens,
            "cache write input tokens",
        ),
        (breakdown.output_tokens, "output tokens"),
        (breakdown.reasoning_output_tokens, "reasoning output tokens"),
    ] {
        non_negative(value, name)?;
    }
    Ok(())
}

fn non_negative(value: i64, name: &str) -> Result<u64, AppError> {
    u64::try_from(value)
        .map_err(|_| AppError::InvalidState(format!("{name} must be non-negative.")))
}

fn counts(breakdown: &TokenUsageBreakdownWire) -> Result<[u64; 6], AppError> {
    validate_breakdown(breakdown)?;
    Ok([
        breakdown.total_tokens as u64,
        breakdown.input_tokens as u64,
        breakdown.cached_input_tokens as u64,
        breakdown.cache_write_input_tokens as u64,
        breakdown.output_tokens as u64,
        breakdown.reasoning_output_tokens as u64,
    ])
}

fn to_sql_counts(values: [u64; 6]) -> Result<[i64; 6], AppError> {
    let values = values
        .map(i64::try_from)
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| {
            AppError::InvalidState("Token count exceeds SQLite integer range.".to_owned())
        })?;
    Ok([
        values[0], values[1], values[2], values[3], values[4], values[5],
    ])
}

#[derive(Serialize)]
struct Fingerprint<'a> {
    thread_id: &'a str,
    turn_id: &'a str,
    total: &'a TokenUsageBreakdownWire,
    last: &'a TokenUsageBreakdownWire,
    model_context_window: Option<i64>,
    project_key: &'a str,
    model_id: Option<&'a str>,
}

fn fingerprint(
    event: &ThreadTokenUsageUpdatedParams,
    project_key: &str,
    model_id: Option<&str>,
    context_window: Option<u64>,
) -> Result<String, AppError> {
    let value = serde_json::to_vec(&Fingerprint {
        thread_id: &event.thread_id,
        turn_id: &event.turn_id,
        total: &event.token_usage.total,
        last: &event.token_usage.last,
        model_context_window: context_window.map(|v| v as i64),
        project_key,
        model_id,
    })?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    Ok(format!("{hash:016x}"))
}

pub(crate) fn normalize_project(cwd: &str) -> (String, String) {
    let mut key = cwd.replace('/', "\\");
    while key.len() > 3 && key.ends_with('\\') {
        key.pop();
    }
    if key.as_bytes().get(1) == Some(&b':') {
        key = format!("{}{}", key[..1].to_ascii_uppercase(), &key[1..]);
    }
    let name = key
        .rsplit('\\')
        .find(|part| !part.is_empty())
        .unwrap_or("Unknown")
        .to_owned();
    if key.is_empty() {
        ("unknown".to_owned(), "Unknown".to_owned())
    } else {
        (key, name)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{normalize_project, ThreadUsageRepository};
    use crate::database::{connection::create_pool, migrations};
    use crate::thread_usage::wire::ThreadTokenUsageUpdatedParams;

    async fn repository() -> ThreadUsageRepository {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        migrations::run(&pool)
            .await
            .expect("database migration should complete");
        ThreadUsageRepository::new(pool)
    }

    fn event(total: [i64; 6], last: [i64; 6]) -> ThreadTokenUsageUpdatedParams {
        serde_json::from_value(json!({
            "threadId": "thread-1",
            "turnId": "turn-1",
            "tokenUsage": {
                "total": {
                    "totalTokens": total[0], "inputTokens": total[1],
                    "cachedInputTokens": total[2], "cacheWriteInputTokens": total[3],
                    "outputTokens": total[4], "reasoningOutputTokens": total[5]
                },
                "last": {
                    "totalTokens": last[0], "inputTokens": last[1],
                    "cachedInputTokens": last[2], "cacheWriteInputTokens": last[3],
                    "outputTokens": last[4], "reasoningOutputTokens": last[5]
                },
                "modelContextWindow": 128000,
                "futureField": "ignored"
            }
        }))
        .expect("fake token event should parse")
    }

    #[test]
    fn normalizes_windows_project_without_canonicalizing() {
        assert_eq!(
            normalize_project("c:/work/demo/"),
            ("C:\\work\\demo".to_owned(), "demo".to_owned())
        );
        assert_eq!(
            normalize_project(""),
            ("unknown".to_owned(), "Unknown".to_owned())
        );
    }

    #[tokio::test]
    async fn first_snapshot_is_baseline_and_second_snapshot_is_delta() {
        let repository = repository().await;
        let first = event([100, 80, 20, 5, 15, 3], [10, 8, 2, 1, 2, 1]);
        let second = event([160, 120, 30, 8, 25, 5], [12, 10, 2, 1, 3, 1]);

        let baseline = repository
            .persist_token_event(&first, 100, None)
            .await
            .expect("baseline should persist");
        assert!(baseline.baseline_only);
        assert!(!baseline.delta_event);

        let delta = repository
            .persist_token_event(&second, 110, None)
            .await
            .expect("delta should persist");
        assert!(!delta.baseline_only);
        assert!(delta.delta_event);

        let row = sqlx::query(
            "SELECT delta_total_tokens, delta_input_tokens, delta_cached_input_tokens,
                    delta_cache_write_input_tokens, delta_output_tokens,
                    delta_reasoning_output_tokens, model_context_window
             FROM thread_token_snapshots ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&repository.pool)
        .await
        .expect("delta row should exist");
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "delta_total_tokens").unwrap(),
            60
        );
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "delta_cached_input_tokens").unwrap(),
            10
        );
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "delta_cache_write_input_tokens").unwrap(),
            3
        );
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "delta_reasoning_output_tokens").unwrap(),
            2
        );
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "model_context_window").unwrap(),
            128000
        );
    }

    #[tokio::test]
    async fn identical_snapshot_is_deduplicated_and_reset_is_new_baseline() {
        let repository = repository().await;
        let first = event([100, 80, 20, 5, 15, 3], [0; 6]);
        let lower = event([90, 70, 19, 4, 14, 2], [0; 6]);
        assert!(
            repository
                .persist_token_event(&first, 100, None)
                .await
                .unwrap()
                .inserted
        );
        assert!(
            !repository
                .persist_token_event(&first, 101, None)
                .await
                .unwrap()
                .inserted
        );
        let reset = repository
            .persist_token_event(&lower, 102, None)
            .await
            .unwrap();
        assert!(reset.baseline_only);
        let row = sqlx::query("SELECT reset_detected, delta_total_tokens FROM thread_token_snapshots ORDER BY id DESC LIMIT 1")
            .fetch_one(&repository.pool).await.unwrap();
        assert_eq!(
            sqlx::Row::try_get::<i64, _>(&row, "reset_detected").unwrap(),
            1
        );
        assert!(
            sqlx::Row::try_get::<Option<i64>, _>(&row, "delta_total_tokens")
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn rejects_negative_tokens_and_keeps_foreign_key_without_preview_columns() {
        let repository = repository().await;
        let invalid = event([-1, 0, 0, 0, 0, 0], [0; 6]);
        assert!(repository
            .persist_token_event(&invalid, 100, None)
            .await
            .is_err());

        let columns = sqlx::query("PRAGMA table_info(thread_metadata)")
            .fetch_all(&repository.pool)
            .await
            .unwrap();
        let names = columns
            .iter()
            .map(|row| sqlx::Row::try_get::<String, _>(row, "name").unwrap())
            .collect::<Vec<_>>();
        assert!(!names
            .iter()
            .any(|name| name == "preview" || name == "prompt"));
        let fk_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_foreign_key_list('thread_token_snapshots')",
        )
        .fetch_one(&repository.pool)
        .await
        .unwrap();
        assert!(fk_count > 0);
    }

    #[tokio::test]
    async fn history_points_keep_the_latest_two_thousand_deltas_in_order() {
        let repository = repository().await;
        sqlx::query(
            "INSERT INTO thread_metadata
             (thread_id, session_id, cwd, project_key, project_name, created_at, updated_at, last_seen_at)
             VALUES ('thread-1', 'session-1', 'C:\\Projects\\Demo', 'C:\\Projects\\Demo', 'Demo', 1, 1, 1)",
        )
        .execute(&repository.pool)
        .await
        .expect("thread metadata should insert");

        for observed_at in 0..=2_000_i64 {
            sqlx::query(
                "INSERT INTO thread_token_snapshots
                 (thread_id, turn_id, observed_at, total_tokens, input_tokens,
                  cached_input_tokens, cache_write_input_tokens, output_tokens,
                  reasoning_output_tokens, last_total_tokens, last_input_tokens,
                  last_cached_input_tokens, last_cache_write_input_tokens, last_output_tokens,
                  last_reasoning_output_tokens, delta_total_tokens, delta_input_tokens,
                  delta_cached_input_tokens, delta_cache_write_input_tokens, delta_output_tokens,
                  delta_reasoning_output_tokens, fingerprint)
                 VALUES ('thread-1', ?, ?, 1, 1, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0,
                         1, 1, 0, 0, 0, 0, ?)",
            )
            .bind(format!("turn-{observed_at}"))
            .bind(observed_at)
            .bind(format!("fingerprint-{observed_at}"))
            .execute(&repository.pool)
            .await
            .expect("token history point should insert");
        }

        let points = repository
            .history_points(None, None, 2_000)
            .await
            .expect("token history should load");
        assert_eq!(points.len(), 2_000);
        assert_eq!(points.first().unwrap().observed_at, 1);
        assert_eq!(points.last().unwrap().observed_at, 2_000);
    }
}
