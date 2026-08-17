# Codex Usage Monitor

Codex Usage Monitor is a Windows desktop application foundation for local Codex usage monitoring. The current milestone establishes the Tauri shell, React UI, Rust backend, SQLite database, and health checks. Codex account and usage integration is intentionally not included yet.

## Current development stage

`DEV-001 — Tauri 2 + React + TypeScript + Rust + SQLite foundation`

## Technology stack

- Tauri 2
- React 19 + TypeScript + Vite
- Rust
- SQLite through SQLx
- Tauri Log Plugin
- npm

## Development environment

The project is intended for Windows desktop development with Node.js/npm and the Rust MSVC toolchain installed. The application uses the standard Tauri window and WebView2 runtime.

## Install dependencies

```bash
npm install
```

## Run the development desktop app

```bash
npm run tauri dev
```

The app creates its SQLite database under the Tauri App Data directory on first launch and runs the bundled migrations before registering the application state.

## Build the frontend

```bash
npm run build
```

## Build the desktop application

```bash
npm run tauri build
```

## Test and quality checks

```bash
cd src-tauri
cargo fmt --check
cargo check
cargo test
cargo clippy -- -D warnings
```

## Directory structure

```text
src/
├─ app/                 # Route definitions and top-level app composition
├─ components/          # Common, dashboard, and layout components
├─ pages/               # Dashboard and Settings pages
├─ services/            # Tauri IPC service wrappers
├─ stores/              # Lightweight application state types
├─ styles/              # CSS variables and global styles
└─ types/               # Frontend API models

src-tauri/
├─ migrations/          # SQLx migrations
└─ src/
   ├─ commands/          # Tauri commands
   ├─ database/          # SQLite pool and migration coordination
   ├─ error/             # Unified backend and command errors
   ├─ models/            # Serializable backend response models
   └─ state/             # Managed application state
```

## Database path

On Windows the default database file is:

```text
%APPDATA%\com.codexusagemonitor.app\codex-usage-monitor.db
```

The current migration creates `app_meta`, `settings`, and `schema_info`, and records schema version `1`.

## DEV-001 currently provides

- Tauri desktop shell
- React frontend
- Rust backend
- SQLite initialization
- Database migration
- Database health check
- Base Dashboard
- Base Settings page
- Unified command error responses
- Console and file logging through the Tauri Log Plugin

## Not implemented yet

Codex account monitoring is not implemented yet.

The following remain intentionally out of scope for DEV-001:

- Codex CLI detection or version discovery
- Codex App Server or JSON-RPC integration
- Account, rate-limit, token-usage, or credit data
- API-equivalent cost calculation
- System tray or startup integration
- HTTP requests, cookie access, or authentication file access

## Next stage

`DEV-002 — Codex CLI installation status, version, executable path, and App Server capability detection`
