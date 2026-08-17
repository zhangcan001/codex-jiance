# Codex Usage Monitor

Codex Usage Monitor is a Windows desktop application for local Codex usage monitoring. The current milestone keeps the DEV-002 CLI detection foundation, adds safe local Codex App Server lifecycle control, completes the initialize/initialized handshake, validates the installed CLI against its locally generated stable App Server schema, reads official account/rate-limit/usage data, and presents a production dashboard with a local reset countdown.

## Current development stage

`DEV-022 — History Dashboard`

## Technology stack

- Tauri 2
- React 19 + TypeScript + Vite
- Rust
- SQLite through SQLx
- Tauri Log Plugin
- Tauri Notification Plugin
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
├─ pages/               # Dashboard, Projects, Models, History, and Settings pages
├─ services/            # Tauri IPC service wrappers
├─ stores/              # Lightweight application state types
├─ styles/              # CSS variables and global styles
└─ types/               # Frontend API models

src-tauri/
├─ migrations/          # SQLx migrations
└─ src/
   ├─ commands/          # Tauri commands
   ├─ alerts/            # Local notifications and in-memory alert history
   ├─ burn_rate/         # Estimated percentage-point consumption rate
   ├─ account/           # Read-only account normalization, cache, and notifications
   ├─ codex/             # CLI discovery, App Server lifecycle, and JSON-RPC transport
   ├─ database/          # SQLite pool and migration coordination
   ├─ error/             # Unified backend and command errors
   ├─ models/            # Serializable backend response models
   ├─ model_usage/       # Derived usage aggregation by observed model
   ├─ pricing/           # Versioned API-equivalent pricing catalog and engine
   ├─ prediction/        # Estimated quota depletion projections
   ├─ project/           # Derived usage aggregation by official thread cwd
   ├─ rate_limit/        # Official rate-limit reads, events, and SQLite history
   ├─ state/              # Managed application state
   ├─ tray/               # System tray and close-to-tray behavior
   ├─ history/            # Bounded monitoring history queries
   ├─ thread_usage/       # Passive token notifications and metadata inventory
   └─ usage/              # Official account usage read and cache
```

## Database path

On Windows the default database file is:

```text
%APPDATA%\com.codexusagemonitor.app\codex-usage-monitor.db
```

The current migrations create `app_meta`, `settings`, `schema_info`, official rate-limit snapshot/window history, thread metadata, and token snapshots. The database records schema version `3`.

## DEV-022 currently provides

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
- Stable installed-schema generation through `codex app-server generate-json-schema --out <temporary-directory>` without `--experimental`
- Recursive JSON Schema reading with bounded file count, directory depth, single-file size, and total-size limits
- Exact method, field, and optional capability checks against the generated schema surface
- Required/core monitoring and optional/live thread-usage compatibility results in the Dashboard
- In-memory compatibility cache keyed by executable path and detected Codex version
- Manual compatibility refresh independent from App Server process polling and lifecycle control
- Read-only `account/read` through the initialized App Server client
- Explicit `refreshToken: false` on every account request
- Tolerant account normalization for ChatGPT, API key, Bedrock, and future account types
- Backend email masking before account data reaches the frontend
- In-memory account cache with `account/updated` invalidation and refresh
- Account watcher replacement on App Server restart, with no watcher/client reference cycle
- Account compatibility gating on the specific `account/read` schema entry
- Dashboard account status, type, plan, masked email, auth mode, credential source, and update time
- Read-only Account service
- Official Rate Limits, Rate Limit Events, and SQLite Rate Limit History
- Official Usage service with local-date Today Tokens and recent daily buckets
- System tray with Open Dashboard, Hide Window, and Quit actions
- Main-window close-to-tray behavior without autostart
- Estimated Burn Rate in percentage points per hour for current official windows
- Estimated quota projections with reset comparison and confidence labels
- Native local notifications for usage thresholds and near-term estimated depletion
- In-memory alert history capped at 50 entries
- Alert worker continues while the dashboard is hidden and shuts down before other services
- Versioned Pricing Engine with exact aliases, snapshot aliases, cache-write pricing, and long-context rules
- Production Dashboard with 5 Hour Usage, Weekly Usage, Today Tokens, and API Equivalent Cost cards
- Usage Overview, all official Rate Limit Details, account details, and collapsed diagnostics
- Local Reset Countdown driven by official Unix-second `resetsAt` values
- Passive Thread Token Usage observation from `thread/tokenUsage/updated`
- Bounded non-archived Thread metadata inventory through `thread/list`
- Derived Project and Model aggregation from observed token deltas
- Explicit pricing coverage with Unknown and unsupported models excluded from cost
- History dashboard with native SVG rate-limit and observed-token charts

The API Equivalent Cost card is derived only from observed delta events with explicit model
attribution and supported pricing. Unknown or unsupported models remain visible in token totals but
are excluded from cost and reflected in pricing coverage.

Account, rate-limit, and usage access is deliberately read-only. The service does not start login,
cancel login, log out, accept API keys, persist credentials, refresh tokens, read `auth.json` or cookies,
or request credit, cost, model, thread, or turn data.

The compatibility source is the stable schema generated by the installed Codex CLI at check time; it is not a hardcoded version-to-capability table. Generated schema files stay in an auto-cleaned temporary directory and raw schema content is not stored or sent to the frontend.

Burn Rate and quota prediction are estimates derived from official `usedPercent`, `updatedAt`, and
`resetsAt` observations. They are always labeled `Estimated`; the monitor never presents them as
official Codex predictions.

## Remaining work

Full Settings remains to be implemented.
The transport can be connected while the Codex protocol handshake is not initialized or after a later transport disconnect.

The following remain intentionally out of scope for DEV-014:

- Authentication flows, credit or cost data
- Codex model calls, Threads, or Turns
- Startup integration
- HTTP requests, cookie access, authentication file access, and token persistence

Thread token usage coverage is limited to token-usage notifications observed by this monitor's App Server connection. The application does not resume threads solely to harvest token history, does not read conversation previews, and does not persist prompt or assistant message text. Rate-limit and token history queries are bounded to 2,000 points.

## Next stage

`DEV-023 — Settings`
