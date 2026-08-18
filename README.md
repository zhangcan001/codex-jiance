# Codex Usage Monitor v0.1.0

Codex Usage Monitor is a Windows desktop monitor for Codex Desktop local activity. This build is the `v0.1.0 Desktop Direct Migration` and remains a Release Candidate until fresh live Desktop acceptance is complete.

## Architecture

- Reads `%USERPROFILE%\.codex` (or `CODEX_HOME`) directly.
- Uses the newest `state_*.sqlite` as an optional read-only index.
- Streams `sessions\YYYY\MM\DD\rollout-*.jsonl` with bounded lines and persistent byte cursors.
- Uses rollout `session_meta`, `turn_context`, and `token_count` records only.
- Derives Desktop token deltas, project/model totals, rate-limit observations, burn rates, and pricing coverage.
- Stores monitor data separately at `%APPDATA%\com.codexusagemonitor.app\codex-usage-monitor.db`.

The monitor does not require the standalone Codex CLI, start another local runtime, call a backend API, read credentials, or create model activity. It does not persist prompts, responses, reasoning text, tool arguments, or rollout JSON lines.

Rate-limit cards are labeled `Official · Desktop observation`; token totals and project/model reports are `Derived`; burn-rate and prediction reports are `Estimated`. An expired local observation is shown as awaiting the next Desktop activity rather than being presented as current.

## Development

```bash
npm install
npm run tauri dev
npm run build
```

Rust checks:

```bash
cd src-tauri
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

Schema version is v4. The migration is `src-tauri/migrations/0004_desktop_direct.sql`. Do not treat the previous NSIS candidate as the final Desktop Direct installer; build a fresh package after live acceptance.

## Release status

`Codex Usage Monitor v0.1.0 Desktop Direct Migration` — Release Candidate `NOT READY`. No tag or GitHub Release is created by this migration.
