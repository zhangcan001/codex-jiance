CREATE TABLE IF NOT EXISTS rate_limit_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    captured_at INTEGER NOT NULL,
    reset_credits_available INTEGER NULL,
    fingerprint TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'official'
);

CREATE TABLE IF NOT EXISTS rate_limit_windows (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id INTEGER NOT NULL,
    limit_id TEXT NOT NULL,
    limit_name TEXT NULL,
    window_kind TEXT NOT NULL,
    used_percent REAL NOT NULL,
    window_duration_mins INTEGER NULL,
    resets_at INTEGER NULL,
    plan_type TEXT NULL,
    rate_limit_reached_type TEXT NULL,
    FOREIGN KEY(snapshot_id)
        REFERENCES rate_limit_snapshots(id)
        ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_rate_limit_snapshots_captured_at
    ON rate_limit_snapshots(captured_at);

CREATE INDEX IF NOT EXISTS idx_rate_limit_snapshots_fingerprint
    ON rate_limit_snapshots(fingerprint);

CREATE INDEX IF NOT EXISTS idx_rate_limit_windows_snapshot_id
    ON rate_limit_windows(snapshot_id);

CREATE INDEX IF NOT EXISTS idx_rate_limit_windows_limit_id
    ON rate_limit_windows(limit_id);

INSERT OR IGNORE INTO schema_info (version, applied_at)
VALUES (2, CAST(strftime('%s','now') AS INTEGER));
