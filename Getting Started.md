# Getting started with Quarry

PostgreSQL setup: use the existing server with a dedicated database named `Quarry`; export
`QUARRY_DATABASE_URL=postgresql://localhost/Quarry` before launching. Tests use the separate
`Quarry_test` database. Both need the pgvector extension provisioned once. Docker is not required: PostgreSQL is the only store. See [storage setup and recovery](docs/postgresql-storage.md).

This guide covers local development for Quarry's browser and desktop applications.
For a product overview and upcoming features, see the [README](README.md).
The [architecture guide](docs/ARCHITECTURE.md) is the detailed reference for configuration,
runtime boundaries, data recovery, and verification.
For the main backend/frontend libraries, upstream documentation and their source locations in
Quarry, see the [key dependency map](docs/dependencies.md).

Run commands from the repository root unless a section names a different directory.

## Prerequisites

| Tool | Requirement |
| --- | --- |
| Git | Required to clone and work with the repository |
| Node.js and npm | Use Node.js 22.12 or newer in the Node 22 line, or a newer supported LTS; the current Vite dependency requires `^20.19.0` or `>=22.12.0` |
| Rust and Cargo | Rust 1.97.1 or newer, with `rustfmt` and Clippy; the backend declares this minimum for Rig 0.46 |
| PostgreSQL | Existing local service with a fresh `Quarry` database; PostgreSQL 18 was validated |
| Bash and curl | Required by the root launcher |
| Tauri system dependencies | Required for the native desktop app; follow the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform |
| Chromium | Required for embedded template previews; configure an installed executable as described below |
| LibreOffice | Required for supported Office-to-PDF conversion flows |

Install Node.js from [nodejs.org](https://nodejs.org/en/download) and Rust through
[rustup](https://www.rust-lang.org/tools/install).
The repository does not pin an exact Node or Rust toolchain version.

On macOS, install Xcode or its command-line tools for native builds:

```sh
xcode-select --install
```

Verify the required tools:

```sh
git --version
node --version
npm --version
rustc --version
cargo --version
curl --version
```

The launcher is a Bash script. The instructions below describe the macOS/Linux development
flow; Windows desktop development also requires the platform-specific Tauri prerequisites.

## Install dependencies

From a fresh checkout:

```sh
git clone https://github.com/ratik-gambhir-WM/Quarry.git
cd Quarry
cd frontend
npm ci
cd ..
```

Use npm and the checked-in lockfile. There is no root npm workspace or root task runner.
Cargo downloads Rust dependencies during the first build or launcher run.

## Configure local capabilities

The default stack requires the exported `QUARRY_DATABASE_URL` shown above. An environment file is optional; add other overrides only as needed:

- `frontend/.env`: public Vite settings.
- `backend/.env`: backend configuration and server-side secrets.

These are ignored local files. Never place provider secrets in `VITE_*` variables or the
browser bundle. The backend does not read the external service's environment file.

| Setting | Where it belongs | Purpose |
| --- | --- | --- |
| `VITE_API_BASE_URL` | Frontend | Public browser API origin; leave empty for the local Vite proxy |
| `VITE_WORKSPACE_DATA_SOURCE` | Frontend | `api` by default; `demo` explicitly selects sample workspace data |
| `QUARRY_API_BASE_URL` | Tauri process environment | Backend origin for the desktop gateway; supplied by the root launcher |
| `OPENAI_API_KEY` | Backend | Enables supported OpenAI capabilities; model settings have documented defaults |
| `QUARRY_HYBRID_SEARCH_ENABLED` | Backend | `false` by default; opt in only after reviewing the search rollout guidance |
| `QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH` | Backend | Absolute path to an installed Chromium executable for template previews |
| `QUARRY_SLIDE_TEMPLATES_PREVIEW` | Backend | `chromium` by default; `disabled` turns preview generation off |
| `QUARRY_SOFFICE` | Backend | Optional LibreOffice executable override |
| `QUARRY_DATABASE_URL` | Backend | Required server-only PostgreSQL URL, e.g. `postgresql://localhost/Quarry` |
| `QUARRY_EMBEDDING_DIMENSION` | Backend | Shared document/notebook vector dimension, default `1536` |

Embedded previews can discover installed Chromium/Playwright caches. If discovery fails,
set `QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH` or disable previews when that capability is not needed.
Install [LibreOffice](https://www.libreoffice.org/download/) for supported Office conversions.
Missing optional providers fail explicitly when invoked; partially configured WM AI settings
fail startup validation.

See [Configuration](docs/ARCHITECTURE.md#11-configuration) for the complete environment contract,
including provider groups, template settings, timeouts, storage, and model dimensions. Changing
an embedding model or dimension requires coordinated reindex planning; configuration alone
does not convert existing data.

## Start the application

From the repository root, choose one target:

```sh
./quarry web
```

```sh
./quarry desktop
```

The launcher starts an existing Postgres.app 18 cluster if needed on macOS and starts the backend, which opens its PostgreSQL pool during bootstrap.
There is no separate database connection preflight. If the connection fails, startup exits
without starting the UI; otherwise the selected UI starts after the API is ready.
Open [localhost:1420](http://localhost:1420) for the web target; the desktop target opens a Tauri
window. Register an account or sign in to use protected workflows.

Templates run inside the backend and persist in Quarry's shared PostgreSQL database.
The browser and desktop app call the backend. The launcher always selects embedded templates
and runs template processing inside Axum.

### Local storage

PostgreSQL (with pgvector) is the only product store: documents, notebooks and templates,
including template classification, lexical postings and vectors. Startup initializes an empty
PostgreSQL schema or validates version 5 and its catalog. A database from an earlier schema is
refused intact; use a fresh database or the explicit guarded reset in
[storage setup and recovery](docs/postgresql-storage.md). Do not delete stores as a setup shortcut.
Retired storage containers and volumes are left untouched.

### Local addresses

| Service | Address | When used |
| --- | --- | --- |
| Web UI / desktop development UI | `http://localhost:1420` | Both launcher targets |
| Backend API | `http://127.0.0.1:3001` | Health: `/api/v1/health` |

### Stop the application

Keep the launcher terminal open. Press `Ctrl-C` to stop the backend and selected UI. If an
application process exits, the launcher stops its other application processes too. PostgreSQL
remains running.

## Development and build commands

### Frontend

Run from `frontend/`. These commands start only the selected frontend target; the backend
must already be available for API workflows.

| Command | Purpose |
| --- | --- |
| `npm run dev:web` | Vite browser development server |
| `npm run dev:desktop-ui` | Vite desktop UI without the native shell |
| `npm run dev:desktop` | Tauri development app; starts its Vite UI through the Tauri hook |
| `npm run build:web` | Typecheck and build the web distribution |
| `npm run build:desktop-ui` | Typecheck and build the desktop UI |
| `npm run build:desktop` | Build/package the native app; broader than a routine code check |
| `npm run preview` | Serve the current Vite build locally; does not start backend services |

The shared product source is `frontend/src/`. Build-time aliases select the web or desktop
router and runtime adapter. Browser transport belongs in `src/api/`; raw Tauri imports belong
in `src/platform/runtime.desktop.ts`.

### Backend

For standalone runtime work, run from `backend/`:

```sh
cargo run --locked
```

This loads `backend/.env`, connects to PostgreSQL, initializes/validates the selected schema and resumes durable template stages.
It is a runtime operation, not a compilation check. Do not run it alongside the root launcher
on the same port. Client code targets `/api/v1`; `/api` is a temporary compatibility mount.

## Test and verify changes

Start with a focused test, then run the relevant broader checks. No root test command or
frontend lint script exists. See the [verification guide](.agents/skills/code-verification/SKILL.md)
for the full decision tree.

### Shared frontend

Run from `frontend/`:

```sh
npm test -- tests/components/data-room/document-search/DataRoomDocumentSearch.test.tsx
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run check:notebook-bundle:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
npm run check:notebook-bundle:desktop
```

Replace the focused test path with the affected test. `typecheck` checks both web and desktop;
`typecheck:web` and `typecheck:desktop` are also available individually. Run bundle checks after
the matching build because they inspect `dist`. For meaningful UI work, inspect the affected
workflow, async states, keyboard/focus behavior, themes, viewport sizes, and console/network activity.

### Backend

Run from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

Use a name filter for focused tests, for example:

```sh
cargo test --locked domains::documents::search
```

Backend tests are manually included from `tests/unit/` and `tests/integration/`; they are not
independent `cargo test --test <file>` targets. Database-backed tests require a separate `_test`
PostgreSQL database and create isolated schemas; other capabilities use test clients.
Some opt-in tests require explicitly selected disposable services and remain ignored in the normal
suite. Do not enable them against a live store. Do not use startup or destructive database maintenance as verification commands.

Before adding or moving backend validation or authorization, use the
[boundary placement decision and review checklist](docs/backend-boundary-checks.md#placement-decision-for-every-new-or-changed-check).
Caller-only checks and initial request authorization belong at the HTTP boundary; checks needing
loaded state, atomic permissions, or ongoing revocation remain at the operation they protect.
Use `handler/validate.rs` for reusable request validation and verify rejection order through the
handler. `cargo test --locked --lib architecture_tests` provides a focused dependency check;
the full backend suite above includes it.

### Desktop shell

Run from `frontend/src-tauri/` when native code, capabilities, or transport changes:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Also check the frontend desktop target, shared boundary checks, and adapter tests for native
contract changes. Reserve `npm run build:desktop` for packaging/release validation.

## Troubleshooting

- **Missing command:** install the prerequisite, reopen the terminal, and verify it is on `PATH`.
- **Node version error:** check `node --version`; use a version compatible with the installed Vite dependency.
- **Port already in use:** check `3001` and `1420`. Stop the owning
  process intentionally; the launcher will not stop an unknown listener.
- **Incompatible PostgreSQL schema:** the database was created by an earlier schema version or a
  different dimension setting. Use a fresh database or the guarded reset; startup never drops data.
- **Embedded template preview failure:** check Chromium discovery or set its absolute executable
  path. Ensure the UI's `/_internal/template-preview` route is available.
- **Legacy preview failure:** run `npx playwright install chromium` in the external service checkout.
- **Office conversion failure:** install LibreOffice and check `QUARRY_SOFFICE` if using an override.
- **AI unavailable:** check the backend provider configuration. Hybrid search is disabled by default
  and offers explicit keyword fallback; see [hybrid search guidance](docs/hybrid-document-search.md).
- **Desktop build failure:** revisit Tauri system dependencies; on macOS, check `xcode-select -p`.

## Repository and runtime map

| Path | Responsibility |
| --- | --- |
| `frontend/` | Shared React/Vite UI, routes, contracts, and platform adapters |
| `frontend/src-tauri/` | Validated IPC, native dialogs/files, and desktop API relay |
| `backend/` | Axum routes, product services, persistence, document processing, search, and AI orchestration |
| `docs/` | Canonical architecture, domain model, and focused operational guidance |
| `.agents/skills/` | Repository-specific development and verification guidance |
| `quarry` | Repository-root development launcher |

Web and desktop share the same product behavior. Web uses `BrowserRouter` and browser HTTP;
desktop uses `HashRouter` and Tauri IPC to the same backend. PostgreSQL owns durable product and
embedded template data, including all search artifacts. Document jobs and several caches
remain in memory. Authentication and membership checks are implemented, but production tenancy,
identity lifecycle, CI, and deployment automation remain incomplete.

Read [AGENTS.md](AGENTS.md) before contributing and consult the
[architecture](docs/ARCHITECTURE.md) and [domain model](docs/DOMAIN_MODEL.md) for detailed contracts.
