# Quarry

Quarry is a multiplatform diligence application with one shared React/Vite UI, a thin Tauri 2
desktop shell, an Axum product API, and a standalone Node/Express Diligence Studio service. The
browser and desktop distributions share their pages, components, routes, contracts, and product
behavior; only their platform adapters and transport paths differ.

This repository is the current implementation. The canonical detailed architecture reference is
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), with product/data ownership documented in
[`docs/DOMAIN_MODEL.md`](docs/DOMAIN_MODEL.md).

## Repository layout

```text
Quarry/
├── frontend/                 shared React/Vite application
│   ├── src/                  shared product UI, routes, contracts, and adapters
│   └── src-tauri/            thin Tauri desktop shell and API relay
├── backend/                  Axum product API and application core
│   ├── src/                  configuration, domains, adapters, and HTTP composition
│   └── tests/                unit and integration suites
├── diligence-studio-server/ standalone template catalog and PowerPoint service
│   ├── src/                  Express API, in-memory repository, catalog, and conversion logic
│   └── test/                 Vitest API and behavior suites
├── docs/                     architecture and domain-model documentation
├── .agents/skills/           repository-local development guidance
└── quarry                    root-only local process launcher
```

There is no root workspace manifest. `frontend/`, `frontend/src-tauri/`, `backend/`, and
`diligence-studio-server/` are independent build roots.

## Set up local development

This guide takes a new developer from a fresh checkout to a running web or desktop application. Run each command from the repository root unless the step says otherwise.

### 1. Install prerequisites

Install these tools before you install project dependencies:

| Tool | Requirement | Download and setup |
| --- | --- | --- |
| Git | Required | [Download Git](https://git-scm.com/downloads) |
| Node.js and npm | Node.js 22.5 or newer | [Download Node.js](https://nodejs.org/en/download) |
| Rust and Cargo | Current stable toolchain | [Install Rust with rustup](https://www.rust-lang.org/tools/install) |
| Docker | Docker Desktop on macOS, or a running Docker Engine on Linux | [Install Docker Desktop](https://docs.docker.com/desktop/setup/install/) |
| Tauri system dependencies | Required for `./quarry desktop` | [Install the Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) |
| LibreOffice | Optional; required only for supported Office conversion flows | [Download LibreOffice](https://www.libreoffice.org/download/) |

On macOS, the Tauri prerequisites require Xcode or the Xcode Command Line Tools. Install the command-line tools with:

```sh
xcode-select --install
```

The launcher can open Docker Desktop automatically on macOS. On Linux, start the Docker daemon before you run the launcher. The launcher does not automate Docker startup on Windows.

Verify the required command-line tools:

```sh
git --version
node --version
npm --version
rustc --version
cargo --version
docker --version
```

### 2. Start Quarry

Quarry uses [HelixDB](https://www.helix-db.com/) for its document graph and search projection. You can read the [HelixDB documentation](https://docs.helix-db.com/) or browse the [HelixDB source repository](https://github.com/HelixDB/helix-db). Quarry starts the compatible Docker container directly, so the Helix command-line interface is optional.

Open Docker Desktop once and complete its setup flow. On Linux, start Docker Engine. Continue when this command succeeds:

```sh
docker info
```

When you first run `./quarry web` or `./quarry desktop`, the launcher looks for the local
`helix-quarry-dev` container. If it is absent, the launcher creates it with the image and tag from
the `[local.dev]` section of [`backend/helix.toml`](backend/helix.toml), maps host port `6969` to
the image's port `8080`, and then starts it. On later runs it reuses the existing container; it
never deletes or recreates one.

If you already have a compatible container under another name, set its name when you start Quarry:

```sh
QUARRY_HELIX_CONTAINER_NAME=existing_helix_container ./quarry web
```

### 3. Install project dependencies

Install both npm packages and the browser used for Diligence Studio previews:

```sh
cd frontend
npm ci
cd ../diligence-studio-server
npm ci
npx playwright install chromium
cd ..
```

`npm ci` uses each package's checked-in lockfile. Do not run `npm install` from the repository root because Quarry has no root npm workspace. Cargo downloads Rust dependencies during the first Rust build or launcher run.

The Playwright command installs the Chromium binary used to generate Diligence Studio template previews.

### 4. Configure optional capabilities

The default local stack does not require an environment file. Add ignored local environment files only when you need an override or optional integration:

- `frontend/.env` contains public Vite settings
- `backend/.env` contains Axum settings and server-side secrets
- `diligence-studio-server/.env` contains Diligence Studio settings and optional provider secrets

Never put secrets in a `VITE_*` variable. See [Configuration](#configuration) for the supported variables.

## Run the project

Use the root launcher for normal development. It starts dependencies and application processes in the required order.

Start the browser distribution from the repository root:

```sh
./quarry web
```

Start the desktop distribution instead:

```sh
./quarry desktop
```

Keep the launcher terminal open while you develop. Press `Ctrl-C` once to stop Axum, the selected UI, and Diligence Studio. Docker and Helix remain running because other local projects may use them.

Stop the default Helix container separately when you no longer need it:

```sh
docker stop helix-quarry-dev
```

Run the launcher from the repository root. It first checks ports `3001`, `1420`, and `43127`. It then waits for Docker and Helix before it starts Axum, the selected user interface, and Diligence Studio. The launcher stops its three application process groups together when one exits.

The browser and Tauri webview call Axum. They do not call Diligence Studio on port `43127` directly.

### Local URLs

| Service | URL | Notes |
| --- | --- | --- |
| Web UI | `http://localhost:1420` | Open this for `./quarry web`; desktop mode uses the Tauri window |
| Axum API | `http://127.0.0.1:3001` | Health check: `/api/v1/health` |
| HelixDB | `http://127.0.0.1:6969` | Required before Axum bootstrap |
| Diligence Studio | `http://127.0.0.1:43127` | Server-side integration; the UI does not call it directly |

### Startup troubleshooting

- `required command not found`: install the missing prerequisite above, restart the terminal, and
  confirm the command is on `PATH`.
- Helix container creation failures: confirm `backend/helix.toml` has a valid `[local.dev]` image
  and tag, then check Docker's error output and registry access.
- `port ... is already in use`: another development process owns one of ports `3001`, `1420`, or
  `43127`. Stop that process intentionally; the launcher will not terminate an unknown listener.
- Docker or Helix startup timeout: open Docker Desktop or start Docker Engine, then check
  `docker info`, `docker ps -a`, and `docker logs helix-quarry-dev` before retrying.
- Diligence Studio preview failures: rerun `npx playwright install chromium` from
  `diligence-studio-server/`.
- Desktop-only build failures: revisit the Tauri prerequisites and confirm `xcode-select -p` and
  `cargo --version` succeed.

The browser uses `BrowserRouter` and Vite's `/api` development proxy. The desktop app uses
`HashRouter`; its Tauri Rust gateway forwards product HTTP, multipart, binary, and SSE traffic to
the same Axum API.

## Frontend commands

Run these from `frontend/`:

```sh
npm test
npm run typecheck
npm run check:boundaries
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
```

For focused development, use `npm run dev:web` or `npm run dev:desktop`. The shared source tree is
`frontend/src/`; build-time aliases select the web or desktop router and runtime adapter. Raw
browser transport belongs under `frontend/src/api/`, and raw Tauri imports belong in the desktop
runtime boundary.

## Backend commands

Run these from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

For local runtime work, `cargo run --locked` starts the API using `backend/.env` and the documented
defaults. Startup opens or migrates SQLite, connects to Helix, and initializes indexes, so it is a
runtime operation rather than a routine compile check.

## Diligence Studio server commands

Run these from `diligence-studio-server/`:

```sh
npm run typecheck
npm test
npm start
```

The checked-in catalog seeds an in-memory SQLite repository at startup. Imported templates,
assets, previews, and optional classifications disappear when this process exits. Preview creation
launches Playwright Chromium and renders through the frontend's
`/_internal/template-preview` route without writing uploaded PowerPoint files to disk.

## Desktop shell commands

Run these from `frontend/src-tauri/` when native code or the desktop transport changes:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

The desktop shell owns validated IPC, native dialogs/files, local data-room authorization, and the
desktop-to-Axum transport hop. Product business rules, durable state, AI orchestration, search, and
persistence remain in the backend.

## Configuration

For local overrides, use the three ignored environment files defined by the repository:

- `frontend/.env` contains public Vite settings shared by the web and desktop UI builds.
- `backend/.env` contains Axum settings and server-side secrets.
- `diligence-studio-server/.env` contains its host, port, preview limits, and optional
  classification provider settings.

The most common frontend settings are:

| Variable | Purpose |
| --- | --- |
| `VITE_API_BASE_URL` | Browser API origin; leave empty in local web development to use the Vite proxy |
| `VITE_WORKSPACE_DATA_SOURCE` | `api` by default, or explicit `demo` workspace data |
| `QUARRY_API_BASE_URL` | Axum origin used by the desktop Rust gateway |

Backend configuration includes API host/port, CORS origins, SQLite location, Helix, data-room roots,
document-job limits, and optional external services. See
[`docs/ARCHITECTURE.md#11-configuration`](docs/ARCHITECTURE.md#11-configuration) for the complete
environment contract and defaults. Never put secrets in `VITE_*` variables or the browser bundle.

## Runtime and API boundaries

- The browser calls the versioned Axum contract through the web adapter.
- The desktop UI calls Tauri IPC; the Rust shell relays product traffic to Axum and handles native
  capabilities at the boundary.
- Axum owns product routes, domain services, persistence, document processing, search, AI
  integrations, and background job state.
- Diligence Studio owns the app-scoped ephemeral template catalog, PowerPoint import/export, and
  preview generation behind Axum's validated template facade.
- SQLite is the canonical store for users, deals, files, versions, and blobs. Helix is the document
  graph/search projection and is required during normal backend bootstrap.

Client code targets `/api/v1`. The `/api` mount remains temporarily available for compatibility and
is not the target for new client code. See the
[`API surface`](docs/ARCHITECTURE.md#7-api-surface) and
[`web and desktop transports`](docs/ARCHITECTURE.md#6-web-and-desktop-transports) sections for the
full contract.

## Current status

Quarry is a tested development foundation, not a production-secure deployment. Current limitations
include profile lookup/creation instead of authentication, no server-side tenant authorization,
local SQLite persistence, Helix-dependent search, in-memory document jobs and caches, and no CI or
deployment manifest. Treat caller-supplied user and workspace identifiers as development behavior,
not as a security boundary.

For repository rules, architecture invariants, data-recovery guidance, and the complete verification
matrix, read [`AGENTS.md`](AGENTS.md) and [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).
