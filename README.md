# Codex Usage Monitor

Codex Usage Monitor is a Windows desktop application for local Codex usage monitoring. The current milestone keeps the DEV-002 CLI detection foundation and adds safe local Codex App Server process lifecycle control. Account and usage integration is intentionally not included yet.

## Current development stage

`DEV-003 — Codex App Server Process Manager`

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
   ├─ codex/              # CLI discovery and App Server process lifecycle
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

## DEV-003 currently provides

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
- PATH-first Codex CLI discovery with Windows npm fallbacks
- Bounded `codex --version` and `codex app-server --help` checks
- Version parsing, executable path, source, timestamp, and capability status in the Dashboard
- Windows `.cmd`/`.bat` execution through `cmd.exe /D /S /C`
- Five-second process timeout handling with child cleanup on timeout
- Long-running `codex app-server --listen stdio://` startup
- In-memory App Server lifecycle state: stopped, starting, running, stopping, failed
- PID and startup timestamp tracking
- stdin/stdout preservation for the future JSON-RPC client
- Bounded stderr diagnostic logging
- Idempotent start/stop and live process status refresh
- Windows process-tree cleanup for `.cmd`/`.bat` wrappers
- Application-exit cleanup through the Tauri lifecycle

## Not implemented yet

JSON-RPC communication, account information, and usage monitoring are not implemented yet.

The following remain intentionally out of scope for DEV-003:

- JSON-RPC communication or the initialization handshake
- Account, authentication, rate-limit, token-usage, or credit data
- Codex model calls, Threads, or Turns
- API-equivalent cost calculation
- System tray or startup integration
- HTTP requests, cookie access, or authentication file access

The database schema remains at version `1`; App Server lifecycle state is kept in memory.

## Next stage

`DEV-004 — Codex App Server JSON-RPC Client`
