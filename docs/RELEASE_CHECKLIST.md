# v0.1.0 Release Checklist

## Desktop Direct Migration

- [x] Monitoring architecture uses Codex Desktop local state and rollout observations.
- [x] Standalone CLI detection, runtime control, account reads, and protocol checks are outside the normal production initialization and Tauri handler.
- [x] Historical App Server live checks below are obsolete for this architecture and must not be used as Desktop Direct acceptance evidence.
- [ ] Fresh Desktop local-data live acceptance and a fresh NSIS build.

## Scope and metadata

- [x] `package.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json` are version `0.1.0`.
- [x] Product name is `Codex Usage Monitor`.
- [x] Identifier remains `com.codexusagemonitor.app`.
- [x] Cargo description describes the Windows Codex usage monitor.
- [x] Roadmap scope DEV-001 through DEV-025 is complete.

## Build and quality

- [x] `npm run build`.
- [x] `cargo fmt --all -- --check`.
- [x] `cargo check`.
- [x] `cargo test` (130 passed).
- [x] `cargo clippy -- -D warnings`.
- [ ] Fresh `npm run tauri build` after live Desktop acceptance.

## Windows package

- [x] Target is NSIS only.
- [x] NSIS install mode is `currentUser`.
- [x] Required source icons exist: `32x32.png`, `128x128.png`, `128x128@2x.png`, and `icon.ico`.
- [ ] Fresh `*-setup.exe` exists under `src-tauri/target/release/bundle/nsis/` and has non-zero size.
- [ ] Fresh release executable and SHA256 recorded.
- [ ] Fresh signing status recorded.

The pre-migration NSIS candidate is historical and is not Desktop Direct acceptance evidence.

## Historical application checks (obsolete after Desktop Direct migration)

- [ ] Manual start and restart persistence for Settings.
- [ ] Close-to-tray true hides the window and keeps monitoring alive.
- [ ] Close-to-tray false exits and runs cleanup.
- [ ] Start-with-Windows enable/disable state is read back and restored after testing.
- [ ] Historical runtime start/stop checks are not applicable to Desktop Direct.

The previous v0.1 candidate acceptance note is retained as history; it does not validate the Desktop Direct migration.

## Data safety

- [x] Existing monitor data is preserved and schema v4 adds Desktop Direct tables without deleting legacy rows.
- [x] Corrupt `app_settings_v1` falls back to defaults with a warning.
- [x] No model request, login, updater, cloud sync, or credential persistence is included.
- [x] Access/refresh tokens, cookies, full email, prompts, assistant messages, and thread previews are excluded from DB/logs.
- [x] Installer, `target`, `dist`, databases, logs, temp files, and generated schemas are ignored or untracked.
- [x] External Desktop SQLite connections use read-only, no-create, single-connection, query-only access.
- [x] Rollout parsing excludes conversation content and bounds a single line at 4 MiB.

## Distribution

- [x] Do not auto-install existing software.
- [x] Do not create a Git tag.
- [x] Do not create or upload a GitHub Release.
- [x] Do not commit the installer binary.

## Final acceptance

- Windows live smoke tests completed: NOT EXECUTED — UI geometry unavailable in the acceptance environment.
- Three restart cycles completed: NOT EXECUTED.
- No Codex model request was executed: YES.
- Original Windows autostart state restored: NOT TESTED; no autostart state was changed.
- Final installer filename: Not built for this migration.
- Final installer size: Not built for this migration.
- Final SHA256: Not built for this migration.
- Signing: Not checked for this migration.
- Manual installer install smoke: Not executed.
- Acceptance conclusion: `Codex Usage Monitor v0.1.0 Release Candidate requires live UI acceptance before final release.`

## Desktop Direct acceptance record

- Desktop Direct migration: development tests pass; live Desktop acceptance: NOT EXECUTED.
- State DB read-only, dynamic schema fallback, rollout cursor, partial-line, and truncation behavior: covered by automated tests.
- No Codex model request, prompt, credential read, backend request, or additional runtime was executed by the migration tests.
- Release Candidate Ready: `NO` — Desktop Direct architecture requires final live acceptance and a fresh NSIS build.
