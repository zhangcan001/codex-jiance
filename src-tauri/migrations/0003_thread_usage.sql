CREATE TABLE IF NOT EXISTS thread_metadata (
    thread_id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    forked_from_id TEXT NULL,
    parent_thread_id TEXT NULL,
    cwd TEXT NOT NULL,
    project_key TEXT NOT NULL,
    project_name TEXT NOT NULL,
    model_provider TEXT NULL,
    model_id TEXT NULL,
    model_source TEXT NULL,
    cli_version TEXT NULL,
    source TEXT NULL,
    thread_source TEXT NULL,
    git_sha TEXT NULL,
    git_branch TEXT NULL,
    thread_name TEXT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    recency_at INTEGER NULL,
    last_seen_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS thread_token_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    thread_id TEXT NOT NULL,
    turn_id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    input_tokens INTEGER NOT NULL,
    cached_input_tokens INTEGER NOT NULL,
    cache_write_input_tokens INTEGER NOT NULL,
    output_tokens INTEGER NOT NULL,
    reasoning_output_tokens INTEGER NOT NULL,
    last_total_tokens INTEGER NOT NULL,
    last_input_tokens INTEGER NOT NULL,
    last_cached_input_tokens INTEGER NOT NULL,
    last_cache_write_input_tokens INTEGER NOT NULL,
    last_output_tokens INTEGER NOT NULL,
    last_reasoning_output_tokens INTEGER NOT NULL,
    model_context_window INTEGER NULL,
    project_key TEXT NULL,
    project_name TEXT NULL,
    model_id TEXT NULL,
    model_source TEXT NULL,
    delta_total_tokens INTEGER NULL,
    delta_input_tokens INTEGER NULL,
    delta_cached_input_tokens INTEGER NULL,
    delta_cache_write_input_tokens INTEGER NULL,
    delta_output_tokens INTEGER NULL,
    delta_reasoning_output_tokens INTEGER NULL,
    baseline_only INTEGER NOT NULL DEFAULT 0,
    reset_detected INTEGER NOT NULL DEFAULT 0,
    fingerprint TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'official_app_server_event',
    FOREIGN KEY(thread_id) REFERENCES thread_metadata(thread_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_thread_observed
    ON thread_token_snapshots(thread_id, observed_at);
CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_turn
    ON thread_token_snapshots(turn_id);
CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_observed
    ON thread_token_snapshots(observed_at);
CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_project_observed
    ON thread_token_snapshots(project_key, observed_at);
CREATE INDEX IF NOT EXISTS idx_thread_token_snapshots_model_observed
    ON thread_token_snapshots(model_id, observed_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_thread_token_snapshots_fingerprint
    ON thread_token_snapshots(fingerprint);

INSERT OR IGNORE INTO schema_info (version, applied_at)
VALUES (3, CAST(strftime('%s','now') AS INTEGER));
