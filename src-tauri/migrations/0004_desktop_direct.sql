ALTER TABLE thread_token_snapshots
    ADD COLUMN cache_write_telemetry_present INTEGER NOT NULL DEFAULT 1;

ALTER TABLE thread_token_snapshots
    ADD COLUMN originator TEXT NULL;

ALTER TABLE thread_token_snapshots
    ADD COLUMN rollout_path TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_source_observed
    ON thread_token_snapshots(source, observed_at);

CREATE TABLE IF NOT EXISTS desktop_rollout_cursors (
    rollout_path TEXT PRIMARY KEY NOT NULL,
    thread_id TEXT NULL,
    byte_offset INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    modified_at INTEGER NULL,
    last_event_at INTEGER NULL,
    originator TEXT NULL,
    is_desktop INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS desktop_rate_limit_observations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_at INTEGER NOT NULL,
    observed_at INTEGER NOT NULL,
    thread_id TEXT NULL,
    limit_id TEXT NULL,
    limit_name TEXT NULL,
    plan_type TEXT NULL,
    fingerprint TEXT NOT NULL UNIQUE,
    source TEXT NOT NULL DEFAULT 'desktop_rollout'
);

CREATE TABLE IF NOT EXISTS desktop_rate_limit_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observation_id INTEGER NOT NULL,
    window_kind TEXT NOT NULL,
    used_percent REAL NOT NULL,
    raw_window_minutes INTEGER NULL,
    canonical_window_minutes INTEGER NULL,
    resets_at INTEGER NULL,
    resets_at_source TEXT NOT NULL,
    FOREIGN KEY(observation_id)
        REFERENCES desktop_rate_limit_observations(id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_desktop_rate_observations_event_at
    ON desktop_rate_limit_observations(event_at);
CREATE INDEX IF NOT EXISTS idx_desktop_rate_windows_observation
    ON desktop_rate_limit_windows(observation_id);
CREATE INDEX IF NOT EXISTS idx_desktop_rate_windows_identity
    ON desktop_rate_limit_windows(window_kind, canonical_window_minutes, resets_at);

INSERT OR IGNORE INTO schema_info (version, applied_at)
VALUES (4, CAST(strftime('%s','now') AS INTEGER));
