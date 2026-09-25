# Diligence Studio server integration plan

Status: proposed; source-inspected 2026-09-25

Baseline: clean Quarry at `6b488a2` and clean Diligence Studio `main` at `9aea6fd`.

Source snapshot: the 106 tracked files under
`/Users/rgambhir/diligence-studio/server` at `9aea6fd` (approximately 4.3 MB). The absolute source
path is provenance for this plan only and must not become a runtime or build dependency.

Target build root: `diligence-studio-server/`

Target npm package name: `diligence-studio-server`

Primary local address: `http://127.0.0.1:43127`

Quarry upstream configuration:
`DILIGENCE_STUDIO_API_BASE_URL=http://127.0.0.1:43127/api/v1`

Preview-render dependency:
`QUARRY_TEMPLATE_PREVIEW_RENDER_URL=http://localhost:1420/_internal/template-preview`

## Outcome

Quarry contains and owns a standalone Node/Express server in
`diligence-studio-server/`. It has its own manifest, lockfile, installed dependency tree,
configuration boundary, tests, and development commands. No file in that server imports from the
old Diligence Studio repository.

The existing Quarry frontend continues to call only Quarry's Axum `/api/v1` routes. Axum continues
to call the Diligence Studio API through the existing typed `DiligenceStudioClient` and fixed
`X-App-Id: Quarry_WestMonroe` header. The root `./quarry web` and `./quarry desktop` launchers start
and supervise the new local server as a third child process and pass its versioned base URL to
Axum explicitly.

This is a relocation and operational-integration change, not a rewrite into Rust or a merge into
the Axum process. Preserve the server's current Express routes, in-memory SQLite behavior,
PowerPoint import/export implementation, checked-in template catalog, Playwright-based preview
generation, optional OpenAI classification, API error shape, request bounds, and app-scoping
behavior.

## Verified baseline and planning consequences

| Area | Live behavior | Planning consequence |
| --- | --- | --- |
| Quarry client boundary | Browser and Tauri webview call Quarry only. Axum owns `/api/v1/templates/*` and delegates through `backend/src/adapters/diligence_studio/`. | Do not point frontend code at port `43127`, add a Vite secret, or bypass Axum. |
| Quarry upstream contract | Axum uses Diligence Studio v1 preview, template, delete, raw-PPTX import/batch-import, and JSON-to-PPTX export routes. It validates status, MIME, body sizes, URLs, image bytes, filenames, and count headers. | Copy the v1 routes without changing paths, methods, headers, payloads, status codes, or pagination. Existing adapter tests are the compatibility oracle. |
| Source server | The source is a Node 22.5+ ESM Express 5 package with TypeScript 5.9, Vitest 4, Node's built-in SQLite, Playwright Chromium, and no production build script. | Keep it as an independent npm build root. Do not add it to `frontend/package.json` or create a root npm workspace. |
| Source lock ownership | The source server is one workspace inside another repository and relies on that repository's root `package-lock.json`; there is no server-local lockfile. | Create and commit a standalone target lockfile while retaining the exact resolved baseline where compatible. `npm ci` from the target directory must work without the old workspace. |
| Source storage | `SqliteTemplateRepository` opens `new DatabaseSync(':memory:')`; imported templates, assets, previews, classifications, and app registrations disappear on process restart. | Do not copy the ignored `server/data/*.sqlite*` files or describe imports as durable. Persistence is a separate future change. |
| Built-in catalog | Startup reads the tracked `src/catalog/manifest.json`, JSON templates, and PNG previews and seeds them into memory. New apps receive built-ins when registered. | Copy the complete tracked catalog with paths unchanged and test startup from the new directory. |
| Preview generation | Imports launch headless Chromium, inject the normalized slide document into Quarry's `/_internal/template-preview` route, block other origins, and screenshot the SVG surface. | Keep the renderer in Quarry's frontend. Start or verify Vite before declaring the full local stack ready, install Chromium explicitly, and preserve the hidden-route contract. |
| App scoping | The source accepts `X-App-Id` or `appId`; Quarry always sends `Quarry_WestMonroe`. App IDs isolate catalog rows but are not authentication. | Preserve both the fixed Quarry identity and the current security warning. Do not silently change the default source app ID. |
| Optional AI | Classification is disabled by default. Enabling it requires the server-held OpenAI key, classification model, and embedding model. | Keep these variables in the new server's ignored `.env`; never forward them through `VITE_*`, Tauri IPC, or Axum request payloads. |
| Root launcher | `./quarry` currently supervises only Axum and the selected UI, with process-group cleanup and readiness checks. | Extend the same lifecycle handling to the new server; do not introduce a second launcher or leave orphaned processes. |
| Documentation | Quarry describes Diligence Studio as an external optional service and in places implies imported templates are durable. | Update architecture, domain-model, onboarding, and agent guidance to describe the co-located but separately running in-memory service accurately. |

## Stable contracts

The copy must remain compatible with the existing Axum adapter at these v1 operations:

| Method | Diligence Studio path | Quarry use |
| --- | --- | --- |
| `GET` | `/api/v1/templates/previews?page=N` | Paginated built-in/imported preview gallery |
| `GET` | `/api/v1/templates/:templateId` | Hydrated canvas document for the editor |
| `DELETE` | `/api/v1/templates/:templateId` | App-scoped template removal |
| `POST` | `/api/v1/import?kind=diagram` | One-slide raw-PPTX import |
| `POST` | `/api/v1/batchImport?kind=diagram` | One template per deck slide |
| `POST` | `/api/v1/export` | Current canvas JSON to PPTX |

Preserve the following cross-process details:

- Quarry sends `X-App-Id: Quarry_WestMonroe` on every upstream request.
- Import bodies remain raw PowerPoint OOXML bytes with the PowerPoint MIME type; Quarry's browser
  multipart wrapper terminates at Axum.
- Single import returns `201` and the expected template, preview, and warning headers; batch import
  returns `201` and imported/warning count headers.
- Export returns a ZIP-signature PPTX with the PowerPoint MIME type, a safe quoted attachment
  filename, and `X-PowerPoint-Warning-Count`.
- Preview pages remain camelCase, include at most ten items, and return app-scoped relative preview
  URLs whose `appId` matches Quarry's fixed identity.
- Hydrated documents and preview responses retain `Cache-Control: private, no-store`.
- Server errors retain the nested `{ "error": { "code", "message", "requestId" } }` shape and
  sanitized logging behavior.
- `/api/v2` routes and retrieval/classification code are copied and remain server-owned, but Quarry
  does not adopt or expose those routes in this change.

## Copy boundary

Use Git's tracked file list at source revision `9aea6fd` as the inclusion boundary, not a recursive
filesystem copy. Copy the complete tracked `server/` subtree into `diligence-studio-server/`, then
make only the target-specific adaptations in this plan.

Include:

- `src/`, including handlers, routes, services, repository, PowerPoint libraries, retrieval code,
  built-in catalog JSON, and preview PNGs;
- `test/` and the test fixtures;
- `scripts/` and its CLI integration test;
- tracked `assets/` and `output.canvas.json`, because they are intentional source-repository
  artifacts even though they are not part of the HTTP runtime;
- `README.md`, `tsconfig.json`, `vitest.config.ts`, and `package.json`.

Exclude:

- `.env` and every `.env.*` file;
- `node_modules/`, `server-dist/`, coverage, Vite/Vitest caches, and `*.tsbuildinfo`;
- ignored `data/templates.sqlite*` files and all other local SQLite files;
- logs, temporary PowerPoint/ZIP extraction output, and OS metadata;
- the source repository's root workspace manifest, web application, lockfile as-is, Git metadata,
  plans, research, and agent instructions that refer to its old workspace layout.

After the copy, search all target files for the absolute source path, `../web`, workspace-only npm
commands, and imports outside the target. There must be no runtime dependency on the old checkout.

## Target structure

```text
Quarry/
├── diligence-studio-server/
│   ├── assets/                     tracked CLI/sample assets
│   ├── scripts/                    stdin/stdout PowerPoint CLIs and tests
│   ├── src/
│   │   ├── catalog/                built-in templates and previews
│   │   ├── handlers/
│   │   ├── integrations/
│   │   ├── lib/                    import/export/shared/retrieval logic
│   │   ├── repositories/
│   │   ├── routes/
│   │   ├── services/
│   │   ├── app.ts                  testable Express app factory
│   │   └── server.ts               configuration and process listener
│   ├── test/
│   ├── package.json
│   ├── package-lock.json           standalone lock owned by this build root
│   ├── tsconfig.json
│   ├── vitest.config.ts
│   └── README.md
├── backend/                        Axum product API and upstream client
├── frontend/                       React UI and preview-render route
└── quarry                          three-process local supervisor
```

There remains no root npm or Cargo workspace. `frontend/`, `diligence-studio-server/`,
`frontend/src-tauri/`, and `backend/` stay independently installable and verifiable.

## Implementation sequence

### Phase 0 — Revalidate and pin the source

1. Run `git status --short` in Quarry and in `/Users/rgambhir/diligence-studio`. Preserve any later
   user-owned changes and stop if they overlap the copy target or contract files.
2. Record both exact revisions. If the source is no longer clean or no longer at `9aea6fd`, inspect
   its server diff and deliberately choose either the pinned revision or the newer live tree; do
   not create an untraceable mixture.
3. Re-open the live source manifest/lock, server entrypoint, app factory, config, routes, handlers,
   repository, catalog seeding, preview generator, README, and tests.
4. Re-open Quarry's Diligence Studio adapter, template domain, configuration, launcher, hidden
   preview route, and relevant architecture sections.
5. Capture the source tracked file list and use it as the copy checklist.

Exit: the source revision is explicit, source and target worktrees are understood, and every file
to include or exclude has an auditable reason.

### Phase 1 — Create the standalone server build root

1. Create `diligence-studio-server/` and copy every tracked source-server file listed above while
   preserving relative paths and binary bytes.
2. Change `package.json` name from `@diligence-studio/server` to the requested exact package name
   `diligence-studio-server`; keep it private, ESM, and Node `>=22.5`.
3. Keep local scripts directly runnable from the new root:
   - `npm run dev`
   - `npm start`
   - `npm run typecheck`
   - `npm test`
   - `npm run pptx:to-json --`
4. Create a standalone `package-lock.json`. Seed it from the source repository's resolved lock
   state so the initial copy does not opportunistically upgrade packages allowed by caret ranges.
   Confirm the important resolved baseline: Express `5.2.1`, Playwright `1.63.0`, TypeScript
   `5.9.3`, Vitest `4.1.10`, OpenAI `6.49.0`, PptxGenJS `4.0.1`, and Zod `4.6.5`.
5. Run `npm ci` from the new build root. Do not place these dependencies in `frontend/node_modules`
   or rely on source-repository hoisting.
6. Install the matching Playwright Chromium runtime through the documented Playwright command.
   Treat the browser binary as a developer/runtime prerequisite, not a committed repository file.
7. Update the copied README so it describes Quarry's sibling `frontend/` preview renderer and
   target-local commands rather than the old `../web` workspace and `npm run server:*` commands.
8. Add a short provenance section naming source repository/revision without embedding an absolute
   machine path as a required location.

Exit: `npm ci`, typechecking, and tests can run entirely inside `diligence-studio-server/`, and the
old repository can be absent without changing resolution.

### Phase 2 — Preserve behavior and make only relocation adaptations

1. Keep `src/app.ts` separate from `src/server.ts`; tests continue to construct the Express app
   without binding a port.
2. Keep `.env` loading relative to the new server root. Do not read `backend/.env` or
   `frontend/.env` from this process.
3. Preserve the current defaults and validation for host, port, upload/export/preview limits,
   request timeouts, preview size, and optional classification settings.
4. Preserve `HOST` as the server's generic configuration name, but have Quarry's launcher set it
   explicitly to `127.0.0.1`; do not accept the source default `0.0.0.0` for the ordinary local
   stack.
5. Preserve the in-memory repository and startup catalog seeding exactly. Do not wire the ignored
   source SQLite database or add a file-backed path as an incidental part of the copy.
6. Preserve raw in-memory PowerPoint processing and the tests that prohibit production file I/O.
7. Preserve `QuarryTemplatePreviewGenerator` and the injected-global contract with
   `TemplatePreviewRenderPage`. Do not duplicate the renderer into the Node server.
8. Preserve request IDs, sanitized structured logs, error middleware, content-encoding rejection,
   body limits, method guards, graceful shutdown, app scoping, and external-request blocking in
   Playwright.
9. If relocation reveals a real path assumption, fix it with a module-relative URL inside the new
   package and add a focused regression test. Do not add the old absolute source path or depend on
   the current working directory.

Exit: the target server is a behaviorally equivalent standalone copy; only package identity,
documentation, and true relocation assumptions differ.

### Phase 3 — Integrate the local process topology

1. Extend the root `quarry` launcher with explicit constants for:
   - Diligence Studio host `127.0.0.1`;
   - port `43127`;
   - versioned base `http://127.0.0.1:43127/api/v1`;
   - preview renderer `http://localhost:1420/_internal/template-preview`.
2. Pass `DILIGENCE_STUDIO_API_BASE_URL` explicitly into the Axum process. Keep Axum's existing
   optional configuration behavior for manual starts, but make the root launcher use the new
   co-located service by default.
3. Start Axum and wait for `/api/v1/health` as today.
4. Start the selected web or desktop UI and wait for Vite on port `1420` to serve the internal
   preview route before declaring the renderer available. A direct route visit may show its
   expected no-input error; the readiness check is for the Vite route, not a rendered template.
5. Start `diligence-studio-server` with its host, port, and preview URL set explicitly. Probe the
   existing side-effect-free `GET /api/v1/templates/previews?page=1` with
   `X-App-Id: Quarry_WestMonroe` until it returns successfully. Do not invent a health endpoint
   solely for launcher convenience.
6. Reject startup when ports `3001`, `1420`, or `43127` are already occupied by a conflicting
   process. Never terminate a pre-existing listener.
7. Track the new server's process group alongside Axum and the UI. On any child failure or
   `INT`/`TERM`/`HUP`, stop the Diligence Studio process, UI, and Axum gracefully, escalate only for
   the exact child groups started by the launcher, wait for them, and preserve the meaningful exit
   status.
8. Update usage/startup messages to name all three runtimes and their addresses. Never print
   secrets or the contents of environment files.
9. Keep `./quarry web` and `./quarry desktop` as the only full-stack entrypoints; do not add a
   parallel script that can drift.

Exit: either launcher starts one coherent local stack, connects Axum to the co-located server, and
leaves no child process or occupied port after shutdown.

### Phase 4 — Repository hygiene and guidance

1. Extend the root `.gitignore` for target-local `.env*`, `node_modules/`, `server-dist/`, coverage,
   caches, `*.tsbuildinfo`, logs, and any future `data/` directory. Keep source/catalog JSON and PNG
   assets tracked.
2. Update root `AGENTS.md`:
   - add `diligence-studio-server/` to the repository/build-root map;
   - describe the Express/Node runtime boundary and the Quarry preview dependency;
   - prohibit browser imports, secret exposure, and accidental file-backed data copies;
   - add its focused and broad verification commands;
   - add generated/local output exclusions;
   - update root-launcher behavior and the API change checklist where relevant.
3. Update the root README repository layout, prerequisites, dependency installation, local-stack
   lifecycle, server commands, configuration files, and runtime boundaries. Document
   `npx playwright install chromium` and Node `>=22.5`.
4. Do not create an env-example file. Follow Quarry's convention of documenting environment names
   and defaults while keeping one ignored live `.env` per runtime.

Exit: future developers can install, run, test, and safely modify the new build root without the
old repository or undocumented local state.

### Phase 5 — Canonical architecture and domain documentation

Update `docs/ARCHITECTURE.md` in the same implementation change. At minimum revise:

- executive summary and system-context diagram to show four runtimes: shared React/Vite, Tauri,
  Axum, and the Node/Express Diligence Studio server;
- repository topology and independent build roots;
- root launcher lifecycle and development ports;
- template-catalog request flow and ownership;
- configuration tables for both Axum's upstream base and every Diligence Studio server variable;
- preview rendering flow from Node/Playwright back into the hidden Vite route;
- persistence/lifetime language: the catalog is seeded at startup and imported templates are
  process-local, not durable;
- trust boundaries: `X-App-Id` scopes data but is not authentication, and the internal render route
  is not protected merely by its name;
- observability, graceful shutdown, standard verification gates, known gaps, change-impact map,
  and primary-source map.

Update `docs/DOMAIN_MODEL.md` so `templates` is described as a separate co-located service-owned
ephemeral catalog rather than an unspecified external durable provider. Quarry's Axum domain still
owns only the validated product-facing facade; it does not copy templates into Quarry SQLite or
turn a template into a deal deliverable.

Exit: code and canonical documentation agree about process ownership, persistence, ports,
configuration, security, and verification.

### Phase 6 — Verification and acceptance

Run the narrow server checks first from `diligence-studio-server/`:

```sh
npm ci
npm run typecheck
npm test
```

Verify CLI behavior through the existing stdin/stdout tests. Do not pass real customer files or
write conversion output as part of routine verification.

Verify the shell and documentation:

```sh
bash -n quarry
git diff --check
```

Confirm every documented path and command exists and search for stale source-workspace references,
absolute source paths, and old claims that Diligence Studio imports are durable.

Run Quarry's existing contract gates because the new process is now the implementation behind the
already-shipped adapter:

```sh
cd backend
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

cd ../frontend
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
```

With known disposable/local Quarry data and Helix available, perform a supervised web-stack smoke
test:

1. Confirm ports `3001`, `1420`, and `43127` are free.
2. Start `./quarry web` and record the exact process groups it creates.
3. Verify Axum health, the Vite preview route, and the Diligence Studio preview-list probe.
4. Request Quarry's `/api/v1/templates/previews?page=1` and confirm built-in previews traverse
   frontend-facing Axum and the new Node server without a direct browser call to `43127`.
5. Open one template through Quarry, export its hydrated JSON through Quarry, and validate the
   resulting PPTX content type, ZIP signature, attachment filename, and warning header.
6. Import one disposable single-slide PPTX through Quarry and confirm a generated preview appears;
   this exercises Playwright against Quarry's hidden renderer. Use a synthetic/test deck only.
7. Exercise a disposable multi-slide batch import and confirm imported-count semantics.
8. Stop the launcher and verify all three recorded process groups exit and all three ports are
   free. Do not stop unrelated pre-existing processes.
9. Restart once and confirm the built-in catalog returns while prior disposable imports are gone,
   documenting the current in-memory lifetime rather than mistaking it for data loss regression.

Repeat a launcher smoke with `./quarry desktop` when the local desktop prerequisites are available,
verifying the template gallery/editor still travel through Tauri to Axum and then to the Node
server. If runtime smoke is unavailable, report it as skipped with the exact missing prerequisite;
do not substitute compilation for integration evidence.

Finally, inspect the full diff and run `git status --short`. Confirm there are no copied secrets,
SQLite files, browser binaries, installed dependencies, caches, generated build output, unrelated
source changes, or accidental frontend/Rust lockfile churn.

## Acceptance criteria

- `diligence-studio-server/` exists as an independent, private npm package named exactly
  `diligence-studio-server`.
- The target contains the complete tracked source/test/catalog dependency closure from the pinned
  source revision and contains no dependency on the old checkout.
- A committed standalone `package-lock.json` makes `npm ci` reproducible from the target directory.
- No `.env`, local SQLite database, installed dependency directory, Playwright browser binary,
  generated build output, log, or secret is committed.
- Existing Diligence Studio typechecks and tests pass from the target build root.
- The existing Quarry v1 template preview/get/delete/import/batch-import/export contracts pass
  without frontend or adapter bypasses.
- `./quarry web` and `./quarry desktop` supervise Axum, the selected UI, and the new server; Axum is
  given the exact versioned upstream URL, and cleanup leaves no owned child process behind.
- Preview generation uses Quarry's existing `/_internal/template-preview` renderer through
  Playwright with its current origin restrictions and bounds.
- Server-held OpenAI settings remain optional and never enter the browser bundle or Tauri IPC.
- Documentation accurately states that the new server's SQLite catalog is in memory and imported
  templates disappear on restart.
- Root README, `AGENTS.md`, `docs/ARCHITECTURE.md`, and `docs/DOMAIN_MODEL.md` describe the new build
  root and runtime boundary consistently.
- `git diff --check` is clean, final status is reviewed, and skipped runtime checks or remaining
  uncertainty are reported explicitly.

## Explicit non-goals

- Rewriting the Express server in Axum or moving its repository/services into `backend/`.
- Combining npm dependency trees with `frontend/` or creating a root monorepo workspace.
- Changing Quarry's browser/Tauri API contract or allowing direct client access to port `43127`.
- Making templates durable, migrating the ignored source SQLite files, or adding a production
  database/object store.
- Adding authentication, authorization, tenancy, TLS termination, rate limiting, or deployment
  infrastructure. These remain required before public production exposure.
- Adopting Diligence Studio v2 retrieval/classification routes in Quarry.
- Changing PowerPoint normalization/rendering behavior, template IDs, catalog data, app IDs,
  preview appearance, or import/export semantics during the copy.
- Deleting or modifying the original `/Users/rgambhir/diligence-studio` checkout after the copy.

## Risks and rollback

| Risk | Mitigation |
| --- | --- |
| A partial copy compiles only because of source-workspace hoisting or missing assets. | Copy the Git-tracked closure, create a standalone lock, install with `npm ci`, and run from the target directory with the old checkout absent from resolution. |
| Dependency versions drift while creating the standalone lock. | Seed from and compare against the source lock's resolved versions before accepting lockfile changes. |
| Preview imports run before Vite is available. | Make Vite route readiness part of full-stack startup and smoke-test a real synthetic import. |
| Launcher changes orphan one of three processes. | Track exact process groups, test each child-failure path, and verify all owned ports are free after cleanup. |
| Developers assume imports survive restart because the service is now in-repository. | Correct architecture/domain documentation and include restart-loss as an explicit acceptance test. |
| The service becomes reachable beyond the local machine. | Bind the launcher to loopback, retain the warning that app IDs are not authorization, and require separate security/deployment work before exposure. |
| The copied server and former source diverge later. | Treat the Quarry copy as owned code after migration, retain the source revision as provenance, and make future changes in Quarry with normal review/tests rather than recopying blindly. |

Rollback is straightforward because the runtime remains behind Axum's optional
`DILIGENCE_STUDIO_API_BASE_URL`: revert the target directory, launcher, ignores, and documentation
changes together. Manual Axum startup can omit the variable to return the existing sanitized
unavailable behavior. Do not delete or mutate the original source repository as part of rollback.
