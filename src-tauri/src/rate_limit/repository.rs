use serde::Serialize;
use sqlx::{Row, SqlitePool};

use crate::{
    account::unix_timestamp,
    error::AppError,
    rate_limit::model::{
        RateLimitHistorySample, RateLimitInfo, RateLimitStatus, RateLimitWindowKind,
    },
};

#[cfg(test)]
use crate::rate_limit::model::RateLimitWindow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PersistResult {
    Inserted(i64),
    NotInserted,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RateLimitHistoryPoint {
    pub(crate) captured_at: i64,
    pub(crate) limit_id: Option<String>,
    pub(crate) window_kind: String,
    pub(crate) duration: Option<i64>,
    pub(crate) used_percent: f64,
    pub(crate) resets_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(test)]
pub(crate) struct PersistedRateLimitSnapshot {
    pub(crate) id: i64,
    pub(crate) captured_at: i64,
    pub(crate) reset_credits_available: Option<u64>,
    pub(crate) fingerprint: String,
    pub(crate) source: String,
    pub(crate) windows: Vec<RateLimitWindow>,
}

#[derive(Clone)]
pub(crate) struct RateLimitRepository {
    pool: SqlitePool,
}

impl RateLimitRepository {
    pub(crate) fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub(crate) async fn persist_snapshot_if_changed(
        &self,
        info: &RateLimitInfo,
    ) -> Result<PersistResult, AppError> {
        if info.status != RateLimitStatus::Available || info.windows.is_empty() {
            return Ok(PersistResult::NotInserted);
        }

        for window in &info.windows {
            if window
                .limit_id
                .as_deref()
                .is_none_or(|limit_id| limit_id.is_empty())
            {
                return Err(AppError::InvalidState(
                    "Rate limit windows require a limit id before persistence.".to_owned(),
                ));
            }
        }

        let fingerprint = fingerprint(info)?;
        let reset_credits_available = info
            .reset_credits_available
            .map(i64::try_from)
            .transpose()
            .map_err(|_| {
                AppError::InvalidState(
                    "Rate limit reset credit count exceeds SQLite integer range.".to_owned(),
                )
            })?;

        let mut transaction = self.pool.begin().await?;
        let latest_fingerprint: Option<String> = sqlx::query_scalar(
            "SELECT fingerprint
             FROM rate_limit_snapshots
             ORDER BY captured_at DESC, id DESC
             LIMIT 1",
        )
        .fetch_optional(&mut *transaction)
        .await?;

        if latest_fingerprint.as_deref() == Some(fingerprint.as_str()) {
            transaction.commit().await?;
            return Ok(PersistResult::NotInserted);
        }

        let result = sqlx::query(
            "INSERT INTO rate_limit_snapshots
                (captured_at, reset_credits_available, fingerprint)
             VALUES (?, ?, ?)",
        )
        .bind(unix_timestamp())
        .bind(reset_credits_available)
        .bind(&fingerprint)
        .execute(&mut *transaction)
        .await?;
        let snapshot_id = result.last_insert_rowid();

        for window in &info.windows {
            sqlx::query(
                "INSERT INTO rate_limit_windows
                    (snapshot_id, limit_id, limit_name, window_kind, used_percent,
                     window_duration_mins, resets_at, plan_type, rate_limit_reached_type)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(snapshot_id)
            .bind(window.limit_id.as_deref())
            .bind(window.limit_name.as_deref())
            .bind(window_kind_name(window.window_kind))
            .bind(window.used_percent)
            .bind(window.window_duration_mins)
            .bind(window.resets_at)
            .bind(window.plan_type.as_deref())
            .bind(window.rate_limit_reached_type.as_deref())
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(PersistResult::Inserted(snapshot_id))
    }

    #[cfg(test)]
    pub(crate) async fn latest_snapshot(
        &self,
    ) -> Result<Option<PersistedRateLimitSnapshot>, AppError> {
        let row = sqlx::query(
            "SELECT id, captured_at, reset_credits_available, fingerprint, source
             FROM rate_limit_snapshots
             ORDER BY captured_at DESC, id DESC
             LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let snapshot_id: i64 = row.try_get("id")?;
        let reset_credits_available = row
            .try_get::<Option<i64>, _>("reset_credits_available")?
            .map(sqlite_i64_to_u64)
            .transpose()?;
        let windows = sqlx::query(
            "SELECT limit_id, limit_name, window_kind, used_percent,
                    window_duration_mins, resets_at, plan_type, rate_limit_reached_type
             FROM rate_limit_windows
             WHERE snapshot_id = ?
             ORDER BY id ASC",
        )
        .bind(snapshot_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(rate_limit_window_from_row)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(PersistedRateLimitSnapshot {
            id: snapshot_id,
            captured_at: row.try_get("captured_at")?,
            reset_credits_available,
            fingerprint: row.try_get("fingerprint")?,
            source: row.try_get("source")?,
            windows,
        }))
    }

    pub(crate) async fn history_for_window(
        &self,
        limit_id: Option<&str>,
        window_kind: RateLimitWindowKind,
        window_duration_mins: Option<i64>,
        resets_at: Option<i64>,
        max_samples: usize,
    ) -> Result<Vec<RateLimitHistorySample>, AppError> {
        let max_samples = i64::try_from(max_samples.min(64)).unwrap_or(64);
        let rows = sqlx::query(
            "SELECT captured_at, used_percent
             FROM (
                 SELECT s.captured_at, w.used_percent, s.id AS snapshot_id, w.id AS window_id
                 FROM rate_limit_snapshots AS s
                 INNER JOIN rate_limit_windows AS w ON w.snapshot_id = s.id
                 WHERE ((w.limit_id = ? OR (w.limit_id IS NULL AND ? IS NULL))
                    AND w.window_kind = ?
                    AND ((w.window_duration_mins = ?) OR (w.window_duration_mins IS NULL AND ? IS NULL))
                    AND ((w.resets_at = ?) OR (w.resets_at IS NULL AND ? IS NULL)))
                 ORDER BY s.captured_at DESC, s.id DESC, w.id DESC
                 LIMIT ?
             )
             ORDER BY captured_at ASC, snapshot_id ASC, window_id ASC",
        )
        .bind(limit_id)
        .bind(limit_id)
        .bind(window_kind_name(window_kind))
        .bind(window_duration_mins)
        .bind(window_duration_mins)
        .bind(resets_at)
        .bind(resets_at)
        .bind(max_samples)
        .fetch_all(&self.pool)
        .await?;

        let mut samples: Vec<RateLimitHistorySample> = Vec::with_capacity(rows.len());
        for row in rows {
            let sample = RateLimitHistorySample {
                captured_at: row.try_get("captured_at")?,
                used_percent: row.try_get("used_percent")?,
            };
            if let Some(previous) = samples.last_mut() {
                if previous.captured_at == sample.captured_at {
                    *previous = sample;
                    continue;
                }
            }
            samples.push(sample);
        }
        Ok(samples)
    }

    pub(crate) async fn history_for_range(
        &self,
        start_at: Option<i64>,
        end_at: Option<i64>,
        max_points: usize,
    ) -> Result<Vec<RateLimitHistoryPoint>, AppError> {
        let limit = i64::try_from(max_points.min(2_000)).unwrap_or(2_000);
        let rows = sqlx::query(
            "SELECT captured_at, limit_id, window_kind, window_duration_mins,
                    used_percent, resets_at
             FROM (
                 SELECT s.captured_at, s.id AS snapshot_id, w.id AS window_id,
                        w.limit_id, w.window_kind, w.window_duration_mins,
                        w.used_percent, w.resets_at
                 FROM rate_limit_snapshots AS s
                 INNER JOIN rate_limit_windows AS w ON w.snapshot_id=s.id
                 WHERE (? IS NULL OR s.captured_at >= ?)
                   AND (? IS NULL OR s.captured_at < ?)
                 ORDER BY s.captured_at DESC, s.id DESC, w.id DESC
                 LIMIT ?
             )
             ORDER BY captured_at ASC, snapshot_id ASC, window_id ASC",
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
                Ok(RateLimitHistoryPoint {
                    captured_at: row.try_get("captured_at")?,
                    limit_id: row.try_get("limit_id")?,
                    window_kind: row.try_get("window_kind")?,
                    duration: row.try_get("window_duration_mins")?,
                    used_percent: row.try_get("used_percent")?,
                    resets_at: row.try_get("resets_at")?,
                })
            })
            .collect()
    }
}

#[derive(Serialize)]
struct CanonicalSnapshot<'a> {
    reset_credits_available: Option<u64>,
    windows: Vec<CanonicalWindow<'a>>,
}

#[derive(Serialize)]
struct CanonicalWindow<'a> {
    limit_id: &'a str,
    limit_name: Option<&'a str>,
    window_kind: &'static str,
    used_percent: f64,
    window_duration_mins: Option<i64>,
    resets_at: Option<i64>,
    plan_type: Option<&'a str>,
    rate_limit_reached_type: Option<&'a str>,
}

fn fingerprint(info: &RateLimitInfo) -> Result<String, AppError> {
    let mut windows = info.windows.iter().collect::<Vec<_>>();
    windows.sort_by(|left, right| {
        left.limit_id
            .cmp(&right.limit_id)
            .then_with(|| {
                window_kind_rank(left.window_kind).cmp(&window_kind_rank(right.window_kind))
            })
            .then_with(|| left.window_duration_mins.cmp(&right.window_duration_mins))
            .then_with(|| left.resets_at.cmp(&right.resets_at))
            .then_with(|| left.limit_name.cmp(&right.limit_name))
            .then_with(|| left.used_percent.total_cmp(&right.used_percent))
            .then_with(|| left.plan_type.cmp(&right.plan_type))
            .then_with(|| {
                left.rate_limit_reached_type
                    .cmp(&right.rate_limit_reached_type)
            })
    });

    let canonical = CanonicalSnapshot {
        reset_credits_available: info.reset_credits_available,
        windows: windows
            .into_iter()
            .map(|window| CanonicalWindow {
                limit_id: window.limit_id.as_deref().unwrap_or_default(),
                limit_name: window.limit_name.as_deref(),
                window_kind: window_kind_name(window.window_kind),
                used_percent: window.used_percent,
                window_duration_mins: window.window_duration_mins,
                resets_at: window.resets_at,
                plan_type: window.plan_type.as_deref(),
                rate_limit_reached_type: window.rate_limit_reached_type.as_deref(),
            })
            .collect(),
    };

    let bytes = serde_json::to_vec(&canonical)?;
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3_u64);
    }
    Ok(format!("{hash:016x}"))
}

fn window_kind_rank(kind: RateLimitWindowKind) -> u8 {
    match kind {
        RateLimitWindowKind::Primary => 0,
        RateLimitWindowKind::Secondary => 1,
    }
}

fn window_kind_name(kind: RateLimitWindowKind) -> &'static str {
    match kind {
        RateLimitWindowKind::Primary => "primary",
        RateLimitWindowKind::Secondary => "secondary",
    }
}

#[cfg(test)]
fn sqlite_i64_to_u64(value: i64) -> Result<u64, AppError> {
    u64::try_from(value)
        .map_err(|_| AppError::InvalidState("Stored reset credit count is invalid.".to_owned()))
}

#[cfg(test)]
fn rate_limit_window_from_row(row: sqlx::sqlite::SqliteRow) -> Result<RateLimitWindow, AppError> {
    let window_kind = match row.try_get::<String, _>("window_kind")?.as_str() {
        "primary" => RateLimitWindowKind::Primary,
        "secondary" => RateLimitWindowKind::Secondary,
        _ => {
            return Err(AppError::InvalidState(
                "Stored rate limit window kind is invalid.".to_owned(),
            ))
        }
    };
    let used_percent: f64 = row.try_get("used_percent")?;

    Ok(RateLimitWindow {
        limit_id: Some(row.try_get("limit_id")?),
        limit_name: row.try_get("limit_name")?,
        window_kind,
        used_percent,
        remaining_percent: (100.0 - used_percent).clamp(0.0, 100.0),
        window_duration_mins: row.try_get("window_duration_mins")?,
        resets_at: row.try_get("resets_at")?,
        plan_type: row.try_get("plan_type")?,
        rate_limit_reached_type: row.try_get("rate_limit_reached_type")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        database::{connection::create_pool, migrations},
        rate_limit::model::{RateLimitWindow, RateLimitWindowKind},
    };

    fn window(
        limit_id: &str,
        kind: RateLimitWindowKind,
        used_percent: f64,
        resets_at: i64,
    ) -> RateLimitWindow {
        RateLimitWindow {
            limit_id: Some(limit_id.to_owned()),
            limit_name: Some("ChatGPT".to_owned()),
            window_kind: kind,
            used_percent,
            remaining_percent: 100.0 - used_percent,
            window_duration_mins: Some(300),
            resets_at: Some(resets_at),
            plan_type: Some("plus".to_owned()),
            rate_limit_reached_type: None,
        }
    }

    fn info(windows: Vec<RateLimitWindow>) -> RateLimitInfo {
        RateLimitInfo {
            status: RateLimitStatus::Available,
            windows,
            reset_credits_available: Some(2),
            updated_at: 1_700_000_000,
            message: None,
        }
    }

    async fn repository() -> RateLimitRepository {
        let pool = create_pool("sqlite::memory:")
            .await
            .expect("memory database should connect");
        migrations::run(&pool)
            .await
            .expect("database migration should complete");
        RateLimitRepository::new(pool)
    }

    #[tokio::test]
    async fn migrates_v2_and_inserts_a_multi_window_snapshot() {
        let repository = repository().await;
        let snapshot = info(vec![
            window("chatgpt", RateLimitWindowKind::Primary, 37.0, 1_700_000_100),
            window(
                "chatgpt",
                RateLimitWindowKind::Secondary,
                12.0,
                1_700_000_200,
            ),
        ]);

        let result = repository
            .persist_snapshot_if_changed(&snapshot)
            .await
            .expect("snapshot should persist");
        assert!(matches!(result, PersistResult::Inserted(_)));

        let latest = repository
            .latest_snapshot()
            .await
            .expect("latest snapshot should load")
            .expect("snapshot should exist");
        assert_eq!(latest.windows.len(), 2);
        assert_eq!(latest.windows[0].used_percent, 37.0);
        assert_eq!(
            latest.windows[1].window_kind,
            RateLimitWindowKind::Secondary
        );
        assert_eq!(latest.reset_credits_available, Some(2));
        assert_eq!(latest.source, "official");
        assert!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM pragma_foreign_key_list('rate_limit_windows')",
            )
            .fetch_one(&repository.pool)
            .await
            .expect("foreign key metadata should be readable")
                > 0
        );
    }

    #[tokio::test]
    async fn identical_snapshot_is_deduplicated_and_changed_fields_insert() {
        let repository = repository().await;
        let first = info(vec![window(
            "chatgpt",
            RateLimitWindowKind::Primary,
            37.0,
            1_700_000_100,
        )]);
        let reordered = info(vec![window(
            "chatgpt",
            RateLimitWindowKind::Primary,
            37.0,
            1_700_000_100,
        )]);
        let changed_used = info(vec![window(
            "chatgpt",
            RateLimitWindowKind::Primary,
            38.0,
            1_700_000_100,
        )]);
        let changed_reset = info(vec![window(
            "chatgpt",
            RateLimitWindowKind::Primary,
            38.0,
            1_700_000_101,
        )]);

        assert!(matches!(
            repository
                .persist_snapshot_if_changed(&first)
                .await
                .expect("first snapshot should persist"),
            PersistResult::Inserted(_)
        ));
        assert_eq!(
            repository
                .persist_snapshot_if_changed(&reordered)
                .await
                .expect("identical snapshot should be handled"),
            PersistResult::NotInserted
        );
        assert!(matches!(
            repository
                .persist_snapshot_if_changed(&changed_used)
                .await
                .expect("changed usage should persist"),
            PersistResult::Inserted(_)
        ));
        assert!(matches!(
            repository
                .persist_snapshot_if_changed(&changed_reset)
                .await
                .expect("changed reset should persist"),
            PersistResult::Inserted(_)
        ));

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limit_snapshots")
            .fetch_one(&repository.pool)
            .await
            .expect("snapshot count should load");
        assert_eq!(count, 3);
        let latest = repository
            .latest_snapshot()
            .await
            .expect("latest snapshot should load")
            .expect("latest snapshot should exist");
        assert_eq!(latest.windows[0].resets_at, Some(1_700_000_101));
    }

    #[tokio::test]
    async fn only_available_non_empty_snapshots_are_persisted() {
        let repository = repository().await;
        let mut unavailable = info(Vec::new());
        unavailable.status = RateLimitStatus::Unavailable;
        assert_eq!(
            repository
                .persist_snapshot_if_changed(&unavailable)
                .await
                .expect("unavailable result should be ignored"),
            PersistResult::NotInserted
        );
        let empty = info(Vec::new());
        assert_eq!(
            repository
                .persist_snapshot_if_changed(&empty)
                .await
                .expect("empty result should be ignored"),
            PersistResult::NotInserted
        );
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limit_snapshots")
            .fetch_one(&repository.pool)
            .await
            .expect("snapshot count should load");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn history_for_range_keeps_the_latest_two_thousand_points_in_order() {
        let repository = repository().await;
        for captured_at in 0..=2_000_i64 {
            let snapshot_id: i64 = sqlx::query_scalar(
                "INSERT INTO rate_limit_snapshots (captured_at, fingerprint) VALUES (?, ?) RETURNING id",
            )
            .bind(captured_at)
            .bind(format!("fingerprint-{captured_at}"))
            .fetch_one(&repository.pool)
            .await
            .expect("snapshot should insert");
            sqlx::query(
                "INSERT INTO rate_limit_windows
                 (snapshot_id, limit_id, window_kind, used_percent)
                 VALUES (?, 'chatgpt', 'primary', ?)",
            )
            .bind(snapshot_id)
            .bind(captured_at as f64)
            .execute(&repository.pool)
            .await
            .expect("window should insert");
        }

        let points = repository
            .history_for_range(None, None, 2_000)
            .await
            .expect("history should load");
        assert_eq!(points.len(), 2_000);
        assert_eq!(points.first().unwrap().captured_at, 1);
        assert_eq!(points.last().unwrap().captured_at, 2_000);
    }

    #[tokio::test]
    async fn history_for_window_keeps_latest_64_points_in_ascending_order() {
        let repository = repository().await;
        for captured_at in 0..100_i64 {
            let snapshot_id: i64 = sqlx::query_scalar(
                "INSERT INTO rate_limit_snapshots (captured_at, fingerprint) VALUES (?, ?) RETURNING id",
            )
            .bind(captured_at)
            .bind(format!("same-cycle-{captured_at}"))
            .fetch_one(&repository.pool)
            .await
            .expect("snapshot should insert");
            sqlx::query(
                "INSERT INTO rate_limit_windows
                 (snapshot_id, limit_id, window_kind, used_percent, window_duration_mins, resets_at)
                 VALUES (?, 'chatgpt', 'primary', ?, 300, 2000)",
            )
            .bind(snapshot_id)
            .bind(captured_at as f64)
            .execute(&repository.pool)
            .await
            .expect("window should insert");
        }

        let points = repository
            .history_for_window(
                Some("chatgpt"),
                RateLimitWindowKind::Primary,
                Some(300),
                Some(2000),
                100,
            )
            .await
            .expect("history should load");
        assert_eq!(points.len(), 64);
        assert_eq!(points.first().unwrap().captured_at, 36);
        assert_eq!(points.last().unwrap().captured_at, 99);
    }

    #[tokio::test]
    async fn transaction_rolls_back_parent_when_a_child_insert_fails() {
        let repository = repository().await;
        sqlx::query(
            "CREATE TRIGGER fail_rate_limit_window
             BEFORE INSERT ON rate_limit_windows
             WHEN NEW.limit_id = 'fail'
             BEGIN
                 SELECT RAISE(ABORT, 'forced child failure');
             END",
        )
        .execute(&repository.pool)
        .await
        .expect("failure trigger should be created");

        let result = repository
            .persist_snapshot_if_changed(&info(vec![
                window("good", RateLimitWindowKind::Primary, 10.0, 1),
                window("fail", RateLimitWindowKind::Secondary, 20.0, 2),
            ]))
            .await;
        assert!(result.is_err());

        let snapshots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limit_snapshots")
            .fetch_one(&repository.pool)
            .await
            .expect("snapshot count should load");
        let windows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM rate_limit_windows")
            .fetch_one(&repository.pool)
            .await
            .expect("window count should load");
        assert_eq!(snapshots, 0);
        assert_eq!(windows, 0);
    }
}
