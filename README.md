# Codex Usage Monitor v0.1.0

Codex Usage Monitor is a Windows desktop monitor for local Codex usage, rate limits, token activity, and bounded history. The v0.1.0 release candidate targets Windows 10/11 x86_64 and uses a current-user NSIS installer.

## What it provides

- Read-only Codex CLI detection and App Server lifecycle control.
- `initialize` / `initialized` handshake and installed-schema compatibility checks.
- Official account, rate-limit, and usage reads with local SQLite history.
- Burn Rate and quota depletion prediction, clearly labeled as `ESTIMATED`.
- Passive thread token observation with project/model/history aggregation.
- Settings for tray behavior, Windows startup, notifications, thresholds, and prediction timing.
- Native alerts, system tray controls, restart-safe watchers, and bounded background cleanup.

Data labels used by the UI and reports are `OFFICIAL`, `DERIVED`, and `ESTIMATED`. Official values come from Codex App Server responses. Derived values are calculated from observed local data. Estimates are not presented as official Codex predictions.

Thread token coverage is limited to notifications observed by this monitor's App Server connection. The application does not resume threads to harvest history, does not read conversation previews, and does not persist prompts or assistant messages. No `thread/resume` is used for monitoring.

Account and monitoring access is read-only. The app does not start login, log out, accept API keys, persist credentials, refresh tokens, read `auth.json` or cookies, or make model requests.

## Release status

`Codex Usage Monitor v0.1.0` — Release Candidate. The Windows installer is unsigned unless a real signing certificate is present in the build environment. No updater, tag, GitHub Release, or binary upload is part of this release candidate.

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

Build the Windows NSIS candidate:

```bash
npm run tauri build
```

The database is stored at `%APPDATA%\com.codexusagemonitor.app\codex-usage-monitor.db`; the current schema is v3. The installer is generated under `src-tauri\target\release\bundle\nsis\` and is intentionally not committed.

## Roadmap

DEV-001 through DEV-025 are complete for the v0.1.0 scope. See [CHANGELOG.md](CHANGELOG.md) and [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md) for the user-facing release notes and validation record.
