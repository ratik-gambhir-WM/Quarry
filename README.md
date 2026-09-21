# Quarry

Quarry is a multiplatform diligence application with one shared React/Vite UI, a thin Tauri 2
desktop shell, and an Axum product API. The browser and desktop distributions share their pages,
components, routes, contracts, and product behavior; only their platform adapters and transport
paths differ.

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
├── docs/                     architecture and domain-model documentation
├── .agents/skills/           repository-local development guidance
└── quarry                    root-only local process launcher
```

There is no root workspace manifest. `frontend/`, `frontend/src-tauri/`, and `backend/` are
independent build roots.

## Prerequisites

- Node.js and npm for the shared frontend
- Rust and Cargo for the Axum API and Tauri shell
- A local Helix service for normal backend startup; the default URL is
  `http://127.0.0.1:6969`
- LibreOffice only for the document conversion flows that use it
- Optional server-side OpenAI, WM AI, or Diligence Studio configuration for those capabilities

The backend uses local SQLite by default. Do not point it at valuable local data while experimenting
with migrations or startup configuration.

## Run the local stack

Install frontend dependencies once:

```sh
cd frontend
npm install
cd ..
```

Start the browser distribution from the repository root:

```sh
./quarry web
```

Start the desktop distribution instead:

```sh
./quarry desktop
```

The launcher must be run from the repository root. It starts the Axum API at
`http://127.0.0.1:3001`, waits for `/api/v1/health`, and then starts the selected UI. It stops the
child processes together when either process exits.

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

For local overrides, use the two ignored environment files defined by the repository:

- `frontend/.env` contains public Vite settings shared by the web and desktop UI builds.
- `backend/.env` contains Axum settings and server-side secrets.

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
