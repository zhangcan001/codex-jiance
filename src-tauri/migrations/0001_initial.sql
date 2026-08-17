CREATE TABLE IF NOT EXISTS app_meta (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS schema_info (
    version INTEGER PRIMARY KEY NOT NULL,
    applied_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO schema_info (version, applied_at)
VALUES (1, CAST(strftime('%s','now') AS INTEGER));
