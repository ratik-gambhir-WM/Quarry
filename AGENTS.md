# Quarry agent guide

This file governs the entire repository. Read it before changing code, then read the
project-local skill that matches the task and the relevant sections of
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

Quarry is a development-stage, multiplatform product with one shared React application,
a thin Tauri desktop boundary, and an Axum product API. The repository has active
uncommitted frontend work. Preserve it.

## Instruction and source-of-truth order

When instructions disagree, use this order:

1. The current user request.
2. The nearest `AGENTS.md` for the file being changed, if a nested one is added later.
3. This root `AGENTS.md`.
4. Current code, manifests, lockfiles, and tests.
5. [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the maintained architecture narrative.
6. ADRs and READMEs as historical context.
7. Files under `plans/` and the retained architecture `.docx` reports as design inputs only.

Code is the final authority. The root README and ADR 0001 currently lag the implemented
desktop API gateway. Do not reintroduce their older “save command only” model by accident.

## Start every task this way

1. Run `git status --short` from the repository root. Re-run it before handoff.
2. Identify the build roots and runtime boundaries the change touches.
3. Load the applicable project skill:
   - React, TypeScript, Vite, CSS, or browser UI:
     [`.agents/skills/react-vite-development/SKILL.md`](.agents/skills/react-vite-development/SKILL.md)
   - Axum, backend Rust, persistence, jobs, or server integrations:
     [`.agents/skills/axum-development/SKILL.md`](.agents/skills/axum-development/SKILL.md)
   - Verification for any code or documentation change:
     [`.agents/skills/code-verification/SKILL.md`](.agents/skills/code-verification/SKILL.md)
4. Inspect the relevant manifest, lockfile, entrypoint, nearby implementation, and nearby tests.
5. State the behavior and contracts that must remain stable before editing.
6. Make the smallest coherent change and verify it in the affected build roots.
7. Before handoff, perform the mandatory architecture-impact check described below.

Do not assume an empty working tree. Existing tracked edits, deletions, and untracked files
belong to the user unless the task explicitly says otherwise. Never discard, overwrite, or
broadly reformat unrelated work.

## Repository map

| Path | Role | Build system |
| --- | --- | --- |
| `frontend/` | Shared React/Vite source for web and desktop UI | npm + `package-lock.json` |
| `frontend/src-tauri/` | Tauri 2 shell, native capabilities, desktop API relay | Cargo |
| `backend/` | Axum product API and Rust application core | Cargo |
| `docs/` | Canonical Markdown architecture, ADRs, retained reports | none |
| `plans/` | Ignored local planning material; not an authoritative product contract | none |

There is no root workspace manifest or root task runner. Run commands from the correct
build root. There is currently no CI, deployment manifest, Docker setup, ESLint/Prettier
configuration, or repository-pinned Node/Rust version. Do not invent scripts that do
not exist.

## Non-negotiable architecture boundaries

### Shared frontend and platform selection

- `frontend/src/` is the only maintained React product tree.
- Web and desktop select `@quarry/router` and `@quarry/runtime` through matching Vite and
  TypeScript aliases. Keep `frontend/vite.config.ts`, `tsconfig.web.json`, and
  `tsconfig.desktop.json` aligned.
- Web uses `BrowserRouter`; desktop uses `HashRouter`. Product routes remain shared.
- Shared UI imports the stable `@quarry/runtime` contract. Raw `@tauri-apps/*` imports belong
  only in `frontend/src/platform/runtime.desktop.ts`.
- Raw `fetch` and `EventSource` usage belongs under `frontend/src/api/`.
- The web build uses `VITE_API_BASE_URL`. It is public, build-time browser configuration and
  must never contain a secret.
- The desktop Rust gateway uses `QUARRY_API_BASE_URL`. Do not substitute the web variable.

### Tauri remains a boundary, not a second backend

- Tauri owns validated IPC, native dialogs/files, local data-room authorization, and desktop
  HTTP/SSE transport to Axum.
- Product business rules, durable product state, AI orchestration, search, and persistence
  remain in the Axum service.
- Every command must validate the expected main window/origin and all caller-controlled input.
- Keep capabilities least-privileged and the CSP restrictive. Never add shell execution or
  broad filesystem access as a convenience.
- Preserve the existing API path, upload-size, PDF, MIME, canonical-path, and atomic-write checks.

### Axum dependency direction

The intended dependency flow is:

```text
main -> AppConfig -> bootstrap -> clients/repositories/services -> AppState
                                                     |
request -> router -> handler -> service -> repository/client
```

- `main.rs` loads ambient configuration and starts the server.
- `config.rs` parses and validates environment-derived configuration.
- `bootstrap.rs` owns migrations, external-client construction, repository construction, and
  service assembly.
- `AppState` contains cheap-to-clone service handles for handlers. It is not a service locator
  for repositories or configuration.
- Handlers extract and validate transport input, call one or more services, and map output.
- Services own use-case orchestration and typed service errors.
- Repositories own persistence/index operations and depend on narrow clients.
- Services and repositories must not import `AppState`, read ambient environment variables, or
  construct infrastructure clients. Current architecture tests cover lower-layer `AppState`
  imports and ambient configuration reads; preserve the full boundary and extend the tests when
  adding another enforceable forbidden-dependency pattern.

### API contract

- `/api/v1` is the client contract. `/api` is a temporary compatibility mount, not a target for
  new client code.
- The contract is handwritten across TypeScript, Tauri Rust, and Axum; there is no generated
  OpenAPI client. An endpoint change may require coordinated changes to:
  `contracts/quarryApi.ts`, the web adapter, the Tauri adapter and commands, Axum routes/DTOs,
  and adapter/route tests.
- Preserve camelCase JSON, URL encoding, status handling, multipart field names and limits,
  PDF response checks, and document-job SSE event names.
- Do not mistake transport validation for authorization. The current server does not yet
  enforce production identity or tenant boundaries.

### Persistence and indexing

- SQLite is the canonical store for users, deals, logical files, immutable versions, and blobs.
- Helix is the document graph/search projection. It is required during normal backend bootstrap.
- Ingestion commits SQLite before indexing Helix. A Helix failure can therefore leave a durable
  version; re-uploading the same bytes can retry indexing. No general reindex command exists, so
  do not “fix” the gap by deleting canonical SQLite data.
- The current schema version is 6. Startup migration recreates older schemas by dropping and
  rebuilding tables. Never point migration experiments or `cargo run` at valuable local data
  without an explicit backup and user authorization.
- `cargo run --bin clear_helix` deletes graph data. Run it only under the explicit, backed-up,
  drained rollout procedure in ADR 0002 and only when the user has authorized that operation.
- Document jobs and several caches are in memory. Do not document them as durable or distributed.

## Frontend development rules

- Use npm; do not mix npm, pnpm, Yarn, or Bun in `frontend/`.
- Preserve `package-lock.json` and avoid dependency or lockfile churn unless required.
- React and React DOM are currently pinned to a React 19 canary in the live working tree. Check
  `package.json` and the lockfile before using canary-only APIs. Preserve the uncommitted `.npmrc`
  unless the task explicitly changes dependency policy.
- Keep TypeScript strict. Avoid `any`, `@ts-ignore`, unexplained non-null assertions, and broad
  suppression comments.
- Keep state at the narrowest useful owner. Use effects for synchronization with external
  systems, not derived render state or ordinary event handling.
- Model loading, empty, error, disabled, stale/cancelled, retry, and success states for async UI.
- Reuse current primitives, CSS variables, Tailwind 4 conventions, and `components.json` aliases.
  Do not edit generated `dist` output or bypass semantic theme tokens with a parallel style system.
- Preserve semantic HTML, accessible names, keyboard/focus behavior, reduced-motion behavior,
  and visible async feedback.
- Put all runtime mock, demo, placeholder, and development-fixture product data under
  `frontend/src/fixtures/`, grouped by feature and split into clearly named fixture files. Pages,
  components, hooks, and `data/` modules must import fixtures explicitly rather than defining mock
  records inline. Keep domain types, mappers, and selectors in `frontend/src/data/`; do not mix
  them with deletable fixture records. Test-only values that are never shipped may remain local to
  one test, while shared test fixtures belong under `frontend/src/test/fixtures/`.
- Keep fixture and server-derived data visibly distinct; do not silently promote fixtures to
  authoritative data or duplicate a fixture as an error fallback. The runtime fixture tree should
  be removable during API integration so TypeScript/import failures identify every remaining
  product-fixture consumer.

## Backend development rules

- Confirm versions in `backend/Cargo.toml` and `Cargo.lock`; this project uses Axum 0.8 APIs.
- Keep handlers thin and body-consuming extractors last.
- Use `ServiceError` for use-case failures, `RepositoryError` for persistence failures, and
  `AppError` only at the HTTP boundary. Log internal context and return sanitized client messages.
- Never hold a blocking filesystem, SQLite, Office, or CPU-heavy operation on a Tokio worker.
  Follow existing client/service offloading patterns.
- Use bound parameters and the existing SQL builder/client. Preserve transaction and file-version
  invariants when changing persistence.
- Update migration tests and ADRs for schema or graph changes. Do not edit local SQLite databases,
  ignored `backend/data/`, or `.env` files as implementation shortcuts.
- Optional OpenAI and WM AI capabilities must fail explicitly when invoked but unconfigured;
  partial WM AI configuration must fail startup validation. Never move provider keys into
  frontend code or logs.
- The Rust SharePoint client under `backend/src/core/clients/sharepoint_client/` is isolated and
  is not currently wired into product bootstrap. Do not imply the frontend SharePoint modal is a
  completed integration.

## Verification matrix

Start with the narrowest relevant check, then run the applicable broader gate. The detailed
decision tree is in the code-verification skill.

### Shared React/Vite (`frontend/`)

```sh
npm test -- src/path/to/file.test.ts
npm run typecheck:web
npm run typecheck:desktop
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Use `npm run typecheck` instead of the two individual typechecks when shared code is affected.
`check:web-bundle` must follow `build:web` because it inspects `dist`. There is no `npm run lint`.

For meaningful UI work, run `npm run dev:web` or `npm run dev:desktop` and inspect the changed
route, browser console/network, keyboard/focus behavior, async states, light/dark themes, and
relevant viewport sizes.

### Axum API (`backend/`)

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

`autotests = false`; files in `backend/tests/` are included manually with `#[path]`. Use
`cargo test <name-filter>` for focused tests. Do not invent `cargo test --test <file>` commands.

Do not use `cargo run` as a routine check. Startup opens/migrates SQLite and connects to Helix.

### Tauri shell (`frontend/src-tauri/`)

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Also run frontend desktop typechecking and boundary checks for native contract changes. Reserve
`npm run build:desktop` for packaging or release validation; it is substantially broader than a
normal code check.

## Change checklists

### When changing a frontend route or feature

- Update the shared route manifest once; verify both router targets.
- Preserve session/navigation state intentionally.
- Test loading, empty, error, and success behavior.
- Verify focus restoration and keyboard behavior for dialogs, menus, and view changes.
- Use lazy loading deliberately; do not hide correctness problems behind a fallback.

### When changing an API operation

- Update the domain contract and DTOs first.
- Update web and desktop adapters without bypassing their transport boundaries.
- Update Tauri relay/native code if method, body, bytes, multipart, or SSE behavior changed.
- Update Axum route, extractor, handler, service, and error mapping as needed.
- Add contract/adapter and route tests; run both frontend typechecks and backend tests.
- Document compatibility or rollout implications.

### When changing SQLite or Helix data shape

- Define canonical ownership and stable IDs before coding.
- Make forward migration and rollback/recovery behavior explicit.
- Protect existing local data; use temporary databases in tests.
- Verify transactionality, idempotency, foreign keys, current-version uniqueness, and failure paths.
- Update ADR 0002 or add a new ADR when the operational rollout changes.

### When changing Tauri capabilities

- Prefer a narrow named command over a generic native escape hatch.
- Validate window/origin, identifiers, paths, sizes, MIME, and output.
- Scope permissions/CSP to the exact capability.
- Add Rust tests and TypeScript adapter tests.
- Verify no Tauri internals enter the web bundle.

## Security and data handling

- Treat `.env*`, local databases, API keys, tokens, absolute paths, user email, and document
  contents as sensitive. Do not print them in command output, fixtures, screenshots, or logs.
- Do not read or modify a real `.env`; use `.env.example` only as a documented schema.
- Do not put secrets in `VITE_*`, browser storage, client bundles, or Tauri command arguments when
  a server-held secret is appropriate.
- The current users table contains a development-era `api_key` field and the login flow is profile
  lookup/creation, not authentication. Do not describe either as production-safe.
- The per-user `api_key` is not used by AI services. Those use the server-side `OPENAI_API_KEY`;
  do not imply that creating a profile configures AI.
- Preserve request limits, filename/path validation, safe URL policy, CORS allowlists, sanitized
  error responses, and activity-log redaction.
- New server endpoints need real authorization before public production use; client-side route
  checks are not security boundaries.

## Generated, local, and ambiguous files

Do not hand-edit or commit generated/local outputs unless the task explicitly requires it:

- `frontend/node_modules/`, `frontend/dist/`
- `frontend/src-tauri/target/`, `frontend/src-tauri/gen/schemas/`
- `backend/target/`, `backend/data/`, `backend/.helix/`
- local `.env*` files and `*.log`

`frontend/tsconfig.node.tsbuildinfo` is tracked even though it is generated. Never edit it by
hand; review any regeneration carefully.

The duplicate icons under `backend/icons/`, empty/stale backend capability artifacts, and
`backend/helix.toml` naming are not proven product contracts. Resolve ownership before expanding
or deleting them.

## Documentation rules

- Every feature, refactor, fix, API change, data change, configuration change, and infrastructure
  change ends with an architecture-impact check. Re-read the relevant sections of
  `docs/ARCHITECTURE.md` after implementation, not only before it.
- Update `docs/ARCHITECTURE.md` in the same change when the work alters any documented route,
  runtime boundary, module responsibility, data flow or schema, API contract, configuration,
  external integration, security/trust boundary, verification command, feature maturity, known
  limitation, or operational behavior.
- If no update is needed, say `Architecture impact: none — <reason>` in the handoff. Do not omit
  the check merely because the code change is small.
- Keep `docs/ARCHITECTURE.md` descriptive: distinguish current implementation, uncommitted work,
  known gaps, and intended evolution.
- Use an ADR for a durable decision that changes a trust boundary, runtime split, data owner,
  public API version, or destructive rollout.
- Update commands from manifests, not memory. Never document a check that does not exist.
- Do not link tracked canonical documentation to ignored `plans/` files.
- If code and documentation drift, update both in the same change or call out the drift explicitly.

## Definition of done

A change is ready only when:

- The requested behavior is complete across every affected runtime.
- `docs/ARCHITECTURE.md` was re-checked and either updated or an explicit no-impact reason is ready
  for the handoff.
- Architecture and security boundaries still hold.
- Relevant focused tests and broad gates pass.
- Meaningful UI behavior has been inspected when practical.
- The final diff contains no unrelated edits, generated artifacts, secrets, or accidental lockfile
  churn.
- `git diff --check` is clean and `git status --short` has been reviewed.
- The handoff reports exact commands and outcomes, skipped checks, remaining uncertainty, and any
  pre-existing failure separately.
