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

To start the complete local stack from the repository root, use the root launcher:

```sh
./quarry web
./quarry desktop
```

Both modes start the Axum API on `http://127.0.0.1:3001`, wait for its health endpoint, and then
start the selected UI. The desktop mode runs Tauri, whose development command starts the desktop
Vite mode. Stopping either child stops the other. The launcher deliberately rejects calls made
from `frontend/`, `backend/`, or any directory other than the repository root.

The executable is named `quarry`, so shells configured to include the repository root on `PATH`
may use `quarry web` and `quarry desktop` without the `./` prefix.

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

`frontend/.env` is the only live frontend environment file. Both Vite modes read it; keep only
public `VITE_*` browser configuration there and do not create additional `.env.local` or
mode-specific environment files.

For local development, an empty `VITE_API_BASE_URL` uses Vite's `/api` proxy to
`http://127.0.0.1:3001`. A packaged desktop build must set an HTTPS `QUARRY_API_BASE_URL`, and the
same exact origin must replace `https://api.example.invalid` in `frontend/src-tauri/tauri.conf.json`
before release.

## Backend commands

```sh
cd backend
cargo test
cargo run
```

`backend/.env` is the only backend environment file. It is ignored by Git and may contain
server-side secrets. Supported keys and defaults are documented in the configuration section of
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md#11-configuration).

The combined `quarry` launcher still relies on the backend configuration and local services below.
It pins only the development API host and port so the web proxy and desktop relay address the same
Axum process.

The shared client uses `/api/v1`. The original `/api` routes remain available temporarily for
backward compatibility. `/api/v1/capabilities` advertises the initial contract features.

The backend still requires its configured local development dependencies, including Helix for
normal startup. Server secrets belong only in `backend/.env`, local secret stores, or deployment
configuration.

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

The credential that existed in the original Quarry-web configuration template was not copied
here. It must still be revoked and removed from the original repository's history by its owners.
