# Changelog

## Codex Usage Monitor v0.1.0 — Release Candidate

### Added

- Windows desktop dashboard for official Codex account, rate-limit, and usage data.
- Passive token activity history with project and model summaries.
- Burn Rate and quota depletion estimates with explicit trust labels.
- Settings for close-to-tray, Windows startup, notifications, usage thresholds, and prediction timing.
- Native alerts, system tray actions, App Server restart handling, and bounded SQLite history.
- Windows x86_64 current-user NSIS packaging.

### Privacy and scope

- Monitoring is read-only and does not perform login or model requests.
- Access/refresh tokens, cookies, full email addresses, prompts, assistant messages, and thread previews are not persisted or logged.
- Thread coverage is notification-based; the monitor does not use `thread/resume` to collect history.

### Release notes

- This is an unsigned release candidate when no real Windows signing certificate is available.
- There is no updater, GitHub Release upload, or automatic installation step.
