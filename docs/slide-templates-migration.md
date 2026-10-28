# Slide Templates Studio migration evidence

Current rollout (2026-10-28): schema **12** adds nine diligence stages to the password/session schema.
There is no automatic schema upgrade, status conversion, or password backfill. Use the deliberately selected,
drained development reset described in [authentication](authentication.md#schema-12-and-development-reset),
paired with a fresh isolated Helix store. Any schema-9/10 upgrade instructions below describe the
previous rollout and do not authorize running an old migration against the current schema.

Source snapshot: local `slide-template-endpoints/server`, inspected 2026-10-19.
The supplied directory has no Git metadata. The checked-in
`backend/tests/fixtures/slide_templates/source-inventory.json` records source hashes
and destination ownership before adaptation. No environment files or databases were copied.
The source manifest declares MIT; no root license/notice file was supplied. Existing
catalog art remains in the separate Diligence Studio checkout.

Dependency decisions: preserve edition 2021 and rusqlite 0.39 / bundled SQLite.
Use a named ZIP 8 dependency for the port, retaining ZIP 4 for existing parsers.
Reuse base64 0.23 and SHA-2 0.10. Preserve serde_json raw-value and arbitrary-precision
semantics; regression tests cover existing consumers after feature unification.

Validation results and operational limits are recorded below during implementation.

## Storage decision and recovery

Implemented 2026-10-20. The user explicitly selected **SQLite canonical storage with a Helix
projection**, superseding the original plan's no-Helix-write scope for this migration.

The embedded service now shares Quarry's existing durable product SQLite database and the same
`SqliteClient` connection. `backend/src/app/migrations.rs` is the sole schema authority (marker 10).
Startup takes one immediate transaction, validates schema 9, adds template tables/indexes/FTS,
validates the result and foreign keys, and advances the marker. All existing product records stay
unchanged. A conflict or validation failure rolls back DDL and marker advancement. Fresh stores
receive the complete schema directly; current layouts are validated without repair/recreation.
Unsupported schemas fail without deletion. There is no independent schema-5 initializer, template
connection, memory default, path-isolation check, or template database location setting.

Built-ins are checked-in authoritative catalog assets; all nine original checksums and
membership/deletion semantics are retained across restart. Canonical built-in rows/checksums
remain after the last app membership is deleted, preventing unchanged reseeding from restoring
that membership. Imports are physically removed after their final membership is deleted. Imports, image/preview BLOBs,
classification metadata, FTS, and available subject/capability embeddings live alongside users,
deals, chats, documents, versions, and blobs. Normal startup requires no extra database setting:
`QUARRY_DATABASE_PATH` and the existing `QUARRY_DATA_DIR` fallback select the shared file.
Previously created standalone template files remain untouched and are not automatically attached,
copied, or imported. Their former `QUARRY_SLIDE_TEMPLATES_DATABASE_PATH` setting is ignored.

The Helix projection has four labels, scoped to `Quarry_WestMonroe`:

| Label | Contents |
| --- | --- |
| `SlideTemplate` | Stable app/template key, title, description, kind, source, creation time, current generation, part count, and available classification/retrieval summary |
| `SlideTemplatePart` | UTF-8 JSON fragments of the complete template document, metadata, classification record, and preview descriptor; generation and ordered part index permit reconstruction |
| `SlideTemplateSubject` | Subject vector, model, dimensions, embedding document, and fingerprint |
| `SlideTemplateCapability` | Capability vector, model, dimensions, embedding document, and fingerprint |

Each Helix request is preflighted below 2 MiB. JSON fragments are staged in bounded pieces;
publication atomically replaces the root and both ready vector roles. Old-generation fragments
are cleaned afterward. Repeating the same publication is idempotent. The full canonical precision
is retained in the JSON classification record; Helix's native vector index receives validated
finite f32 vectors. Both roles use cosine indexes filtered by app. The classification model,
embedding model, dimensions, input/classified fingerprints, and embedding fingerprints remain
part of the projected record. Image references resolve to SQLite-owned assets; binary images are
not duplicated into Helix vectors or root properties.

SQLite commits before projection publication. A failed or uncertain Helix write leaves canonical
records intact and returns an explicit saved-in-SQLite/indexing-unconfirmed failure. No ambiguous
import/delete is automatically retried. Refresh the catalog before retrying an import: the original
record may already exist. Deletion retries also remove a stale Helix projection when SQLite has
already removed the membership. Dropped responses after commit remain uncertain outcomes.

Embedded startup reconciles only this app's template projection against the shared SQLite
catalog and republishes its records. This repairs interrupted publication and removes stale
deleted records. It never clears document labels or runs `clear_helix`. Canonical shared-database data survives failed reconciliation; startup fails
explicitly so the failure can be resolved before serving. There is no new general reindex CLI,
background indexing worker, or public template-search route. Existing internal retrieval modes
continue to use canonical SQLite classification/FTS/vector artifacts.

## Selection, classification, and cutover

The repository and launcher default to `embedded`: use ordinary `./quarry web` or
`./quarry desktop`. Explicit `QUARRY_TEMPLATE_BACKEND=diligence-studio` selects Node rollback. Embedded mode does not
start or wait for the Node template service, and does not require its port to be free. Both modes retain normal Helix startup requirements.
The renderer becomes available after Vite starts; API startup does not wait for screenshots.

Ordinary single/deck imports retain their existing behavior. Optional native
`/api/v1/templates/slide-templates/import/classified` waits for classification and both embeddings
before reporting success. A failed provider does not publish ready metadata/FTS or partial vectors.
Provider calls are bounded and never retried automatically. Classified metadata and embeddings
are then projected into Helix with the template. Built-ins and unclassified imports have no invented
embeddings. Model/key settings are server-only and disabled by default. Raw-vector HTTP inspection
requires the separate server opt-in and is absent from gallery DTOs and browser persistence.

Before upgrading a valuable schema-9 database, drain requests and stop every Quarry process
using it. Take a consistent SQLite backup (use SQLite's backup mechanism, or checkpoint and close
all connections before copying; do not copy an active WAL database's main file alone). Back up
the matching Helix store under the documented drain/backup procedure. Keep source PPTX files and
any prior standalone template databases. This implementation was tested only on disposable stores;
it has not migrated a real local database during development.

Restart on the new version to apply the transactional additive upgrade. If migration fails,
resolve the reported access/layout issue before retrying; no reset or deletion is needed. If
Helix reconciliation fails after the SQLite upgrade committed, the schema-10 database and templates
remain canonical; repair Helix availability and restart to retry reconciliation. Do not clear the
document graph. The explicit offline development reset is still destructive and now also deletes
templates, assets, previews, memberships, FTS, and embeddings; it is never a migration fallback.

To switch the selected backend, drain in-flight requests, stop the launcher, set
`QUARRY_TEMPLATE_BACKEND=diligence-studio`, and restart this version. The Node in-memory catalog
is separate; mode changes neither transfer nor dual-write imports. Embedded SQLite data remains
intact while legacy mode is selected. Legacy mode still uses Quarry migrations/schema 10 for
product data and does not publish or reconcile the embedded template projection. Switching back
to embedded reconciles its persisted catalog.

To roll back the executable to a version that only understands schema 9, drain and stop it first,
then restore the consistent pre-upgrade SQLite backup and matching Helix backup. Do not decrement
`user_version` or delete template tables by hand. Restoring an older backup loses intervening writes;
retain the schema-10 backup and source imports for a deliberate recovery. Changing vector dimensions
requires a separately reviewed template-index rollout; indexes are never reset automatically.

Chromium and the reachable hidden React renderer are still required for previews in web and
desktop development. Missing auto-discovered Chromium gives the existing preview-unavailable
behavior. A packaged/hosted renderer strategy, production identity/tenant authorization, and
production deployment remain outside this change.

## Intentional source adaptations

- Keep Quarry edition 2021, adapting let chains; declare Rust 1.94 minimum. Validation uses the
  installed Rust 1.99 compiler.
- Use Quarry's `SqliteClient`, bound builders, transaction ownership, and typed repository errors;
  all schema DDL lives in `app/migrations.rs`. Add typed row-count, FTS MATCH, and parameterized BM25 builder operations; ranking
  weights and template joins remain in the domain repository. No arbitrary SQL escape hatch is exposed.
- Use host HTTP policy and flat sanitized errors; do not import source Express route-case,
  slash, ETag/conditional-GET, HEAD/logging, or inactivity-timeout middleware. Preserve functional
  conversion, package safety, multipart, catalog, insertion, and storage assertions in adapted tests.
- Refuse legacy template schemas instead of copying source destructive derived-table migrations.
- Explicitly map the provider's snake_case schema to canonical camelCase metadata. The supplied
  source had a schema/parser mismatch; collision and valid-wire-response tests cover the correction.
- Preserve source format and retrieval tests outside production modules through manual inclusion
  hooks. The inventory identifies the original source; adapter/HTTP/repository files are adapted,
  not represented as byte-identical copies.

## Verification evidence

The working tree already contained npm dependency changes, an architecture edit, a deleted plan,
and untracked documentation/planning files. Those changes were preserved. No real environment
file, product database, or existing Helix graph was read or changed for validation.

Prior port verification (before shared-database correction), completed 2026-10-20:

| Build root | Exact command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --offline --all-targets` | Passed |
| `backend/` | `cargo test --locked --offline --all-targets` | 360 passed, 7 ignored |
| `backend/` | `cargo clippy --locked --offline --all-targets -- -D warnings` | Blocked by the pre-existing PDF parser lint below |
| `backend/` | `cargo clippy --locked --offline --all-targets -- -D warnings -A clippy::chunks_exact_to_as_chunks` | Passed |
| `backend/` | `cargo test --locked --offline live_helix_templates_metadata_dual_vectors_and_deletion -- --ignored` | 1 passed against disposable Helix on port 16969 |
| `backend/` | `cargo test --locked --offline slide_templates_headless_preview_tests -- --ignored` | 3 passed with real Chromium and synthetic loopback renderer |
| `frontend/` | `npm run typecheck` | Both web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test` | 271 tests across 68 files passed |
| `frontend/` | `npm run build:web` | Passed; existing large-chunk warnings |
| `frontend/` | `npm run check:web-bundle` | Passed after the web build |
| `frontend/` | `npm run build:desktop-ui` | Passed |
| `$HOME/diligence-studio-server/` | `npm run typecheck` | Passed |
| `$HOME/diligence-studio-server/` | `npm test` | 134 tests across 16 files passed; experimental Node SQLite warning |
| Repository root | `bash -n quarry` | Passed |
| Repository root | `./quarry --help` | Passed |
| Repository root | `git diff --check` | Passed |

The first final Rust-suite attempt inside the restricted sandbox could not bind temporary
loopback sockets or complete subprocess fixtures. The authorized rerun passed all 360 enabled
cases. It did not use existing stores or start the skipped walkthrough API.

The unmodified PDF parser at `backend/src/domains/documents/formats/pdf.rs:451` triggers
Rust 1.99's `clippy::chunks_exact_to_as_chunks` under `-D warnings`. The scoped Clippy run
allows only that pre-existing lint; no broad warning suppression or unrelated PDF edit was added.

The live Helix case verifies complete template reconstruction, metadata and preview descriptors,
both vector roles, dimensions/model, repeated publication, and deletion. The Chromium cases
verify injected input, external-request blocking, screenshot bounds, unavailable readiness,
and cancellation/browser cleanup. They use a synthetic renderer, so they do not establish
visual fidelity with Quarry's actual React render page. The separate full-browser background-tab
animation test remains ignored. The other ignored cases include explicit infrastructure,
manual fixture generation, and the disposable walkthrough-server harness; ignored is not passed.

The user explicitly elected to skip the browser walkthrough after automatic approval review
timed out twice when starting its disposable API. Existing web/desktop gallery, editor, import,
delete, export, keyboard/focus, and native-save workflows were therefore not manually exercised.
No real OpenAI request was made; fake providers cover the classification/dual-embedding pipeline.
No Tauri source or contract changed, so Tauri packaging/native runtime checks were not run.
Compilation and integration evidence do not establish the plan's full UI parity acceptance gate.

Review covered correctness, readability, dependency direction, security, and resource bounds.
Template FTS joins/ranking remain domain-owned; the shared adapter exposes typed MATCH/BM25
operations with bound values. No newly unreachable implementation was identified. The source's
background worker and internal retrieval capabilities are intentionally retained but not started
or exposed as public search. Runtime UI acceptance remains the review limitation.

The owned disposable Helix container was stopped and removed after validation; the existing
local development Helix container was left untouched. No walkthrough API or Vite server was
started. Architecture impact: updated `docs/ARCHITECTURE.md` for the embedded runtime,
configuration, routes, canonical template storage, Helix projection, cutover and recovery.


## Shared-database correction verification

The correction removes the standalone schema initializer and passes `sqlite.clone()` through
bootstrap. Frozen schema-9 fixture tests compare every seeded product column before/after upgrade,
force a mid-DDL conflict and check rollback/marker preservation, and reject damaged current layouts.
Persistence tests reopen one shared file to check imports, assets, previews, metadata, FTS, both
vectors, product coexistence, and repeated seeding/deleted built-ins. Projection-failure tests
check persisted imports across failed publication, failed reconciliation, and reopen.

Correction checks completed 2026-10-20:

| Build root | Exact command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo test --locked --offline app::migrations` | 13 passed |
| `backend/` | `cargo test --locked --offline shared_database_reopens_import_assets_preview_metadata_and_both_vectors` | 1 passed |
| `backend/` | `cargo test --locked --offline app::bootstrap::tests` | 2 passed, 1 opt-in harness ignored |
| `backend/` | `cargo test --locked --offline --all-targets` | 364 passed, 7 ignored; authorized run permits disposable loopback/subprocess fixtures |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --offline --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --offline --all-targets -- -D warnings` | Only the pre-existing PDF parser lint at `formats/pdf.rs:451` |
| `backend/` | `cargo clippy --locked --offline --all-targets -- -D warnings -A clippy::chunks_exact_to_as_chunks` | Passed |
| Repository root | `bash -n quarry` | Passed |
| Repository root | `./quarry --help` | Passed; default Rust / explicit Node rollback documented |
| Repository root | `git diff --check` | Passed |

Initial focused runs exposed and corrected test fixture expectations and the built-in last-membership
reseed bug. A restricted-sandbox template-suite run also failed its existing loopback provider fixture;
the authorized full rerun passed. No production API, user database, standalone template database, or
existing Helix instance was started, read, reset, or migrated. All database tests use disposable stores.
The schema-9 fixture matches the previous product DDL exactly; template DDL was transferred without
semantic changes except removing `IF NOT EXISTS` so conflicts fail atomically.

Review verdict: approve for this correction, with no unresolved required findings. Correctness,
dependency direction, shared-client ownership, blocking-pool use, query parameterization, resource
bounds, and recovery were reviewed. The independent schema initializer, path-isolation helper, and
separate template-path field were removed; no new unreachable code was identified.

Architecture impact: updated `docs/ARCHITECTURE.md` and `AGENTS.md` for shared schema 10,
additive migration, default embedded mode, built-in deletion persistence, and recovery behavior.
Prior port results above are historical evidence, not reruns for this correction. Frontend/Tauri API contracts and Node sources
are unchanged; browser walkthrough, native save, real providers, and live Helix/Chromium are not
re-exercised for this storage/configuration correction.


## Browser validation and follow-up fixes

A subsequent user-authorized browser pass on 2026-10-20 used Vite on port 1421, the production-
composed disposable API harness on port 3002, a temporary shared SQLite database, and a fresh
Helix container on port 16969 without host volumes. Existing listeners on 1420/3001 and the
normal Helix container were left untouched. The harness now accepts test-only
`QUARRY_VALIDATION_API_PORT`, `QUARRY_VALIDATION_DATABASE_PATH`,
`QUARRY_VALIDATION_CORS_ORIGINS`, and `QUARRY_VALIDATION_RENDER_URL` overrides, allowing isolated
ports and database reuse across controlled restarts without reading environment files.

The pass found and fixed three issues:

- Gzip responses could repoll an exhausted `unfold` admission stream and panic. Fusing the
  stream preserves admission ownership and makes end-of-body polls safe. A focused gzip
  regression test reproduced the panic before the fix and passes after it.
- Cross-origin PowerPoint export returned HTTP 200 but the browser could not read the required
  filename/warning headers. CORS now exposes `content-disposition` and
  `x-powerpoint-warning-count` to already-allowed origins, with an actual export regression test.
- At 390 px, long gallery import labels crowded out the title/back control and clipped the
  deck button. Compact visible Slide/Deck labels and a hidden small-screen subtitle keep both
  controls available; accessible import names stay unchanged.

Observed browser results:

| Workflow | Outcome |
| --- | --- |
| Disposable profile and deal setup | Passed |
| Built-in gallery, real previews, keyboard carousel navigation | Passed after gzip fix |
| Invalid PPTX feedback and disabled in-flight controls | Passed |
| Multi-slide file in single-slide mode | Explicit deck-import guidance; catalog unchanged |
| Real single-slide PPTX import | Passed; Chromium captured the actual hidden React renderer |
| Two-slide deck import | Passed; two templates appeared, total catalog reached 12 |
| Carousel across the second API page | Passed; reached final slide with Next disabled |
| Built-in deletion and API restart | Deleted built-in stayed absent; all three imports survived |
| Editor text editing and keyboard commit | Passed |
| Browser PowerPoint export | Success feedback and actual download after CORS fix; ZIP validity and edited slide text verified |
| Empty gallery and keyboard-triggered import from empty state | Passed |
| 1280 px, 900 px, and 390 px layout inspection | Passed after compact mobile header fix |
| Browser console warnings/errors in final import flow | None captured |

Native confirmation handling is a browser-automation limitation: one built-in deletion completed,
but a later attempt to inspect/cancel the native confirmation blocked CDP focus commands. That
cancellation path is not claimed as verified. The blocked test tab was closed. Dark mode was not
forced: `DARK_THEME_ENABLED` is false in the current product. Native Tauri dialogs/save, packaged
renderer provisioning, real OpenAI classification, and visual PowerPoint fidelity in Office were
not exercised. The successful export ZIP/content check does not replace an Office render review.

Follow-up automated checks:

| Build root | Exact command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo test --locked --offline compressed_preview_response_finishes` | Passed after reproducing failure before fix |
| `backend/` | `cargo test --locked --offline slide_templates_integration_tests` | Passed |
| `backend/` | `cargo test --locked --offline quicklook_` | 4 passed |
| `backend/` | `cargo test --locked --offline --all-targets -- --test-threads=1` | 366 passed, 7 ignored |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --offline --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --offline --all-targets -- -D warnings -A clippy::chunks_exact_to_as_chunks` | Passed; same previously documented PDF lint exclusion |
| `frontend/` | `npm test -- tests/components/deal-room/DeliverablesHeader.test.tsx tests/components/deal-room/DeliverableTemplatesView.test.tsx` | 25 passed |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test` | 271 tests in 68 files passed |
| `frontend/` | `npm run build:web` | Passed; existing chunk-size warnings |
| `frontend/` | `npm run check:web-bundle` | Passed after web build |
| `frontend/` | `npm run build:desktop-ui` | Passed; existing chunk-size warnings |

The first parallel backend run had 364 passes and two failures in unchanged Quick Look
subprocess fixtures. Their focused rerun passed all four cases, and the complete serial suite
passed. This intermittent fixture behavior remains distinct from the two fixed HTTP regressions.
Review found no remaining required issue in these fixes and no newly unreachable code.
Architecture impact: updated the canonical architecture for safe compressed template responses,
CORS-visible export headers, and compact mobile controls.


Cleanup completed: the temporary SQLite database and sidecars, uploaded/generated test fixtures,
captured API responses, export file, and raw test logs were deleted. The isolated Helix container
was stopped and removed, with no mounted host store. The test API and Vite processes were stopped.
The product test tab was closed (discarding its session storage); a cleanup page loaded on the
same test origin with `Clear-Site-Data: "cache", "cookies", "storage"`, then that page and its
cleanup listener were closed. The original API/Vite listeners and original Helix container were
left running. No screenshot files or sample product records are retained as deliverables. This
document retains only aggregate verification results and limitations.
