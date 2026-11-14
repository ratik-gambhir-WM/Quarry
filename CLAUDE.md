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
6. READMEs as historical context.
7. Files under `plans/` and the retained architecture `.docx` reports as design inputs only.

Code is the final authority. The root README is a product overview; use
[`Getting Started.md`](Getting%20Started.md) for developer onboarding. Preserve the implemented
desktop API gateway; do not reintroduce the older “save command only” model.

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
   - Multi-axis review after implementation, before merge, or when review is requested:
     [`.agents/skills/code-review-and-quality/SKILL.md`](.agents/skills/code-review-and-quality/SKILL.md)
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
| `docs/` | Canonical Markdown architecture, domain model, retained reports | none |
| `plans/` | Tracked implementation plans retained as design inputs; not an authoritative product contract | none |

There is no root workspace manifest or root task runner. Run commands from the correct
build root. There is currently no CI, deployment manifest, repository-owned container manifest, ESLint/Prettier
configuration, or repository-pinned Node/Rust version. Do not invent scripts that do
not exist.

### Embedded templates

- Templates run only inside Axum through `domains/templates/slide_templates`, with native
  conversion, Chromium/CDP previews, shared durable PostgreSQL and Helix metadata/dual-vector
  projection. The browser and Tauri webview call only Axum. Quarry's hidden Vite route supplies
  the shared read-only SVG render surface.
- Bootstrap's main API assembly initializes one template engine for the facade, native routes
  and shutdown lifecycle; there is no alternate template backend or service selector.
- `app/schema.sql` owns the fresh PostgreSQL schema, equivalent to former SQLite marker 18;
  `app/migrations.rs` initializes/validates version 2, checksums and catalog fingerprints.
  Bootstrap passes one bounded asynchronous `PostgresClient` pool to product and template
  repositories. The database is `Quarry`; tests require a separate `_test` database and isolated
  schemas. Old SQLite files and Helix stores remain untouched; no import, fallback or reset exists.
- Template provider keys belong in the ignored `backend/.env`; never place them in `VITE_*`,
  Tauri IPC payloads, or Axum request data.

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
- Do not mistake transport validation for authorization. The server verifies bearer sessions and per-user ownership; organization tenancy and
  production identity lifecycle remain incomplete.

### Persistence and indexing

- PostgreSQL is canonical for users, deals, logical files, immutable versions, blobs, notebooks
  and embedded templates/assets/previews/classification/embeddings. The dedicated database is `Quarry`.
- Helix remains the graph/search projection, required at normal startup. Ingestion commits
  PostgreSQL before indexing; graph failure preserves canonical records and exact-content retry
  reuses the same identity and accepted image text. There is no distributed transaction or general
  bulk reindex tool. Never delete canonical rows to conceal a graph failure.
- PostgreSQL schema version 2 adds bounded resource revisions and retained deal lifecycle guards
  to the former marker-18 layout. Version 1 is intentionally unsupported; select an empty database. Startup
  initializes an empty selected schema or validates its version, checksum, live catalog and owner/
  document readiness invariants. Incompatible nonempty layouts fail without deletion. No old-data
  migration, SQLite fallback or destructive relational reset is supported. Leave old stores alone.
- One application writer lease is retained by the pool's dedicated session through router and
  detached-worker clones. Writes fail closed on ownership loss. Reads use ordinary pooled
  connections with no writer-session ping, ownership/generation check or explicit row lock,
  including at snapshot commit. Aggregate writes additionally
  use a schema-scoped transaction advisory lock and retained revision/lease guards; read aggregates
  use a repeatable-read snapshot. Keep helpers on the same transaction/connection.
  Typed resource locks, mutation RETURNING and database-clock deadlines are available in the query
  layer; read-only runners reject locking SELECT and mutations. Global write controls remain until
  durable worker claims/recovery and remote-publication safeguards are implemented and verified. Do not infer
  multi-instance support from independent reads or test-only unguarded transaction fixtures.
- Template lexical search uses indexed Unicode 6.1 field frequencies and FTS5-compatible weighted
  BM25 in PostgreSQL. Preserve captured lexical parity fixtures, owner/app filters and stable ties;
  document and template Helix projection/search contracts remain unchanged.
- `sqliteCommitted` and deletion phase `sqlite` are retained historical wire values meaning
  canonical PostgreSQL commit/cleanup. Do not rename them accidentally.
- Keep `HELIX_VECTOR_DIMENSION` aligned with both document and notebook indexes. Template vector
  dimensions are configured independently. A configuration change does not resize existing indexes;
  do not add automatic drop/recreation or pad/truncate vectors. A requested dimension change needs
  a drained, explicitly scoped operation and a re-embedding plan for populated projections; see
  `docs/postgresql-storage.md` for the verified-empty-index case.
- `clear_helix` is destructive. Run it only with explicit authorization, matching backups and a
  drained operational procedure; never use it as verification. Document jobs/caches remain in memory.
- See `docs/postgresql-storage.md` for server-only URL/pool configuration, fresh local graph port
  6970, isolated PostgreSQL tests, backups and recovery.

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
- Include purposeful motion when building or changing UI. Read
  [`docs/interface-motion.md`](docs/interface-motion.md), reuse the existing timing/easing tokens
  and shared controls, keep keyboard-triggered changes instant, and implement reduced-motion and
  interruption behavior in the same change. Preserve state, focus, and scroll rather than remounting
  content to replay an animation. Verify the animated and immediate paths before handoff.
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

### Frontend workflow and subscription pattern

Follow this pattern for ongoing frontend development and refactoring. Apply it to the feature
being changed, keep abstractions proportional to its complexity, and preserve unrelated work.

- Keep components focused on rendering, editable form state, and interaction/focus behavior.
  Move multi-step request orchestration into a feature-owned hook or lifecycle store. Give each
  workflow one owner for explicit progress stages, errors, retries, and completion. Use
  [`useDealSaveWorkflow.ts`](frontend/src/components/hub/sidebar/useDealSaveWorkflow.ts) and
  [`uploadWorkflow.ts`](frontend/src/components/data-room/uploadWorkflow.ts) as existing examples.
- Keep provider values stable for actions and store handles. Subscribe consumers to the specific
  state they render, using the existing
  [`useStoreSelector`](frontend/src/hooks/useStoreSelector.ts) where appropriate. Separate catalog,
  selected-document/composer, and save/stream state so typing and streamed tokens do not refresh
  unrelated navigation/history consumers. Preserve unchanged object and array references; add
  memoization at demonstrated parent-render boundaries rather than wrapping everything.
- Bound concurrent uploads and other fan-out work. Guard every asynchronous continuation against
  unmount, identity changes, and superseded attempts before starting follow-up work or publishing
  results. Invalidate stale callbacks and release owned subscriptions/timers on cleanup. Handle
  synchronous terminal replay before a subscription returns its cleanup function. Local cleanup
  must not be presented as cancellation of already-admitted server work.
- Prevent duplicate submissions synchronously. Retain committed resource IDs across partial
  failures, reconcile uncertain create responses before replaying them, and reuse immutable
  operation IDs and submitted content where the API supports idempotent retries. Keep newer edits
  distinct from the version being saved.
- For rich-document edit buffers, track edit versions and submitted/saved versions instead of
  serializing documents during rendering or unload checks. Preserve undo-to-saved detection,
  edits made during pending saves, retry snapshots, revision conflicts, and explicit reload
  confirmation. Follow the ownership split in
  [`NotebookProvider.tsx`](frontend/src/components/notebook/NotebookProvider.tsx),
  [`notebookSession.ts`](frontend/src/data/notebook/notebookSession.ts), and
  [`notebookPersistence.ts`](frontend/src/data/notebook/notebookPersistence.ts).
- Choose lazy boundaries from measured application imports and loading costs. Measure the entry
  and its complete static import graph for both targets; keep deferred editors/viewers out of
  eager startup. Preserve explicit manual-chunk ownership so shared dependencies do not pull
  deferred features into startup. Do not relax budgets or merely redistribute eager code to make
  the entry file appear smaller.
- Add focused regression coverage for the lifecycle and state contracts being changed, including
  relevant late responses, stale attempts, cleanup, concurrency, duplicate prevention, and
  partial-failure recovery. Verify subscription isolation and loading costs with reproducible
  evidence. Preserve accessible feedback, focus, motion, routes, API contracts, and both runtime
  targets, then follow the verification and architecture-impact gates below.

## Backend development rules

- Confirm versions in `backend/Cargo.toml` and `Cargo.lock`; this project uses Axum 0.8 APIs.
- Keep all backend Rust tests and test-only support code under `backend/tests/`, including
  mock constructors, fixture types, helper functions, and test-only inherent implementations.
  Unit suites mirror `backend/src/` under `tests/unit/`; cross-module suites and shared support
  live under `tests/integration/`. Source files may retain only test-gated `#[path = "..."]`
  module declarations pointing into `backend/tests/`. Do not inline test bodies or helpers in
  `backend/src/`, widen production visibility for tests, or enable unused helpers in production.
  Because `autotests = false`, wire every test/support module into the crate's test-only module
  tree and verify it is compiled; the architecture suite enforces this separation.
- Before adding, moving, or reviewing a backend check, apply the
  [boundary placement decision](docs/backend-boundary-checks.md#placement-decision-for-every-new-or-changed-check).
  State the loaded-state or lifecycle reason for any check kept later; a business-logic label
  alone is not a reason to delay a caller-only check.
- Keep handlers thin and body-consuming extractors last. Check every client-controlled path,
  query, header, JSON, and multipart value at the HTTP handler boundary before invoking product
  work. Put reusable request-validation functions in handler-owned `handler/validate.rs` modules;
  use `validate.rs`, not `validation.rs` or a generic `file.rs`, for dedicated validation modules.
  Multipart extraction and streaming size limits may live in a handler-owned `upload.rs`.
  Models hold data; services and adapters must not own request-shape validation or HTTP errors.
- Authenticate and authorize requests at the HTTP boundary before protected product work.
  Shared authentication middleware may enforce common header/session/path policy; handlers own
  resource checks derived from request bodies. Use the verified actor, never a caller-supplied
  identity, and delegate state lookup through service/policy interfaces rather than repositories.
- Move checks that can safely reject a request early into that boundary. Keep checks that require
  loaded state or must protect an operation at its point of use: atomic transaction permissions,
  ownership/current-version constraints, canonical filesystem containment, persisted/upstream
  content integrity, worker and stream revocation, and final result authorization. Do not remove
  these safeguards in favor of a stale preflight. Pure shared parsers may support both boundary
  validation and those integrity checks, but must not depend on HTTP types or errors.
- Use `ServiceError` for use-case failures, `RepositoryError` for persistence failures, and
  `AppError` only at the HTTP boundary. Log internal context and return sanitized client messages.
- Never hold a blocking filesystem, Office, or CPU-heavy operation on a Tokio worker.
  Follow existing client/service offloading patterns.
- Do not write ad hoc SQL in handlers/services. Use bound parameters through the owned PostgreSQL query modules/client,
  and preserve transaction and file-version invariants when changing persistence.
- Update migration tests and architecture/recovery documentation for schema or graph changes. Do
  not edit local PostgreSQL test schemas, ignored `backend/data/`, or `.env` files as implementation
  shortcuts.
- Optional OpenAI and WM AI capabilities must fail explicitly when invoked but unconfigured;
  partial WM AI configuration must fail startup validation. Never move provider keys into
  frontend code or logs.
- The Rust SharePoint client under `backend/src/adapters/sharepoint/` is isolated and
  is not currently wired into product bootstrap. Do not imply the frontend SharePoint modal is a
  completed integration.

## Verification matrix

Start with the narrowest relevant check, then run the applicable broader gate. The detailed
decision tree is in the code-verification skill.

### Shared React/Vite (`frontend/`)

```sh
npm test -- tests/path/to/file.test.ts
npm run typecheck:web
npm run typecheck:desktop
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

Use `npm run typecheck` instead of the two individual typechecks when shared code is affected.
`check:web-bundle` must follow `build:web` because it inspects `dist`. There is no `npm run lint`.
Run each bundle-size check immediately after its matching build, before another target overwrites
`dist`. The checks enforce the entry and complete static startup JavaScript budgets as well as
individual application chunk limits.
Run the matching notebook-bundle check after each build to execute the production editor modules;
source tests and compilation alone do not detect chunk initialization cycles.

For meaningful UI work, run `npm run dev:web` or `npm run dev:desktop` and inspect the changed
route, browser console/network, keyboard/focus behavior, async states, light/dark themes, and
relevant viewport sizes.

### Axum API (`backend/`)

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

`autotests = false`; unit tests mirror `backend/src/` under `backend/tests/unit/`, and cross-module
suites live under `backend/tests/integration/`. Both are included manually with `#[path]`. Use
`cargo test <name-filter>` for focused tests. Do not invent `cargo test --test <file>` commands.

Do not use `cargo run` as a routine check. Startup initializes/validates PostgreSQL and connects to Helix. Tests never reset the development database.

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

The root `./quarry web` and `./quarry desktop` launchers supervise Axum and the selected UI with
embedded templates. On macOS they start an existing Postgres.app 18 cluster if stopped, check
the configured database connection, and ensure Docker Desktop and the configured local Helix
container are ready. PostgreSQL, Docker, and Helix remain running after the launcher exits.

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
- For backend validation/authorization changes, review check placement and test early rejection
  before protected work, plus the retained state-dependent guards. Run the architecture suite
  (directly or through the full backend gate); static dependency checks alone do not prove order.
- When a template operation changes, update and test the facade and native template routes,
  headers, request bounds, owner/app scoping, and error contract.
- Add contract/adapter and route tests; run both frontend typechecks and backend tests.
- Document compatibility or rollout implications.

### When changing PostgreSQL or Helix data shape

- Define canonical ownership and stable IDs before coding.
- Make forward migration and rollback/recovery behavior explicit.
- Protect existing local data; use temporary databases in tests.
- Verify transactionality, idempotency, foreign keys, current-version uniqueness, and failure paths.
- Update the canonical architecture and recovery procedure when the operational rollout changes.

### When changing Tauri capabilities

- Prefer a narrow named command over a generic native escape hatch.
- Validate window/origin, identifiers, paths, sizes, MIME, and output.
- Scope permissions/CSP to the exact capability.
- Add Rust tests and TypeScript adapter tests.
- Verify no Tauri internals enter the web bundle.

## Security and data handling

- Treat `.env*`, local databases, API keys, tokens, absolute paths, user email, and document
  contents as sensitive. Do not print them in command output, fixtures, screenshots, or logs.
- Do not read or modify a real `.env` unless the user explicitly requests configuration work. Use
  the configuration sections of `docs/ARCHITECTURE.md` as the documented environment schema.
- Do not put provider keys or passwords in `VITE_*`, browser storage, client bundles, or Tauri
  command arguments. The user-authorized bearer-session cache in `localStorage` is a deliberate
  exception, documented in `docs/authentication.md`; never expand it to other secrets.
- Users have salted Argon2id password hashes; only public profile fields are serialized. Sessions
  store token digests in PostgreSQL. The app restores unexpired bearer sessions from origin-scoped
  `localStorage` and sends a matching `X-User-Id` consistency header; the token is the identity authority.
- User-provided API-key collection was removed. AI services use server-side `OPENAI_API_KEY`.
  Job titles grant no privileges. See `docs/authentication.md` for current limits and reset policy.
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
- Record durable decisions that change a trust boundary, runtime split, data owner, public API
  version, or destructive rollout in the canonical architecture and an appropriately scoped
  standalone document when more detail is required.
- Update commands from manifests, not memory. Never document a check that does not exist.
- Do not treat or link `plans/` files as canonical documentation.
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

## Post-feature refactoring review

After completing a feature and before handoff, use the
globally installed `$refactor` skill to look for opportunities to simplify the code
changed by the feature. Apply small, behavior-preserving improvements when they clearly improve
readability or maintainability. Keep the review scoped to the feature, preserve existing user
work, and follow this guide's verification and architecture-impact requirements for any changes.
If no useful refactor is warranted, leave the code as it is.

### Frontend review and guidance improvements

For frontend changes, also read the globally installed `$dev-refactor-frontend` skill alongside
[react-vite-development](.agents/skills/react-vite-development/SKILL.md). Keep both refactoring
skills global; do not add project-local copies. Resolve their SKILL.md paths from the current
Codex skill catalog.

Include a brief review coverage record for applicable architecture/state, accessibility and
interaction, visual consistency, performance, and verification concerns. Mark anything not
inspected or not applicable explicitly. Give confirmed findings file/line evidence, user impact,
a concrete remedy, and a disposition (fixed, deferred with a reason, or outside scope). For a
larger requested audit, map accepted findings to tasks and acceptance criteria; avoid duplicate
findings and mandatory report files for routine feature work.

Evaluate conflicting skill recommendations against Quarry's actual stack and contracts. When a
recommendation improves correctness, accessibility, maintainability, performance, or verification,
explain the evidence, tradeoffs, and checks, then update the affected Quarry guidance explicitly
in the same change before treating it as the new rule. A difference from Ring's preferred
framework or library is not itself a defect. See
[the skill evaluation](docs/frontend-refactor-skill-evaluation.md) for the adopted decisions.
