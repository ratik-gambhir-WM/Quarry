# Slide Templates Studio: Rust migration into Quarry

Date: 2026-10-19

Status: Proposed implementation plan; no runtime changes have been made.

## 1. Objective and scope

Integrate the existing Rust implementation in
`/Users/rgambhir/slide-template-endpoints/server` into Quarry's Axum backend as
the **Slide Templates Studio** subdomain:

```text
backend/src/domains/templates/slide_templates/
Rust ownership: domains::templates::slide_templates
HTTP namespace: /api/v1/templates/slide-templates
```

Rust identifiers use underscores; the HTTP namespace uses `slide-templates`.
This is a subdomain of `templates`, not another top-level domain, standalone
server, Cargo workspace, or Tauri backend.

The migration must provide Diligence Studio's import, export, insertion, catalog,
asset, preview, and optional classification/retrieval functionality inside the
existing Axum process. Reuse the supplied Rust mechanisms and parity fixtures;
do not replace them with another HTTP call to the external Rust project or a
Node subprocess that performs the conversion.

Keep all of `diligence-studio-server/` and
`backend/src/adapters/diligence_studio/`, including their tests and configuration.
Introduce an explicit backend selection so the current integration remains usable
through validation and rollback. Removing the old implementation requires a
later, separately authorized change.

Deliver this in reviewable stages. The first working milestone covers the five
operations already used by Quarry's UI; completion of that milestone alone does
not establish full Diligence Studio feature parity.

## 2. Findings from the current code

| Area | Verified current behavior | Migration consequence |
| --- | --- | --- |
| Quarry template entrypoints | `domains/templates/route.rs` exposes previews, hydrated document reads, deletion, multipart import, and PPTX export. | Preserve these public routes and their frontend/Tauri contracts. |
| Quarry orchestration | `TemplateService` directly owns an optional `Arc<DiligenceStudioClient>`; its handler also imports validation constants from that adapter. | Add backend dispatch behind the service and give transport limits a template-owned definition. |
| Host composition | `app/bootstrap.rs` constructs dependencies and merges feature routers. Routers bind private state; there is no global `AppState`. | Follow the live domain architecture rather than older horizontal-service examples. |
| Supplied Rust code | `endpoints/slide_templates/` already implements Axum handlers, OOXML reading/writing/insertion, normalization, SQLite catalog, assets, previews, classification, and retrieval. | Port and adapt the existing implementation, with source-to-destination traceability. |
| Supplied router | Its router installs `/api/v1` and `/api/v2`, fallback handling, Express-compatible URI/HEAD/ETag behavior, request IDs, and timeout middleware. | Do not mount the complete router under Quarry: it would duplicate versioning and override host policy. |
| Wire formats | Quarry import accepts one multipart `files` part plus `mode=single\|batch`; the Rust source's import routes accept raw PPTX with `kind=diagram\|commentary`. | Keep two deliberate HTTP mappings over one local use-case implementation. |
| Catalog lifetime | Node constructs an in-memory SQLite repository. Rust defaults to a file-backed template database, also supports `:memory:`, and uses its own schema marker 5. Quarry's product database uses marker 9. | Use an isolated template store; never initialize the source schema against Quarry's product database. |
| Preview runtime | Node uses Playwright. Rust uses native Chrome DevTools Protocol, a Chromium subprocess, and Quarry's existing hidden render page. Rust also offers QuickLook and disabled preview implementations. | Reuse the React render contract and verify browser provisioning/cancellation explicitly. Rust endpoints still require a browser for SVG screenshots. |
| Classification | Both servers implement synchronous classified import; their background worker is dormant. The Rust retrieval API is an in-process capability, not a mounted search endpoint. | Preserve the synchronous pipeline and internal retrieval behavior; do not invent a background job or public search API. |
| Evidence | The Rust source includes API, HTTP/JSON/repository/PPTX parity, cancellation/timing, and ignored browser suites. | These are migration inputs, not proof that Quarry integration already passes. |

The working tree was clean at planning preflight. No real environment files,
databases, or document stores were inspected or changed.

## 3. Stable contracts and explicit defaults

1. Browser and desktop continue to call Axum. Shared UI still uses
   `@quarry/runtime`; raw browser transport stays under `frontend/src/api/`.
2. Existing `/api/v1/templates/*` routes retain their paths, camelCase DTOs,
   pagination, encoded IDs, statuses, limits, and download behavior.
3. Existing imports remain diagram imports. Single mode rejects a multi-slide
   deck with the existing “Use Import Deck Template instead” message; it never
   retries automatically as batch. Batch publishes every slide atomically.
4. Quarry's catalog scope remains the server-selected `Quarry_WestMonroe`.
   Neither headers nor query parameters can select another application's catalog.
5. One configured backend owns all five existing operations for a process.
   Do not mix catalogs by operation, dual-write imports/deletes, or fall back to
   another backend after a failure.
6. Default behavior initially remains Diligence Studio. If its URL is absent,
   preserve the explicit unavailable capability; do not silently enable local
   templates. Embedded mode must be explicitly selected during validation.
7. Embedded storage defaults to a dedicated in-memory SQLite connection to match
   today's catalog lifetime. Support an explicitly configured, separate template
   database for callers that need the supplied Rust server's persistence.
   Document and test both lifetimes; durability is never implied for memory mode.
8. Retain current preview-unavailable behavior: an import can succeed without a
   preview and remain absent from the preview-only gallery. Classification that
   requires a preview must report the appropriate failure explicitly.

These defaults make the plan executable without a naming or routing clarification.
Changing persistence defaults or exposing additional UI actions is a separate
product decision, not a prerequisite for the Rust port.

## 4. Target module ownership

The following tree is proposed; these files do not exist yet except the current
parent template files.

```text
backend/src/domains/templates/
  mod.rs                         register slide_templates; expose a narrow facade
  handler.rs                     existing multipart/document HTTP contract
  route.rs                       existing UI-facing paths and private state
  service.rs                     existing facade; explicit legacy/local dispatch
  model.rs                       template-owned facade DTOs and limits
  slide_templates/
    mod.rs                       local service facade and route builder exports
    handler.rs                   native endpoint extraction and response mapping
    route.rs                     relative routes; bind local feature state
    model.rs                     canvas/catalog/import/export domain types
    error.rs                     typed local failures without HTTP dependencies
    service.rs                   import/export/catalog use-case orchestration
    repository.rs                template-owned persistence via SqliteClient
    repository/                  app scope and classification persistence
    catalog.rs + catalog/        checked-in built-ins and preview assets
    normalize.rs                 canvas normalization and validation
    json_contract.rs             compact/hydrated JSON compatibility
    pptx/                        OOXML reader, writer, insertion, package assets
    retrieval/                   classification input/metadata/vector/query logic
    classification.rs            synchronous classification orchestration

backend/src/adapters/slide_templates/
  mod.rs
  preview.rs + preview/          Chromium/CDP and optional QuickLook mechanisms
  providers.rs                  concrete classifier/embedder HTTP mechanisms

backend/src/app/
  config.rs                     typed backend, storage, preview, provider settings
  bootstrap.rs                  construct store/adapters/services; select backend
  slide_template_schema.rs       isolated template-schema initialization/validation

backend/tests/unit/domains/templates/slide_templates/...
backend/tests/unit/adapters/slide_templates/...
backend/tests/integration/slide_templates/...
backend/tests/fixtures/slide_templates/...
```

Keep PPTX formats and retrieval rules with their owning domain. Keep Chromium
process control and provider-specific HTTP in adapters. Adapters must not import
domain modules: their narrow input/output types belong to the adapter, with
mapping in the domain orchestration where needed. Do not build a generic plugin
framework to support these two backend choices.

`TemplateService` can dispatch through a private enum with unavailable,
Diligence Studio, and embedded variants. The local service receives constructed
dependencies from bootstrap and is shared by the facade and native endpoints.
The facade invokes it directly in process; it must never call Axum over loopback.
Existing Diligence Studio adapter internals can remain unchanged.

Refactor source `ApiError`/`StatusCode` usage out of reusable services and format
code. Use typed domain causes, Quarry's service/repository error boundaries, and
HTTP mapping only in handlers. Preserve useful cause chains and sanitized public
messages. Full presentation JSON remains schema-preserving where the current
contract allows unknown fields; strict query/operation DTOs need not imply
rejecting unknown canvas extensions.

## 5. Endpoint and compatibility map

### Existing Quarry contract: preserved

All paths below are relative to `/api/v1`.

| Method/path | Embedded execution | Required compatibility |
| --- | --- | --- |
| `GET /templates/previews?page=N` | Local catalog preview page | Same camelCase preview/pagination DTO, page size 10, ordering, validation, and safe app-scoped URLs. |
| `GET /templates/{id}` | Local lookup with hydrated assets | Same bounded presentation JSON, 404 semantics, and `private, no-store`. |
| `DELETE /templates/{id}` | Local app-scoped deletion | 204 on deletion, 404 when absent, associated-record cleanup and cross-app isolation. |
| `POST /templates/import?mode=single\|batch` | Collect the existing multipart upload; call local single/deck import with diagram kind | 201 and only `{ importMode, importedCount, warningCount }`; retain exact single-mode rejection behavior. |
| `POST /templates/export` | Local normalize, hydrate, and OOXML write | Same PowerPoint MIME, safe attachment name, warning-count header, bounded bytes, ZIP signature, and private no-store response. |

Keep 25 MiB file / 26 MiB multipart import limits, 50 MiB document/export JSON
limits, and 64 MiB PPTX output limit on these paths. Remove neither declared-size
nor actual-byte checks. Bound local output construction as well as the final
response so moving conversion in process does not introduce unbounded allocation.

### Native Slide Templates Studio contract: added in embedded mode

Use prefix **`/api/v1/templates/slide-templates`**. These routes expose the
source's additional capabilities without repurposing the existing UI routes.
They use Quarry request IDs, tracing, timeout/CORS policy, and HTTP error mapping.
Source Express transport quirks are not a second global Quarry contract.

| Source capability/path | Proposed suffix | Behavior to preserve |
| --- | --- | --- |
| `POST /api/v1/export` | `POST /export` | JSON presentation to downloadable PPTX, including warnings and branding. |
| `POST /api/v1/export/insert` | `POST /export/insert` | Multipart `presentation`, `target`, `insertAfterSlide`; preserve insertion ordering/clamping and unrelated target package parts. |
| `POST /api/v1/import` | `POST /import?kind=diagram\|commentary` | Raw PPTX single-slide import, compact JSON result and template/preview/warning headers. |
| `POST /api/v1/batchImport` | `POST /batch-import?kind=diagram\|commentary` | Raw deck import, one template per slide, stable order, atomic publication and count headers. |
| `GET /api/v1/templates` | `GET /catalog?kind=...` | Kind-filtered catalog including templates without previews. |
| `GET /api/v1/templates/previews` | `GET /catalog/previews?page=N` | Paginated PNG previews and catalog metadata. |
| `GET /api/v1/templates/{id}` | `GET /catalog/{id}` | Hydrated presentation document. |
| `GET /api/v1/import/{id}` | `GET /import/{id}` | Equivalent document lookup alias retained within the namespace. |
| `DELETE /api/v1/templates/{id}` | `DELETE /catalog/{id}` | Scoped deletion of the template and related data. |
| `GET /api/v1/templates/{id}/preview` | `GET /catalog/{id}/preview` | Bounded PNG with dimensions and correct unavailable/not-found distinction. |
| `GET /api/v1/import/{id}/assets/{assetId}` | `GET /import/{id}/assets/{assetId}` | Only assets belonging to an app-visible template, with validated media type and byte bounds. |
| `POST /api/v2/import` | `POST /import/classified` | Synchronous import/classify/embed/publish; no success before required processing finishes. |
| `GET /api/v2/templates/{id}` | `GET /catalog/{id}/retrieval?metadata=true&embeddings=true` | Source-compatible optional retrieval inspection with bounded results and explicit unavailable processing state. |

The last two suffixes carry the source v2 *functionality* within Quarry's v1
namespace. Do not globally mount `/api/v2` or promise unchanged upstream URLs.
Default inspection omits metadata/vectors; optional raw vector inspection must
remain server-controlled and must not enter activity logs, the gallery DTO, or
browser persistence. These new routes inherit the existing development-only
authorization limitation and must not be presented as production tenant security.

Preserve internal retrieval modes (`text`, `semantic`, `hybrid`, `filter`,
`similar`, `by_id`, `facets`), two embedding roles, ranking/fallback warnings,
and finding-to-slide matching as Rust service capabilities. No new public search
endpoint or agent UI is needed for parity because neither source server mounts one.

Register static native routes deliberately against `/templates/{id}` and test
route matching, encoded IDs, methods, and missing IDs. Rewrite every generated
`Location`, `Link`, preview URL, and asset URL to an actual Quarry route; do not
leave hard-coded source `/api/v1/templates` or `/api/v1/import` paths pointing at
the wrong handler. Keep existing facade URL invariants, including its one matching
`appId` query, until a coordinated contract change intentionally revises them.

## 6. Persistence, configuration, and runtime policy

### Isolated template store

- Bootstrap opens one dedicated `SqliteClient::open_in_memory()` for the default
  embedded mode; do not open a new memory connection for each request.
- If `QUARRY_SLIDE_TEMPLATES_DATABASE_PATH` is explicitly supplied, open a
  dedicated template file after validating that it is distinct from the main
  product database, including aliases/symlinks and existing file identity.
  Missing/unwritable/incompatible configuration fails clearly; never fall back
  from a requested file to memory.
- Initialize/validate this store through `app/slide_template_schema.rs`, with
  its own marker and transaction. Keep Quarry's product schema marker 9 and
  migration/reset behavior unchanged. Do not run the source's marker-5 migration
  against the main database or copy either project's local database files.
- Preserve template JSON, externalized image BLOBs, PNG previews, metadata,
  app memberships, classification/FTS state, and dual embedding artifacts as
  owned template records. Seed checked-in built-ins for `Quarry_WestMonroe`
  deliberately; do not inherit the source's default `DiligenceStudio_WestMonroe`
  scope accidentally. Match built-in checksums, order, refresh/deletion semantics.
- Port normal reads/writes to the existing SQL builder/client, with bound
  parameters and transactions. FTS5 `MATCH`/ranking and any missing conflict/join
  operation need narrow, tested builder support rather than copied ad hoc SQL
  or a public arbitrary-SQL escape hatch. Schema DDL stays in the schema module.
- Reject nonempty unknown template schemas without deleting them. If older
  template schema support is needed, add reviewed forward migrations and backup
  tests; do not blindly copy source migrations that drop derived tables.
- Batch import is all-or-nothing for template records, assets, and previews;
  deletion preserves other app memberships and removes only appropriate related
  records. Retain model/dimension/fingerprint and atomic classification/FTS
  publication invariants. Do not add Helix writes during this port.

### Typed configuration and composition

Proposed new settings are parsed only in `app/config.rs`, tested with injected
maps, and documented in the architecture when implemented:

| Setting | Proposed behavior |
| --- | --- |
| `QUARRY_TEMPLATE_BACKEND` | `diligence-studio` initially; `embedded` enables local execution and the native namespace. Reject unknown values. |
| `QUARRY_SLIDE_TEMPLATES_DATABASE_PATH` | Absent means dedicated memory store; an explicit path enables isolated file persistence. |
| `QUARRY_SLIDE_TEMPLATES_CHROMIUM_PATH` | Optional executable override resolved server-side; browser discovery is performed during composition, not inside domain code. |
| Preview settings | Reuse the meaning of `QUARRY_TEMPLATE_PREVIEW_RENDER_URL`, preview size/byte/render/time limits; move their parsing into typed backend configuration for embedded mode. |
| Classification settings | Optional disabled/OpenAI group with explicit classification/embedding models, dimensions and input/time limits; reuse backend secret wrappers and compatible HTTP infrastructure. Partial enabled configuration fails startup validation. |

Keep `DILIGENCE_STUDIO_API_BASE_URL` and the legacy service configuration intact.
Do not let standalone `PORT`, `HOST`, `SQLITE_DB_PATH`, or source `Config::from_env`
override Quarry's host configuration. Do not read the Node `.env` from Rust.
Do not add `VITE_*` secrets or a client-selectable backend/app scope.

Construct database, preview generator, classifier/embedder clients, and services
in bootstrap; propagate construction errors. Keep native adapter contracts free
of domain imports and widen architecture tests to cover the new constructors.

### Blocking work, preview, and failure behavior

- Offload ZIP/XML conversion, SQLite access, filesystem operations, CDP blocking
  sockets, and browser subprocess work. Reuse the source's cooperative
  cancellation and RAII cleanup; acquire bounded capacity before spawning work.
- Bound concurrent conversions/previews and queued admissions; retain byte,
  package-entry, expanded-ZIP, XML/JSON complexity, slide, and image limits.
  Never hold a database/mutex guard across an await or while rendering a preview.
- Connect cancellation to client disconnect, host timeout, and shutdown. Aborting
  a `spawn_blocking` handle does not stop its work: check cancellation before
  publication, stop/reap owned children, and release temporary directories.
- Preserve PNG dimensions/byte limits and unavailable-preview reporting. Reuse
  `window.__TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__`, SVG readiness markers, and
  `/_internal/template-preview`. Block external navigations/requests except the
  configured renderer origin and permitted `blob:`/`data:` resources.
- Keep browser launch and render budgets within a documented host request budget;
  do not stack the source's independent timeout middleware over Quarry's timeout.
  Map native processing failures deliberately; keep the facade's existing
  sanitized 400/404/503 behavior where callers already depend on it.
- No automatic retry or backend fallback for ambiguous imports/deletes. An
  observed cancellation before commit prevents publication; a lost response
  after commit remains an uncertain outcome and requires catalog refresh.
- Emit bounded structured diagnostics: operation, request ID, duration, safe
  counts/status. Never log presentation JSON, vectors, credentials, filesystem
  paths, or raw provider/browser errors that might contain sensitive inputs.

Rust removes Node from the conversion path, but preview rendering still needs
installed Chromium and a reachable Quarry render page. Verify this in both web
and desktop development and document a packaged/hosted renderer strategy before
claiming production preview readiness. QuickLook may remain an optional
macOS-only adapter; it is not the cross-platform default.

## 7. Implementation sequence and exit criteria

### Phase 1 — Freeze contracts and resolve port compatibility

1. Record the Rust source revision/file inventory and provenance before copying.
   Preserve attribution and inspect its license/assets rather than assuming that
   a package manifest alone supplies every redistribution notice.
2. Capture the current Node and Quarry contracts with reusable fixtures: catalog,
   app scoping, compact/hydrated JSON, headers, warnings, error cases and PPTX
   semantic output. Use committed synthetic documents only.
3. Resolve manifest incompatibilities in a small compilable slice. Both use
   Axum 0.8.9, but Quarry is edition 2021 and the source is edition 2024 with
   `rust-version = 1.94`. Adapt edition-specific syntax instead of changing the
   whole backend edition incidentally; establish a supported compiler minimum.
4. Keep one compatible `rusqlite`/`libsqlite3-sys` pair: Quarry locks 0.39.0 /
   0.37.0, while the source locks 0.40.2 / 0.38.2. Do not introduce incompatible
   native SQLite link dependencies through a path crate.
5. Audit source ZIP 8.6.0 APIs against Quarry's direct ZIP 4.6.1 use. Prefer a
   narrowly named ZIP-8 dependency for this port if adapting to ZIP 4 loses
   package safety; Quarry already locks ZIP 8 transitively. Avoid a broad parser
   upgrade. Review base64 0.22/0.23, SHA-2 0.10/0.11, reqwest features, and source
   additions such as CDP/ICU/regex/tempfile. Enable required serde_json features
   only after checking feature-unification effects on existing JSON consumers.

Exit: dependency/edition choices and semantic fixture comparisons are recorded;
no unexplained lockfile churn or runtime selection change.

### Phase 2 — Build the embedded subdomain

Port formats, normalization, JSON mapping, catalog/assets, repository,
classification and retrieval into the ownership layout above. Separate source
HTTP errors from reusable logic and source infrastructure from domain services.
Replace embedded test bodies with manually included Quarry test modules.
Add isolated schema initialization, typed config, adapter construction, resource
bounds, cancellation, and private route state. Implement the native contract in
sections 5–6 with all URLs scoped to the new namespace.

Exit: synthetic import → catalog/preview → hydrated edit document → export →
re-import → delete works in process. Batch atomicity, insertion and optional
classification/retrieval have passing focused tests. Legacy runtime behavior
remains selected by default.

### Phase 3 — Switch the existing facade without changing clients

Refactor `TemplateService` to dispatch all five operations to the selected backend.
Move facade DTOs/limits to template ownership and remove the handler's adapter
constant dependency. Keep legacy-specific header/payload validation in the
legacy adapter; validate equivalent local outcomes without inventing fake HTTP
responses. Explicitly map counts, warnings, hydration, errors and safe URLs.

Exit: the same existing frontend and Tauri requests pass with either configured
backend. Embedded operations make zero requests to port 43127 or the external
Rust server. Existing legacy adapter and service tests still pass.

### Phase 4 — Prove full parity and runtime integration

Run the parity matrix below, then inspect gallery pagination, empty/error/retry
states, deletion, single and deck imports, editor hydration, local edits and PPTX
download in both supported transports. Inspect actual slide rendering and
PowerPoint contents rather than relying only on ZIP signatures or compilation.
The frontend integration acceptance gate in section 8 is mandatory even when
the API contract and frontend implementation remain unchanged.

Update `quarry` only where necessary to pass the explicit backend choice and
preview settings. Initially retain its Node supervision in legacy mode. In
embedded mode, provide a runnable validation path that does not depend on Node
health/readiness while keeping its code and optional legacy launch path intact.
Ensure Vite's render route becomes reachable before import validation; do not
create a backend-readiness/UI-readiness cycle. Do not weaken normal Helix startup.

Exit: the frontend integration acceptance gate and applicable automated gates
pass, Chromium checks are actually run or
explicitly unresolved, and Node is not required for embedded template operations.
Do not declare full parity while a required advanced capability is deferred.

### Phase 5 — Controlled adoption and rollback

Keep the repository default legacy during implementation. Validate embedded mode
explicitly with disposable configuration, then record parity results and any
remaining limitations before proposing a default change. Retaining the adapter
does not mean silently using it after embedded failures.

Switch the entire backend mode through server configuration and restart/drain
cleanly. This selects a different catalog: existing Node imports are not
automatically available locally, and local imports are not present in Node after
rollback. Warn about this before operational cutover; transfer needed source
PPTX files explicitly or use a separately designed export/import migration.
Memory-mode imports disappear on shutdown. File-mode records remain in their
separate database and must be backed up before incompatible changes.

Exit: both execution paths remain runnable; rollback instructions and data
limitations are documented. No deletion of the Node service, adapter, or tests.

## 8. Verification plan

### Frontend integration acceptance gate

The existing API contract is expected to stay unchanged. Verify that the current
frontend actually works against the embedded Rust implementation; matching types
or passing mocked adapter tests alone does not establish integration compatibility.
Keep `QuarryApi` signatures, `/api/v1/templates/*` paths, DTO fields, multipart
names, and Tauri commands unchanged unless a demonstrated regression requires a
coordinated fix.

Run the existing adapter/component/page tests listed below, then exercise the
API-backed frontend with disposable data against each backend separately:
first Diligence Studio as the baseline, then embedded Rust. Test both the web
HTTP transport and the desktop Tauri transport using their existing adapters.
Do not use demo fixtures or mocked responses as the runtime integration evidence.

- Load and paginate the template gallery through `listTemplatePreviews`, including
  empty, loading, error/retry, and stale-response behavior.
- Open a template through `getTemplate`, validate its hydrated document with the
  existing frontend parser, render its assets, and make local editor changes.
- Import both a single slide and a deck through `importPptxTemplate`; verify the
  existing `files` multipart field, mode/count/warning DTO, gallery refresh,
  multi-slide rejection message, and preview-unavailable behavior.
- Export the edited presentation through `exportPowerPoint`; verify filename,
  MIME, warning count and usable PPTX contents through web download and desktop
  native save.
- Delete through `deleteTemplate`; verify confirmation, gallery refresh, missing
  template handling, and visible failure/retry feedback.

Inspect browser network/console and desktop IPC/backend diagnostics for unexpected
URLs, payload/response mismatches, CORS failures, and runtime errors without
capturing sensitive document content. Confirm embedded mode reaches Axum's
existing routes and succeeds without contacting Diligence Studio or the external
Rust server. Record pass/fail evidence for each workflow, platform, and backend,
including commands run and any blocked checks. This gate passes only when the
existing frontend works with embedded Rust; compilation or an unchanged contract
alone cannot satisfy it.

### Required behavioral evidence

| Layer | Required checks |
| --- | --- |
| Formats/JSON | Port source normalization and compact/hydrated reference fixtures; unknown extension fields, deep/numeric edge cases, geometry, typography, tables, nested transforms, inherited/hidden content, connectors, images and warning counts. |
| OOXML safety/insertion | ZIP64/multidisk/path/expansion rejection; invalid XML/relationships/assets; target parts and slide order preserved; filename/MIME/output bounds. Compare semantic XML/content, not nondeterministic ZIP bytes. |
| Repository/schema | Separate memory/file stores, correct app scope, fresh seeding, restarts, incompatible-schema refusal, main-database alias rejection, assets/previews cleanup, cross-app membership preservation, atomic batch rollback, concurrent reads/deletes and no partial publication. |
| Classification/retrieval | Disabled capability, partial config, bounded provider inputs, fake provider success/failure/timeouts, both finite correctly dimensioned vectors, atomic metadata/FTS publication, matching fingerprints, ranking parity and hybrid fallback warnings. Keep workers dormant. |
| HTTP | Existing facade paths/DTOs/errors unchanged; native routes/aliases/static-vs-dynamic IDs; query/header conflicts; 201/204/404; import/insertion multipart rules; URL rewrites; no external app selection; headers and cache policy; real body limits and malformed extractor failures. |
| Dispatch | All operations use exactly one backend; no dual write, self-HTTP, hidden fallback or retry. Legacy optional/unconfigured behavior remains covered. |
| Cancellation/load | Timeout/disconnect before commit, uncertain result after commit, bounded admission under load, browser/worker termination, no abandoned temporary files, response construction caps and host health responsiveness. |
| Preview | Actual Chromium plus Quarry renderer: injected input, SVG readiness, bounded PNG, blocked external requests, timeout/cancellation/child cleanup. Missing Chromium yields explicit unavailable behavior. |
| UI/platform | Existing gallery/editor/import/delete/export flows, stale completions, preview-unavailable imports, keyboard/focus and visible async feedback; web Blob download and desktop exact-route relay/atomic native save. |

Port source `tests/api.rs`, `http_parity.rs`, `pptx_parity.rs`,
`repository_parity.rs`, `repository_lifecycle.rs`, `transport_timing.rs`,
`extracted_input_boundary.rs`, and the JSON/headless parity material to suitable
Quarry suites. Classify source Express-only assertions as intentional transport
differences instead of changing Quarry globally to satisfy them. Preserve all
functional conversion/storage/security assertions. Source inline and
`retrieval/tests.rs` tests must move out of production source.

Quarry has `autotests = false`: add explicit `#[cfg(test)]`/`#[path]` inclusion
hooks for every new suite. Unit paths mirror source modules under `tests/unit`;
cross-module flows belong under `tests/integration`. Verify that test counts
include the ported cases. Use `cargo test <actual-name-filter>`, not invented
`cargo test --test <filename>` targets.

### Implementation gates — to run when code exists

From `backend/`, after focused tests:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Run existing
`backend/tests/integration/adapters/diligence_studio/templates_tests.rs`,
`backend/tests/unit/domains/templates/service_tests.rs`, and architecture tests
as part of that suite. Extend architecture enforcement for new constructors,
domain environment reads, source-tree tests, and adapter dependency direction.

From `frontend/`, because behavior must remain correct in both transports:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Relevant existing coverage includes `tests/api/httpQuarryApi.test.ts`,
`tests/api/tauriQuarryApi.test.ts`, `tests/pages/TemplatePreviewRenderPage.test.tsx`,
`tests/pages/DeliverableTemplateEditorPage.test.tsx`,
`tests/components/deal-room/TemplateDocumentStore.test.tsx`, and
`tests/components/deal-room/DeliverableTemplatesView.test.tsx`.

Keep Node regression evidence with `npm run typecheck` and `npm test` from
`diligence-studio-server/`; use `npm ci` only when its dependencies need installing.
If any native relay/path contract actually changes, also run Tauri's format,
Clippy and test gates from `frontend/src-tauri/`. Preserving existing paths should
avoid widening Tauri permissions just to expose server capabilities.

Browser/parity suites that launch Chromium must be run separately and reported
explicitly; their ignored status is not a pass. Use temporary data, fake providers,
and explicitly disposable full-stack dependencies. Never run `clear_helix`,
`reset_development`, or ordinary `cargo run` as a harmless test. Record and clean
up only processes started by the verification session.

## 9. Documentation, scope boundaries, and completion

Update [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) with each implemented stage:
domain registry and module ownership, endpoint/version map, backend selection,
template database ownership/lifetime, preview/browser dependency, optional
classification, failure mapping, trust boundary, launcher behavior and checks.
Re-read it after implementation. Record a standalone scoped decision if durable
storage or another runtime/trust boundary is changed.

The existing `plans/diligence-studio-helix-embeddings-plan.md` is also only a
proposal. It does not authorize making Helix vector migration part of this port.
Preserve the current source's SQLite/vector retrieval behavior for parity. If
the Helix proposal is implemented first, rebaseline this plan against that code
and explicitly reconcile ownership before porting storage; never maintain two
competing vector authorities by accident.

No new authentication system, public template-search UI, document graph changes,
editor persistence, root workspace, broad dependency modernization, or legacy
removal is included here. Existing missing authentication/tenant authorization
remains a documented development limitation; fixed app selection is not security
identity.

The migration is complete when all specified capabilities work inside Axum,
existing clients work unchanged against the embedded backend, parity and resource
safety are demonstrated, the legacy path remains available, and operational
data/cutover limits are documented. Review the final diff across correctness,
readability, architecture, security and performance; run `git diff --check` and
review `git status --short` before handoff.

## 10. Evidence and planning-only verification

Inspected sources:

- Rust source: `server/Cargo.toml`, workspace `Cargo.lock`, startup/config,
  `src/endpoints/slide_templates/`, and the test/fixture inventory under
  `/Users/rgambhir/slide-template-endpoints`.
- Quarry [template routes](../backend/src/domains/templates/route.rs),
  [handlers](../backend/src/domains/templates/handler.rs),
  [service](../backend/src/domains/templates/service.rs), and
  [Diligence Studio adapter](../backend/src/adapters/diligence_studio/templates.rs).
- Quarry [bootstrap](../backend/src/app/bootstrap.rs),
  [configuration](../backend/src/app/config.rs),
  [HTTP composition](../backend/src/app/http/mod.rs),
  [SQLite client](../backend/src/adapters/sqlite/client.rs),
  [migrations](../backend/src/app/migrations.rs), and
  [architecture tests](../backend/tests/integration/architecture_tests.rs).
- Node [composition](../diligence-studio-server/src/server.ts),
  [app](../diligence-studio-server/src/app.ts), routes, package manifest/lockfile,
  checked-in catalog and existing tests.
- Frontend [contract](../frontend/src/contracts/quarryApi.ts), web/desktop API
  adapters, hidden render page, package scripts, native relay and root launcher.

This change creates only this plan. Structural/path/command checks and final
diff/status review apply; runtime, Rust, Node and browser suites are not run for
this documentation-only change. Feature parity remains to be established during
implementation, particularly browser capture and PowerPoint rendering fidelity.

Architecture impact: none — this plan describes proposed work without changing
the current routes, runtimes, storage, configuration, or integration behavior.
