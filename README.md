# Codex Usage Monitor

Codex Usage Monitor is a Windows desktop application for local Codex usage monitoring. The current milestone keeps the DEV-002 CLI detection foundation, adds safe local Codex App Server lifecycle control, connects the stdio pipes to a JSON-RPC transport client, and completes the App Server initialize/initialized handshake. Account and usage integration is intentionally not included yet.

## Current development stage

`DEV-005 — App Server initialize / initialized handshake`

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
   ├─ codex/              # CLI discovery, App Server lifecycle, and JSON-RPC transport
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

## DEV-005 currently provides

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
- stdin/stdout handoff to the JSON-RPC client
- Bounded stderr diagnostic logging
- Idempotent start/stop and live process status refresh
- Lifecycle-changing App Server operations are serialized so shutdown waits for in-flight startup before cleanup
- Windows process-tree cleanup for `.cmd`/`.bat` wrappers
- Application-exit cleanup through the Tauri lifecycle
- Generic newline-delimited JSON-RPC client over the preserved App Server stdio pipes
- Monotonic request IDs, concurrent requests, out-of-order response routing, and remote errors
- Notification and server-request broadcast channels
- Request timeouts, EOF/disconnect cleanup, malformed-message handling, and bounded input lines
- Explicit client shutdown integrated with App Server stop and application-exit cleanup
- App Server status exposes whether the JSON-RPC transport is currently connected
- Codex App Server `initialize` request with explicit client metadata
- `initialized` notification sent only after a successful initialize response
- Protocol handshake lifecycle state: not initialized, initializing, initialized, or failed
- Server user-agent and runtime platform metadata from the initialize response
- Complete process and JSON-RPC cleanup when initialization fails
- One initialize handshake per transport connection, with a fresh handshake after restart

## Not implemented yet

Account information, rate limits, and token usage are not implemented yet.
The transport can be connected while the Codex protocol handshake is not initialized or after a later transport disconnect.

The following remain intentionally out of scope for DEV-005:

- Account, authentication, rate-limit, token-usage, or credit data
- Codex model calls, Threads, or Turns
- API-equivalent cost calculation
- System tray or startup integration
- HTTP requests, cookie access, or authentication file access

The database schema remains at version `1`; App Server lifecycle state is kept in memory.

## Next stage

`DEV-006 — App Server schema compatibility and installed-version protocol validation`
