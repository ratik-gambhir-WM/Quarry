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

## Prerequisites

- [Node.js](https://nodejs.org/en/download) 22.5 or newer, including npm, for the shared frontend
  and Diligence Studio server
- [Rust and Cargo](https://www.rust-lang.org/tools/install) for the Axum API and Tauri shell
- The [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/). On macOS, desktop
  development requires Xcode or the Xcode Command Line Tools.
- [Docker Desktop for Mac](https://docs.docker.com/desktop/setup/install/mac-install/), including
  its `docker` CLI
- A local [HelixDB](https://www.helix-db.com/) container. The
  [HelixDB documentation](https://docs.helix-db.com/) and
  [source repository](https://github.com/HelixDB/helix-db) cover the optional Helix CLI and the
  current upstream local-development workflow. Quarry itself starts the compatible Docker
  container directly and does not require the Helix CLI.
- [LibreOffice](https://www.libreoffice.org/download/) only for document conversion flows that
  use it
- Optional server-side OpenAI and WM AI configuration for those capabilities

The launcher is currently Mac-oriented because it uses `open -a "Docker Desktop"` when the Docker
daemon is unavailable. The backend uses local SQLite by default. Do not point it at valuable local
data while experimenting with migrations or startup configuration.

## First-time setup

Run all commands below from the repository root unless a step says otherwise.

### 1. Verify the toolchain

```sh
node --version
npm --version
cargo --version
docker --version
```

Node must report 22.5 or newer. For desktop mode on a Mac that does not already have Apple's build
tools, install them once with `xcode-select --install`.

### 2. Create the local Helix container

Install and open Docker Desktop once so that its license/setup flow completes. Wait until this
command succeeds:

```sh
docker info
```

Check whether the expected Quarry container already exists:

```sh
docker container inspect helix-rgambhir-dev
```

If Docker reports `No such container`, create it once using the image and port mapping declared by
[`backend/helix.toml`](backend/helix.toml):

```sh
docker pull ghcr.io/helixdb/enterprise-dev:latest
docker create \
  --name helix-rgambhir-dev \
  --restart unless-stopped \
  --publish 6969:8080 \
  ghcr.io/helixdb/enterprise-dev:latest
```

Do not run `docker create` again when the container already exists. The `./quarry` launcher starts
the container and waits for port `6969`; it does not create or delete containers.

If your compatible Helix container has a different name, pass it to the launcher without editing
the script:

```sh
QUARRY_HELIX_CONTAINER_NAME=helix-quarry-web-dev ./quarry web
```

### 3. Install project dependencies

Install the independent npm dependencies once:

```sh
cd frontend
npm ci
cd ../diligence-studio-server
npm ci
npx playwright install chromium
cd ..
```

`npm ci` uses each package's checked-in lockfile. The Playwright command installs the Chromium
binary used to generate Diligence Studio template previews.

## Start the project

Start the browser distribution from the repository root:

```sh
./quarry web
```

Start the desktop distribution instead:

```sh
./quarry desktop
```

Keep the launcher terminal open while developing. Press `Ctrl-C` once to stop Axum, the selected
UI, and Diligence Studio together. Docker Desktop and Helix remain running because they may be
shared with other local work. Stop Helix separately only when desired:

```sh
docker stop helix-rgambhir-dev
```

The launcher must be run from the repository root. After confirming its application ports are
free, it opens Docker Desktop if the daemon is unavailable, waits for Docker, and ensures the
configured Helix container is serving `http://127.0.0.1:6969`. It then starts the Axum API at
`http://127.0.0.1:3001`, the selected UI at `http://localhost:1420`, and Diligence Studio at
`http://127.0.0.1:43127`. It waits for each runtime in dependency order and stops all three owned
process groups when any child exits. Docker Desktop and Helix are shared external dependencies and
remain running. The launcher passes the versioned Diligence Studio URL to Axum; the browser and
Tauri webview never call port `43127` directly.

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
- `Helix container ... does not exist`: create it with the first-time command above or set
  `QUARRY_HELIX_CONTAINER_NAME` to the exact name shown by `docker ps -a`.
- `port ... is already in use`: another development process owns one of ports `3001`, `1420`, or
  `43127`. Stop that process intentionally; the launcher will not terminate an unknown listener.
- Docker or Helix startup timeout: open Docker Desktop, wait for its status to become ready, then
  check `docker info`, `docker ps -a`, and `docker logs helix-rgambhir-dev` before retrying.
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
