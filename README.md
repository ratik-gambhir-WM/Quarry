# Quarry Multiplatform

This repository is the consolidation destination for Quarry's browser and Tauri desktop clients.
It contains one React/Vite application, one Axum product API, and a thin native shell.

The source projects were used as read-only references:

- `Quarry` at `e36c569580b3f30dbe7785e7640a5047b2e752cd`
- `Quarry-web` at `8d9c249f15d7b16d70a90cb08d2eb6d325efa836`

Neither source repository is modified by this project.

## Layout

```text
Quarry-multiplatform/
├── frontend/                 shared React/Vite source
│   ├── src/api/              versioned HTTP product API
│   ├── src/contracts/        transport-neutral contracts
│   ├── src/platform/         build-selected web/desktop adapters
│   └── src-tauri/            thin Tauri 2 shell
├── backend/                  hosted Axum API baseline
│   ├── src/                  application source
│   └── tests/                mirrors src/ with *_tests.rs unit-test files
├── docs/adr/                 architecture decisions
└── plans/                    full deployment and migration plan
```

## Frontend commands

```sh
cd frontend
npm install
npm test
npm run build:web
npm run build:desktop-ui
npm run dev:web
npm run dev:desktop
```

Web mode uses `BrowserRouter`; configure the static host to rewrite application routes to
`index.html`. Desktop mode uses `HashRouter` and bundles the same UI source.

For local development, an empty `VITE_API_BASE_URL` uses Vite's `/api` proxy to
`http://127.0.0.1:3001`. A packaged desktop build must set an HTTPS `VITE_API_BASE_URL`, and the
same exact origin must replace `https://api.example.invalid` in `frontend/src-tauri/tauri.conf.json`
before release.

## Backend commands

```sh
cd backend
cp .env.example .env
cargo test
cargo run
```

The shared client uses `/api/v1`. The original `/api` routes remain available temporarily for
backward compatibility. `/api/v1/capabilities` advertises the initial contract features.

The inherited backend still requires the local development dependencies described in
`backend/.env.example`, including Helix for normal startup. The example OpenAI key is deliberately
empty; server secrets belong only in local secret stores or deployment configuration.

## Desktop-native boundary

The desktop shell currently exposes only `save_text_file`. It validates the bundled window and
origin, content size, MIME type, filename, and extension; prompts with the native save dialog; and
writes through a sibling temporary file. Native product persistence and AI/search backends were
intentionally not copied.

Run its checks with:

```sh
cd frontend/src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Release status

This is a tested consolidation foundation, not a production deployment. Before public release,
complete the remaining gates in
[`plans/quarry-shared-desktop-web-deployment-plan.md`](plans/quarry-shared-desktop-web-deployment-plan.md):
real identity and tenant authorization, removal of user-managed OpenAI keys, durable Postgres/object
storage/jobs, provider-specific deployment, production CSP/CORS, signed desktop artifacts, and
updater configuration.

The credential that existed in the original `Quarry-web/backend/.env.example` was not copied here.
It must still be revoked and removed from the original repository's history by its owners.
