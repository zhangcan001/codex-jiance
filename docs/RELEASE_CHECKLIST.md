# v0.1.0 Release Checklist

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
- [x] `cargo test` (122 passed).
- [x] `cargo clippy -- -D warnings`.
- [x] `npm run tauri build`.

## Windows package

- [x] Target is NSIS only.
- [x] NSIS install mode is `currentUser`.
- [x] Required source icons exist: `32x32.png`, `128x128.png`, `128x128@2x.png`, and `icon.ico`.
- [x] The generated `*-setup.exe` exists under `src-tauri/target/release/bundle/nsis/` and has non-zero size.
- [x] SHA256 is recorded: `C25060B340B673C311F7A3B5D2DBF9D20DA5EE13B7750A4A2CFC6ADEEE139768`.
- [x] Release executable exists under `src-tauri/target/release/`.
- [x] Signing status is `Unsigned` (`signtool` unavailable; Authenticode status `NotSigned`).

Installer: `Codex Usage Monitor_0.1.0_x64-setup.exe` (3,680,773 bytes). Release executable: `codex-usage-monitor.exe` (15,509,504 bytes).

## Application checks

- [ ] Manual start and restart persistence for Settings.
- [ ] Close-to-tray true hides the window and keeps monitoring alive.
- [ ] Close-to-tray false exits and runs cleanup.
- [ ] Start-with-Windows enable/disable state is read back and restored after testing.
- [ ] App Server start, stop, restart, duplicate start, duplicate stop, unexpected exit, and RPC disconnect paths are safe.
- [ ] Account, rate-limit, and thread watchers rebind after App Server restart without duplicates.
- [ ] Three rounds of start → App Server → account/rate limit → hide/restore → stop → restart complete without model requests.

Live UI acceptance note: the development app started successfully and reported `Application ready`, database connected, schema v3, and stable Codex schema compatibility. The Windows Computer Use helper could not capture window geometry (`SetIsBorderRequired` is unsupported in this environment), so the interactive Settings, tray, App Server, watcher, autostart, and three-round smoke checks remain intentionally unchecked rather than being inferred from automated tests.

## Data safety

- [x] Existing database path and schema v3 are preserved; no migration is added for settings.
- [x] Corrupt `app_settings_v1` falls back to defaults with a warning.
- [x] No model request, login, updater, cloud sync, or credential persistence is included.
- [x] Access/refresh tokens, cookies, full email, prompts, assistant messages, and thread previews are excluded from DB/logs.
- [x] Installer, `target`, `dist`, databases, logs, temp files, and generated schemas are ignored or untracked.

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
- Final installer filename: `Codex Usage Monitor_0.1.0_x64-setup.exe`.
- Final installer size: 3,680,773 bytes.
- Final SHA256: `C25060B340B673C311F7A3B5D2DBF9D20DA5EE13B7750A4A2CFC6ADEEE139768`.
- Signing: Unsigned.
- Manual installer install smoke: Not executed.
- Acceptance conclusion: `Codex Usage Monitor v0.1.0 Release Candidate requires live UI acceptance before final release.`
