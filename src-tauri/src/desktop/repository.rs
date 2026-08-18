use sqlx::{Row, SqlitePool};

use crate::{
    error::AppError,
    pricing::{calculate_api_equivalent_cost, TokenCostInput},
    rate_limit::{
        RateLimitHistorySample, RateLimitInfo, RateLimitStatus, RateLimitWindow,
        RateLimitWindowKind,
    },
};

use super::{
    model::DesktopRateLimitSnapshot,
    rollout::{RateLimitObservation, TokenCounts},
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CursorRecord {
    pub(crate) thread_id: Option<String>,
    pub(crate) byte_offset: u64,
    pub(crate) file_size: u64,
    pub(crate) modified_at: Option<i64>,
    pub(crate) last_event_at: Option<i64>,
    pub(crate) originator: Option<String>,
    pub(crate) is_desktop: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TokenPersistResult {
    pub(crate) inserted: bool,
    pub(crate) baseline_only: bool,
    pub(crate) delta_event: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DesktopCounts {
    pub(crate) indexed_sessions: usize,
    pub(crate) tracked_rollouts: usize,
    pub(crate) token_events: usize,
    pub(crate) delta_events: usize,
    pub(crate) baseline_only_events: usize,
    pub(crate) latest_event_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DesktopActivityTotals {
    pub(crate) observed_tokens: u64,
    pub(crate) today_tokens: u64,
    pub(crate) observed_threads: usize,
    pub(crate) observed_turns: usize,
    pub(crate) input_tokens: u64,
    pub(crate) cached_input_tokens: u64,
    pub(crate) cache_write_input_tokens: u64,
    pub(crate) output_tokens: u64,
    pub(crate) reasoning_output_tokens: u64,
    pub(crate) priced_events: usize,
    pub(crate) total_events: usize,
    pub(crate) api_equivalent_cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DesktopTokenHistoryPoint {
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DesktopRateLimitHistoryPoint {
    pub(crate) captured_at: i64,
    pub(crate) limit_id: Option<String>,
    pub(crate) window_kind: String,
    pub(crate) duration: Option<i64>,
    pub(crate) used_percent: f64,
    pub(crate) resets_at: Option<i64>,
}

#[derive(Clone)]
pub(crate) struct DesktopRepository {
    pub(crate) pool: SqlitePool,
}

impl DesktopRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn cursor(&self, path: &str) -> Result<CursorRecord, AppError> {
        let row = sqlx::query(
            "SELECT thread_id, byte_offset, file_size, modified_at, last_event_at, originator, is_desktop
             FROM desktop_rollout_cursors WHERE rollout_path=?",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(CursorRecord::default());
        };
        Ok(CursorRecord {
            thread_id: row.try_get("thread_id")?,
            byte_offset: non_negative_u64(row.try_get("byte_offset")?, "rollout cursor")?,
            file_size: non_negative_u64(row.try_get("file_size")?, "rollout file size")?,
            modified_at: row.try_get("modified_at")?,
            last_event_at: row.try_get("last_event_at")?,
            originator: row.try_get("originator")?,
            is_desktop: row.try_get::<i64, _>("is_desktop")? != 0,
        })
    }

    pub(crate) async fn save_cursor(
        &self,
        path: &str,
        cursor: &CursorRecord,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO desktop_rollout_cursors
             (rollout_path, thread_id, byte_offset, file_size, modified_at, last_event_at, originator, is_desktop, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(rollout_path) DO UPDATE SET
               thread_id=excluded.thread_id, byte_offset=excluded.byte_offset,
               file_size=excluded.file_size, modified_at=excluded.modified_at,
               last_event_at=excluded.last_event_at, originator=excluded.originator,
               is_desktop=excluded.is_desktop, updated_at=excluded.updated_at",
        )
        .bind(path)
        .bind(&cursor.thread_id)
        .bind(sqlite_i64(cursor.byte_offset, "rollout cursor")?)
        .bind(sqlite_i64(cursor.file_size, "rollout file size")?)
        .bind(cursor.modified_at)
        .bind(cursor.last_event_at)
        .bind(&cursor.originator)
        .bind(cursor.is_desktop as i64)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn upsert_thread(
        &self,
        thread_id: &str,
        cwd: Option<&str>,
        model: Option<&str>,
        cli_version: Option<&str>,
        originator: Option<&str>,
        event_at: i64,
    ) -> Result<(), AppError> {
        let cwd = cwd.unwrap_or("");
        let (project_key, project_name) = normalize_project(cwd);
        sqlx::query(
            "INSERT INTO thread_metadata
             (thread_id, session_id, cwd, project_key, project_name, model_id, model_source,
              cli_version, source, thread_source, created_at, updated_at, recency_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, 'turn_context', ?, 'desktop', ?, ?, ?, ?, ?)
             ON CONFLICT(thread_id) DO UPDATE SET
               cwd=CASE WHEN excluded.cwd='' THEN thread_metadata.cwd ELSE excluded.cwd END,
               project_key=CASE WHEN excluded.cwd='' THEN thread_metadata.project_key ELSE excluded.project_key END,
               project_name=CASE WHEN excluded.cwd='' THEN thread_metadata.project_name ELSE excluded.project_name END,
               model_id=COALESCE(excluded.model_id, thread_metadata.model_id),
               cli_version=COALESCE(excluded.cli_version, thread_metadata.cli_version),
               source='desktop', thread_source=COALESCE(excluded.thread_source, thread_metadata.thread_source),
               updated_at=MAX(thread_metadata.updated_at, excluded.updated_at),
               recency_at=MAX(COALESCE(thread_metadata.recency_at, 0), excluded.recency_at),
               last_seen_at=excluded.last_seen_at",
        )
        .bind(thread_id)
        .bind(thread_id)
        .bind(cwd)
        .bind(project_key)
        .bind(project_name)
        .bind(model)
        .bind(cli_version)
        .bind(originator)
        .bind(event_at)
        .bind(event_at)
        .bind(event_at)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub(crate) async fn thread_context(
        &self,
        thread_id: &str,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>, AppError> {
        let row = sqlx::query(
            "SELECT cwd, model_id, cli_version FROM thread_metadata
             WHERE thread_id=? AND source='desktop' LIMIT 1",
        )
        .bind(thread_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok((
                row.try_get("cwd")?,
                row.try_get("model_id")?,
                row.try_get("cli_version")?,
            ))
        })
        .transpose()
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn persist_token_event(
        &self,
        rollout_path: &str,
        thread_id: &str,
        turn_id: &str,
        observed_at: i64,
        total: &TokenCounts,
        last: &TokenCounts,
        model_context_window: Option<u64>,
        cwd: Option<&str>,
        model: Option<&str>,
        originator: Option<&str>,
        byte_offset: u64,
    ) -> Result<TokenPersistResult, AppError> {
        let current = total.values();
        let last_values = last.values();
        let mut transaction = self.pool.begin().await?;
        let fingerprint = fingerprint(
            rollout_path,
            thread_id,
            turn_id,
            observed_at,
            &current,
            byte_offset,
        );
        let duplicate: Option<i64> =
            sqlx::query_scalar("SELECT id FROM thread_token_snapshots WHERE fingerprint=? LIMIT 1")
                .bind(&fingerprint)
                .fetch_optional(&mut *transaction)
                .await?;
        if duplicate.is_some() {
            transaction.commit().await?;
            return Ok(TokenPersistResult {
                inserted: false,
                baseline_only: false,
                delta_event: false,
            });
        }

        let (project_key, project_name) = normalize_project(cwd.unwrap_or(""));
        sqlx::query(
            "INSERT INTO thread_metadata
             (thread_id, session_id, cwd, project_key, project_name, model_id, model_source,
              source, thread_source, created_at, updated_at, recency_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, 'turn_context', 'desktop', ?, ?, ?, ?, ?)
             ON CONFLICT(thread_id) DO UPDATE SET
               cwd=CASE WHEN excluded.cwd='' THEN thread_metadata.cwd ELSE excluded.cwd END,
               project_key=CASE WHEN excluded.cwd='' THEN thread_metadata.project_key ELSE excluded.project_key END,
               project_name=CASE WHEN excluded.cwd='' THEN thread_metadata.project_name ELSE excluded.project_name END,
               model_id=COALESCE(excluded.model_id, thread_metadata.model_id),
               source='desktop', thread_source=COALESCE(excluded.thread_source, thread_metadata.thread_source),
               updated_at=MAX(thread_metadata.updated_at, excluded.updated_at), last_seen_at=excluded.last_seen_at",
        )
        .bind(thread_id).bind(thread_id).bind(cwd.unwrap_or("")).bind(&project_key).bind(&project_name)
        .bind(model).bind(originator).bind(observed_at).bind(observed_at).bind(observed_at).bind(now())
        .execute(&mut *transaction).await?;

        let previous = sqlx::query(
            "SELECT total_tokens, input_tokens, cached_input_tokens, cache_write_input_tokens,
                    output_tokens, reasoning_output_tokens
             FROM thread_token_snapshots
             WHERE thread_id=? AND source='desktop_rollout'
             ORDER BY observed_at DESC, id DESC LIMIT 1",
        )
        .bind(thread_id)
        .fetch_optional(&mut *transaction)
        .await?
        .map(|row| {
            Ok::<_, AppError>([
                non_negative_u64(row.try_get("total_tokens")?, "stored total")?,
                non_negative_u64(row.try_get("input_tokens")?, "stored input")?,
                non_negative_u64(row.try_get("cached_input_tokens")?, "stored cached input")?,
                non_negative_u64(
                    row.try_get("cache_write_input_tokens")?,
                    "stored cache write",
                )?,
                non_negative_u64(row.try_get("output_tokens")?, "stored output")?,
                non_negative_u64(row.try_get("reasoning_output_tokens")?, "stored reasoning")?,
            ])
        })
        .transpose()?;
        let (baseline_only, reset_detected, delta) = match previous {
            None if current == last_values => {
                let has_tokens = current.iter().any(|value| *value != 0);
                (false, false, has_tokens.then_some(current))
            }
            None => (true, false, None),
            Some(before)
                if current
                    .iter()
                    .zip(before)
                    .any(|(now, before)| *now < before) =>
            {
                (true, true, None)
            }
            Some(before) => {
                let delta = current
                    .map(|value| value)
                    .into_iter()
                    .zip(before)
                    .map(|(now, before)| now - before)
                    .collect::<Vec<_>>();
                let all_zero = delta.iter().all(|value| *value == 0);
                (
                    false,
                    false,
                    (!all_zero).then_some(delta.try_into().map_err(|_| {
                        AppError::InvalidState("Token delta shape is invalid.".to_owned())
                    })?),
                )
            }
        };
        let total_sql = total
            .values()
            .into_iter()
            .map(|value| sqlite_i64(value, "token total"))
            .collect::<Result<Vec<_>, _>>()?;
        let last_sql = last
            .values()
            .into_iter()
            .map(|value| sqlite_i64(value, "last token usage"))
            .collect::<Result<Vec<_>, _>>()?;
        let context_window = model_context_window
            .map(|value| sqlite_i64(value, "context window"))
            .transpose()?;
        sqlx::query(
            "INSERT INTO thread_token_snapshots
             (thread_id, turn_id, observed_at, total_tokens, input_tokens, cached_input_tokens,
              cache_write_input_tokens, output_tokens, reasoning_output_tokens, last_total_tokens,
              last_input_tokens, last_cached_input_tokens, last_cache_write_input_tokens,
              last_output_tokens, last_reasoning_output_tokens, model_context_window, project_key,
              project_name, model_id, model_source, delta_total_tokens, delta_input_tokens,
              delta_cached_input_tokens, delta_cache_write_input_tokens, delta_output_tokens,
              delta_reasoning_output_tokens, baseline_only, reset_detected, fingerprint, source,
              cache_write_telemetry_present, originator, rollout_path)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'desktop_rollout', ?, ?, ?)",
        )
        .bind(thread_id).bind(turn_id).bind(observed_at)
        .bind(total_sql[0]).bind(total_sql[1]).bind(total_sql[2]).bind(total_sql[3]).bind(total_sql[4]).bind(total_sql[5])
        .bind(last_sql[0]).bind(last_sql[1]).bind(last_sql[2]).bind(last_sql[3]).bind(last_sql[4]).bind(last_sql[5])
        .bind(context_window).bind(&project_key).bind(&project_name).bind(model).bind("turn_context")
        .bind(delta.as_ref().and_then(|values| values.first().copied()).map(|value| sqlite_i64(value, "delta total")).transpose()?)
        .bind(delta.as_ref().and_then(|values| values.get(1).copied()).map(|value| sqlite_i64(value, "delta input")).transpose()?)
        .bind(delta.as_ref().and_then(|values| values.get(2).copied()).map(|value| sqlite_i64(value, "delta cached input")).transpose()?)
        .bind(delta.as_ref().and_then(|values| values.get(3).copied()).map(|value| sqlite_i64(value, "delta cache write")).transpose()?)
        .bind(delta.as_ref().and_then(|values| values.get(4).copied()).map(|value| sqlite_i64(value, "delta output")).transpose()?)
        .bind(delta.as_ref().and_then(|values| values.get(5).copied()).map(|value| sqlite_i64(value, "delta reasoning")).transpose()?)
        .bind(baseline_only as i64).bind(reset_detected as i64).bind(&fingerprint)
        .bind(total.cache_write_input_tokens.is_some() as i64).bind(originator).bind(rollout_path)
        .execute(&mut *transaction).await?;
        transaction.commit().await?;
        Ok(TokenPersistResult {
            inserted: true,
            baseline_only,
            delta_event: delta.is_some(),
        })
    }

    pub(crate) async fn persist_rate_limits(
        &self,
        observations: &[RateLimitObservation],
    ) -> Result<bool, AppError> {
        let mut inserted = false;
        for observation in observations {
            for window in &observation.windows {
                let fingerprint = fingerprint_rate(observation, window);
                let mut transaction = self.pool.begin().await?;
                let result = sqlx::query(
                    "INSERT OR IGNORE INTO desktop_rate_limit_observations
                     (event_at, observed_at, thread_id, limit_id, limit_name, plan_type, fingerprint)
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(observation.event_at).bind(now()).bind(&observation.thread_id)
                .bind(&observation.limit_id).bind(&observation.limit_name).bind(&observation.plan_type).bind(&fingerprint)
                .execute(&mut *transaction).await?;
                if result.rows_affected() == 1 {
                    let observation_id = result.last_insert_rowid();
                    sqlx::query(
                        "INSERT INTO desktop_rate_limit_windows
                         (observation_id, window_kind, used_percent, raw_window_minutes,
                          canonical_window_minutes, resets_at, resets_at_source)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(observation_id)
                    .bind(&window.window_kind)
                    .bind(window.used_percent)
                    .bind(window.raw_window_minutes)
                    .bind(window.canonical_window_minutes)
                    .bind(window.resets_at)
                    .bind(&window.resets_at_source)
                    .execute(&mut *transaction)
                    .await?;
                    inserted = true;
                }
                transaction.commit().await?;
            }
        }
        Ok(inserted)
    }

    pub(crate) async fn latest_rate_limits(
        &self,
    ) -> Result<Option<DesktopRateLimitSnapshot>, AppError> {
        let rows = sqlx::query(
            "SELECT o.event_at, o.limit_id, o.limit_name, o.plan_type, w.window_kind,
                    w.used_percent, w.canonical_window_minutes, w.resets_at
             FROM desktop_rate_limit_observations o
             JOIN desktop_rate_limit_windows w ON w.observation_id=o.id
             WHERE o.source='desktop_rollout'
             ORDER BY o.event_at DESC, o.id DESC, w.id DESC
             LIMIT 128",
        )
        .fetch_all(&self.pool)
        .await?;
        let Some(first) = rows.first() else {
            return Ok(None);
        };
        let latest_event_at: i64 = first.try_get("event_at")?;
        let mut windows = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for row in rows {
            let limit_id: Option<String> = row.try_get("limit_id")?;
            let window_kind = parse_window_kind(&row.try_get::<String, _>("window_kind")?)?;
            let duration: Option<i64> = row.try_get("canonical_window_minutes")?;
            let resets_at: Option<i64> = row.try_get("resets_at")?;
            let key = (limit_id.clone(), window_kind, duration);
            if !seen.insert(key) {
                continue;
            }
            windows.push(RateLimitWindow {
                limit_id,
                limit_name: row.try_get("limit_name")?,
                window_kind,
                used_percent: row.try_get("used_percent")?,
                remaining_percent: (100.0 - row.try_get::<f64, _>("used_percent")?)
                    .clamp(0.0, 100.0),
                window_duration_mins: duration,
                resets_at,
                plan_type: row.try_get("plan_type")?,
                rate_limit_reached_type: None,
            });
        }
        Ok(Some(DesktopRateLimitSnapshot {
            info: RateLimitInfo {
                status: RateLimitStatus::Available,
                windows,
                reset_credits_available: None,
                updated_at: latest_event_at,
                message: None,
            },
        }))
    }

    pub(crate) async fn history_for_window(
        &self,
        limit_id: Option<&str>,
        window_kind: RateLimitWindowKind,
        duration: Option<i64>,
        resets_at: Option<i64>,
        max_points: usize,
    ) -> Result<Vec<RateLimitHistorySample>, AppError> {
        let kind = match window_kind {
            RateLimitWindowKind::Primary => "primary",
            RateLimitWindowKind::Secondary => "secondary",
        };
        let limit = i64::try_from(max_points.min(64)).unwrap_or(64);
        let rows = sqlx::query(
            "SELECT o.event_at, w.used_percent
             FROM desktop_rate_limit_observations o
             JOIN desktop_rate_limit_windows w ON w.observation_id=o.id
             WHERE (o.limit_id=? OR (o.limit_id IS NULL AND ? IS NULL))
               AND w.window_kind=? AND (w.canonical_window_minutes=? OR (w.canonical_window_minutes IS NULL AND ? IS NULL))
               AND (w.resets_at=? OR (w.resets_at IS NULL AND ? IS NULL))
             ORDER BY o.event_at DESC, o.id DESC, w.id DESC LIMIT ?",
        )
        .bind(limit_id).bind(limit_id).bind(kind).bind(duration).bind(duration).bind(resets_at).bind(resets_at).bind(limit)
        .fetch_all(&self.pool).await?;
        let mut samples = rows
            .into_iter()
            .map(|row| {
                Ok(RateLimitHistorySample {
                    captured_at: row.try_get("event_at")?,
                    used_percent: row.try_get("used_percent")?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        samples.reverse();
        Ok(samples)
    }

    pub(crate) async fn history_for_range(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
        max_points: usize,
    ) -> Result<Vec<DesktopRateLimitHistoryPoint>, AppError> {
        let limit = i64::try_from(max_points.min(2_000)).unwrap_or(2_000);
        let rows = sqlx::query(
            "SELECT o.event_at, o.limit_id, w.window_kind, w.canonical_window_minutes,
                    w.used_percent, w.resets_at
             FROM desktop_rate_limit_observations o
             JOIN desktop_rate_limit_windows w ON w.observation_id=o.id
             WHERE (? IS NULL OR o.event_at >= ?) AND (? IS NULL OR o.event_at < ?)
             ORDER BY o.event_at DESC, o.id DESC, w.id DESC LIMIT ?",
        )
        .bind(start_at)
        .bind(start_at)
        .bind(end_at)
        .bind(end_at)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        let mut points = rows
            .into_iter()
            .map(|row| {
                Ok(DesktopRateLimitHistoryPoint {
                    captured_at: row.try_get("event_at")?,
                    limit_id: row.try_get("limit_id")?,
                    window_kind: row.try_get("window_kind")?,
                    duration: row.try_get("canonical_window_minutes")?,
                    used_percent: row.try_get("used_percent")?,
                    resets_at: row.try_get("resets_at")?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        points.reverse();
        Ok(points)
    }

    pub(crate) async fn history_points(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
        max_points: usize,
    ) -> Result<Vec<DesktopTokenHistoryPoint>, AppError> {
        let limit = i64::try_from(max_points.min(2_000)).unwrap_or(2_000);
        let rows = sqlx::query(
            "SELECT observed_at, delta_total_tokens, delta_input_tokens,
                    delta_cached_input_tokens, delta_cache_write_input_tokens,
                    delta_output_tokens, delta_reasoning_output_tokens, project_key, model_id
             FROM (
                 SELECT observed_at, id, delta_total_tokens, delta_input_tokens,
                        delta_cached_input_tokens, delta_cache_write_input_tokens,
                        delta_output_tokens, delta_reasoning_output_tokens, project_key, model_id
                 FROM thread_token_snapshots
                 WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
                   AND (? IS NULL OR observed_at >= ?) AND (? IS NULL OR observed_at < ?)
                 ORDER BY observed_at DESC, id DESC LIMIT ?
             ) ORDER BY observed_at ASC, id ASC",
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
                Ok(DesktopTokenHistoryPoint {
                    observed_at: row.try_get("observed_at")?,
                    delta_total_tokens: non_negative_u64(
                        row.try_get("delta_total_tokens")?,
                        "history total",
                    )?,
                    delta_input_tokens: non_negative_u64(
                        row.try_get("delta_input_tokens")?,
                        "history input",
                    )?,
                    delta_cached_input_tokens: non_negative_u64(
                        row.try_get("delta_cached_input_tokens")?,
                        "history cached input",
                    )?,
                    delta_cache_write_input_tokens: non_negative_u64(
                        row.try_get("delta_cache_write_input_tokens")?,
                        "history cache write",
                    )?,
                    delta_output_tokens: non_negative_u64(
                        row.try_get("delta_output_tokens")?,
                        "history output",
                    )?,
                    delta_reasoning_output_tokens: non_negative_u64(
                        row.try_get("delta_reasoning_output_tokens")?,
                        "history reasoning",
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
             WHERE source='desktop_rollout' AND (? IS NULL OR observed_at >= ?) AND (? IS NULL OR observed_at < ?)",
        )
        .bind(start_at)
        .bind(start_at)
        .bind(end_at)
        .bind(end_at)
        .fetch_one(&self.pool)
        .await?;
        Ok((
            usize::try_from(row.try_get::<i64, _>("observed_threads")?).unwrap_or(0),
            usize::try_from(row.try_get::<Option<i64>, _>("delta_events")?.unwrap_or(0))
                .unwrap_or(0),
            usize::try_from(
                row.try_get::<Option<i64>, _>("baseline_events")?
                    .unwrap_or(0),
            )
            .unwrap_or(0),
            usize::try_from(
                row.try_get::<Option<i64>, _>("unknown_projects")?
                    .unwrap_or(0),
            )
            .unwrap_or(0),
            usize::try_from(
                row.try_get::<Option<i64>, _>("unknown_models")?
                    .unwrap_or(0),
            )
            .unwrap_or(0),
        ))
    }

    pub(crate) async fn counts(&self) -> Result<DesktopCounts, AppError> {
        let row = sqlx::query(
            "SELECT
               (SELECT COUNT(*) FROM desktop_rollout_cursors WHERE is_desktop=1) AS tracked_rollouts,
               (SELECT COUNT(*) FROM thread_metadata WHERE source='desktop') AS indexed_sessions,
               (SELECT COUNT(*) FROM thread_token_snapshots WHERE source='desktop_rollout') AS token_events,
               (SELECT COUNT(*) FROM thread_token_snapshots WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL) AS delta_events,
               (SELECT COUNT(*) FROM thread_token_snapshots WHERE source='desktop_rollout' AND baseline_only=1) AS baseline_only_events,
               (SELECT MAX(observed_at) FROM thread_token_snapshots WHERE source='desktop_rollout') AS latest_event_at",
        ).fetch_one(&self.pool).await?;
        Ok(DesktopCounts {
            indexed_sessions: usize::try_from(row.try_get::<i64, _>("indexed_sessions")?)
                .unwrap_or(0),
            tracked_rollouts: usize::try_from(row.try_get::<i64, _>("tracked_rollouts")?)
                .unwrap_or(0),
            token_events: usize::try_from(row.try_get::<i64, _>("token_events")?).unwrap_or(0),
            delta_events: usize::try_from(row.try_get::<i64, _>("delta_events")?).unwrap_or(0),
            baseline_only_events: usize::try_from(row.try_get::<i64, _>("baseline_only_events")?)
                .unwrap_or(0),
            latest_event_at: row.try_get("latest_event_at")?,
        })
    }

    pub(crate) async fn activity(&self) -> Result<DesktopActivityTotals, AppError> {
        let rows = sqlx::query(
            "SELECT thread_id, turn_id, observed_at, delta_total_tokens, delta_input_tokens,
                    delta_cached_input_tokens, delta_cache_write_input_tokens, delta_output_tokens,
                    delta_reasoning_output_tokens, model_id, cache_write_telemetry_present,
                    date(observed_at, 'unixepoch', 'localtime') = date('now', 'localtime') AS is_today
             FROM thread_token_snapshots
             WHERE source='desktop_rollout' AND delta_total_tokens IS NOT NULL
             ORDER BY observed_at ASC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut total = DesktopActivityTotals {
            observed_tokens: 0,
            today_tokens: 0,
            observed_threads: 0,
            observed_turns: 0,
            input_tokens: 0,
            cached_input_tokens: 0,
            cache_write_input_tokens: 0,
            output_tokens: 0,
            reasoning_output_tokens: 0,
            priced_events: 0,
            total_events: 0,
            api_equivalent_cost_usd: Some(0.0),
        };
        let mut threads = std::collections::HashSet::new();
        let mut turns = std::collections::HashSet::new();
        for row in rows {
            let input = non_negative_u64(row.try_get("delta_input_tokens")?, "activity input")?;
            let cached = non_negative_u64(
                row.try_get("delta_cached_input_tokens")?,
                "activity cached input",
            )?;
            let cache_write = non_negative_u64(
                row.try_get("delta_cache_write_input_tokens")?,
                "activity cache write",
            )?;
            let output = non_negative_u64(row.try_get("delta_output_tokens")?, "activity output")?;
            let reasoning = non_negative_u64(
                row.try_get("delta_reasoning_output_tokens")?,
                "activity reasoning",
            )?;
            let total_tokens =
                non_negative_u64(row.try_get("delta_total_tokens")?, "activity total")?;
            total.observed_tokens = total.observed_tokens.saturating_add(total_tokens);
            total.input_tokens = total.input_tokens.saturating_add(input);
            total.cached_input_tokens = total.cached_input_tokens.saturating_add(cached);
            total.cache_write_input_tokens =
                total.cache_write_input_tokens.saturating_add(cache_write);
            total.output_tokens = total.output_tokens.saturating_add(output);
            total.reasoning_output_tokens = total.reasoning_output_tokens.saturating_add(reasoning);
            if row.try_get::<i64, _>("is_today")? != 0 {
                total.today_tokens = total.today_tokens.saturating_add(total_tokens);
            }
            threads.insert(row.try_get::<String, _>("thread_id")?);
            turns.insert(row.try_get::<String, _>("turn_id")?);
            total.total_events += 1;
            let complete = row.try_get::<i64, _>("cache_write_telemetry_present")? != 0;
            let model: Option<String> = row.try_get("model_id")?;
            if complete {
                if let Some(model) = model {
                    if let Ok(cost) = calculate_api_equivalent_cost(TokenCostInput {
                        model: model.clone(),
                        uncached_input_tokens: input
                            .saturating_sub(cached)
                            .saturating_sub(cache_write),
                        cached_input_tokens: cached,
                        cache_write_input_tokens: cache_write,
                        output_tokens: output,
                    }) {
                        total.priced_events += 1;
                        if let Some(value) = total.api_equivalent_cost_usd.as_mut() {
                            *value += cost.total_usd;
                        }
                    }
                }
            }
        }
        total.observed_threads = threads.len();
        total.observed_turns = turns.len();
        if total.priced_events == 0 {
            total.api_equivalent_cost_usd = None;
        }
        Ok(total)
    }
}

fn normalize_project(cwd: &str) -> (String, String) {
    let trimmed = cwd.trim().trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return ("unknown".to_owned(), "Unknown".to_owned());
    }
    let key = trimmed.to_ascii_lowercase().replace('/', "\\");
    let name = trimmed
        .rsplit(['\\', '/'])
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(trimmed);
    (key, name.to_owned())
}

fn fingerprint(
    path: &str,
    thread_id: &str,
    turn_id: &str,
    event_at: i64,
    values: &[u64; 6],
    offset: u64,
) -> String {
    let input = format!("{path}|{thread_id}|{turn_id}|{event_at}|{values:?}|{offset}");
    fnv(&input)
}

fn fingerprint_rate(
    observation: &RateLimitObservation,
    window: &super::rollout::RateLimitWindowObservation,
) -> String {
    fnv(&format!(
        "{}|{:?}|{:?}|{:?}|{}|{:?}|{:?}|{}|{:?}",
        observation.event_at,
        observation.thread_id,
        observation.limit_id,
        observation.plan_type,
        window.window_kind,
        window.raw_window_minutes,
        window.canonical_window_minutes,
        window.used_percent,
        window.resets_at
    ))
}

fn fnv(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn parse_window_kind(value: &str) -> Result<RateLimitWindowKind, AppError> {
    match value.to_ascii_lowercase().as_str() {
        "primary" => Ok(RateLimitWindowKind::Primary),
        "secondary" => Ok(RateLimitWindowKind::Secondary),
        _ => Err(AppError::InvalidState(
            "Desktop rate-limit window kind is unknown.".to_owned(),
        )),
    }
}

fn non_negative_u64(value: i64, field: &str) -> Result<u64, AppError> {
    u64::try_from(value).map_err(|_| AppError::InvalidState(format!("{field} is negative.")))
}
fn sqlite_i64(value: u64, field: &str) -> Result<i64, AppError> {
    i64::try_from(value)
        .map_err(|_| AppError::InvalidState(format!("{field} exceeds SQLite integer range.")))
}
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{connection::create_pool, migrations};

    fn counts(
        input: u64,
        cached: u64,
        cache_write: Option<u64>,
        output: u64,
        reasoning: u64,
        total: u64,
    ) -> TokenCounts {
        TokenCounts {
            input_tokens: input,
            cached_input_tokens: cached,
            cache_write_input_tokens: cache_write,
            output_tokens: output,
            reasoning_output_tokens: reasoning,
            total_tokens: total,
        }
    }

    async fn repository() -> DesktopRepository {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        migrations::run(&pool)
            .await
            .expect("migration should complete");
        DesktopRepository::new(pool)
    }

    #[tokio::test]
    async fn cumulative_deltas_ignore_repeated_last_usage_and_handle_resets() {
        let repository = repository().await;
        let first = counts(1_000, 400, Some(100), 200, 50, 1_200);
        let last = counts(1_000, 400, Some(100), 200, 50, 1_200);
        let result = repository
            .persist_token_event(
                "rollout-a",
                "thread-a",
                "turn-a",
                100,
                &first,
                &last,
                None,
                Some("C:\\Projects\\Demo"),
                Some("gpt-5.6-luna"),
                Some("Codex Desktop"),
                100,
            )
            .await
            .expect("first event should persist");
        assert!(result.delta_event);

        let second = counts(1_600, 600, Some(150), 300, 80, 1_830);
        let second_last = counts(600, 200, Some(50), 100, 30, 630);
        let result = repository
            .persist_token_event(
                "rollout-a",
                "thread-a",
                "turn-a",
                200,
                &second,
                &second_last,
                None,
                Some("C:\\Projects\\Demo"),
                Some("gpt-5.6-luna"),
                Some("Codex Desktop"),
                200,
            )
            .await
            .expect("second event should persist");
        assert!(result.delta_event);

        let repeated = repository
            .persist_token_event(
                "rollout-a",
                "thread-a",
                "turn-a",
                201,
                &second,
                &second_last,
                None,
                Some("C:\\Projects\\Demo"),
                Some("gpt-5.6-luna"),
                Some("Codex Desktop"),
                300,
            )
            .await
            .expect("repeat event should persist as a snapshot");
        assert!(!repeated.delta_event);

        let reset = counts(10, 4, Some(1), 2, 1, 12);
        let reset_result = repository
            .persist_token_event(
                "rollout-a",
                "thread-a",
                "turn-b",
                300,
                &reset,
                &reset,
                None,
                Some("C:\\Projects\\Demo"),
                Some("gpt-5.6-luna"),
                Some("Codex Desktop"),
                400,
            )
            .await
            .expect("reset event should persist");
        assert!(reset_result.baseline_only);
        let row = sqlx::query("SELECT delta_input_tokens, baseline_only, reset_detected, source FROM thread_token_snapshots ORDER BY id DESC LIMIT 1").fetch_one(&repository.pool).await.expect("snapshot should exist");
        assert!(row
            .try_get::<Option<i64>, _>("delta_input_tokens")
            .expect("delta should load")
            .is_none());
        assert_eq!(row.try_get::<i64, _>("baseline_only").unwrap(), 1);
        assert_eq!(row.try_get::<i64, _>("reset_detected").unwrap(), 1);
        assert_eq!(
            row.try_get::<String, _>("source").unwrap(),
            "desktop_rollout"
        );
    }

    #[tokio::test]
    async fn desktop_rate_limit_history_is_canonical_and_read_from_new_tables() {
        let repository = repository().await;
        let observation = RateLimitObservation {
            event_at: 100,
            thread_id: Some("thread-a".to_owned()),
            limit_id: Some("chatgpt".to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            plan_type: Some("Plus".to_owned()),
            windows: vec![super::super::rollout::RateLimitWindowObservation {
                window_kind: "primary".to_owned(),
                used_percent: 20.0,
                raw_window_minutes: Some(299),
                canonical_window_minutes: Some(300),
                resets_at: Some(1_000),
                resets_at_source: "reported".to_owned(),
            }],
        };
        assert!(repository
            .persist_rate_limits(&[observation.clone()])
            .await
            .unwrap());
        assert!(!repository
            .persist_rate_limits(&[observation])
            .await
            .unwrap());
        let snapshot = repository.latest_rate_limits().await.unwrap().unwrap();
        assert_eq!(snapshot.info.windows[0].window_duration_mins, Some(300));
        let history = repository
            .history_for_window(
                Some("chatgpt"),
                RateLimitWindowKind::Primary,
                Some(300),
                Some(1_000),
                64,
            )
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].used_percent, 20.0);
    }
}
