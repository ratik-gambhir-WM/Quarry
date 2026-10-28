# Granular deal diligence statuses

Date: 2026-10-27. Reviewed against the working tree: 2026-10-28. Status: proposed; planning only.

The user has explicitly accepted discarding the current development data and recreating it
manually. Use an explicit offline drop-and-recreate reset for the schema change; no existing-data
migration, export/import pipeline, status backfill, or provenance column is required. This document
does not execute that reset or change application code.

## 1. Requested outcome

Replace the broad Active and On hold stages with the six diligence steps in the supplied image.
Keep Pipeline and Closed, add **Pencils Down** as the replacement for Cancelled, and preserve
archiving. Apply the same vocabulary to creation, editing, the portfolio list, Kanban, deal
headers, and derived lifecycle/counts. Keep Quarry's theme and the current Kanban presentation.

The screenshot supplies stage names and ordering. Its prose about interviews, deliverables,
timing, QA, or report handling is reference context, not authorization to schedule meetings,
send reports, enforce deadlines, or trigger AI workflows.

## 2. Recommended status model

Use stable snake_case API/database codes and explicit UI labels:

| Order | Code | Label | Lifecycle |
| --- | --- | --- | --- |
| 0 | `pipeline` | Pipeline | Current |
| 1 | `prepare` | Prepare | Current |
| 2 | `review_plan_discovery` | Review & plan discovery | Current |
| 3 | `interviews_site_visit` | Interviews & site visit (SV) | Current |
| 4 | `debrief_flash_report` | Debrief & flash report | Current |
| 5 | `follow_ups_analysis` | Follow-ups & analysis | Current |
| 6 | `qa_readout_closeout` | QA, readout & closeout | Current |
| 7 | `closed` | Closed | Historic |
| 8 | `pencils_down` | Pencils Down | Historic |

Pencils Down means work has stopped without representing a completed diligence engagement.
It replaces Cancelled, rather than becoming another intermediate step. Both Closed and Pencils
Down remain visible in the portfolio until explicitly archived. Preserve separate terminal Kanban
lanes so stopped work is not presented as completed work.

Archive remains a separate `archivedAt` timestamp, not another value of `DealStatus`. An archived
deal retains its last stage and associated data and stays excluded from normal portfolio lists.
For example, a Pencils Down deal can later be archived without losing its stopped-work status.
The current UI's “Historic / Closed” lane groups Completed and Cancelled; split that presentation
into Closed and Pencils Down. There is no current canonical `closed` status code: it replaces
`completed` in this proposal.

Allow users to move forward, backward, skip stages, close, or mark Pencils Down through the
existing status editor. Do not make the stages a mandatory workflow, automatically advance them,
or change dates. Reopening a terminal deal means explicitly choosing a current stage. Existing
archive restrictions still apply; this work does not invent an unarchive endpoint.

## 3. Current implementation and affected contracts

| Existing source | Required work |
| --- | --- |
| [dealModel.ts](../frontend/src/data/dealModel.ts) defines `pipeline`, `active`, `completed`, `on_hold`, `cancelled`. | Replace with the new nine-value union/options; define exact labels, ordering, and terminal-status helpers. Generic `codeLabel` alone cannot preserve punctuation or SV/QA capitalization. |
| [Backend deal model](../backend/src/domains/deals/model.rs) mirrors those five statuses in Rust/serde. | Update enum, serialization, code conversion, and validation to the same nine codes. |
| [migrations.rs](../backend/src/app/migrations.rs) restricts `deals.status` with a SQLite CHECK; the current shared marker is 11. | Update the fresh-schema CHECK and compatibility marker, then recreate the selected development schema through the existing explicit reset. Do not build a data-preserving table migration. |
| [dealsView.ts](../frontend/src/data/dealsView.ts) derives Current/Historic, lane order, and groups Completed/Cancelled. | Make `closed` and `pencils_down` terminal; replace lane definitions with the ordered status catalog. |
| [dealExtraction.ts](../frontend/src/data/dealExtraction.ts) derives `complete` from Completed/Cancelled. | Audit what `complete` means for each consumer. Preserve historic/stopped display behavior without claiming Pencils Down is a successfully completed engagement. Rename or split the derived flag if consumers need that distinction. |
| [DealFieldsEditor.tsx](../frontend/src/components/hub/sidebar/DealFieldsEditor.tsx) consumes the shared options and displays “Active and completed acquisitions require exactly one target.” | Apply labels/grouping consistently and rewrite the target-rule helper for working stages/Closed; Pipeline remains the new-deal default. |
| [Deal service](../backend/src/domains/deals/service.rs) requires one target for Active/Completed acquisitions. | Replace this enum match with explicit stage policy, described below; do not accidentally drop the invariant. |
| [WorkspaceProvider.tsx](../frontend/src/app/WorkspaceProvider.tsx) excludes archived deals. | Keep archive filtering independent of stage and keep existing cache updates after edits. |
| [DealsKanban.tsx](../frontend/src/components/deals/DealsKanban.tsx) and its scoped CSS use existing lane IDs. | Render the new IDs/order and update stage-ring styling without restoring broad Active/Historic grouping. |
| [Authentication boundary](../backend/src/app/http/auth.rs) and [resource access](../backend/src/domains/identity/access.rs) authenticate sessions and check deal ownership. | Preserve bearer-session authority, matching `X-User-Id`, owner-scoped lists, and foreign-resource 404 behavior. Do not restore caller-email identity. |
| [reset_development.rs](../backend/src/bin/reset_development.rs) explicitly drops and recreates shared SQLite tables. | Reuse this offline mechanism for deliberately selected disposable data; keep ordinary startup non-destructive and pair the reset with a fresh isolated Helix store. |

The shared [QuarryApi contract](../frontend/src/contracts/quarryApi.ts) imports deal fields and
supports the existing create/update/list/get/archive operations. Keep those endpoints and camelCase
DTO fields. Raw status codes change, so this is still an API compatibility change across web,
desktop, and Axum, even if transport shapes do not change. Tauri mostly relays JSON: verify its
validators/mappings rather than assuming it has a matching Rust enum or needs a new command.
Do not modify Assistant message/run statuses with the same old strings.

Build roots: `frontend/`, `backend/`, and Tauri verification under `frontend/src-tauri/` when
native contracts/validation change. Use the project React/Vite, Axum, verification, and review
guidance, with current manifests/code taking precedence over older skill descriptions.

The backend currently uses `app/bootstrap.rs` plus `domains/deals/{route,handler,service,repository,model}.rs`.
Domain routers bind private service state before composition; there is no global `AppState` or
horizontal `src/services`/`src/repository` tree to extend. Keep transport validation in handlers,
target-party policy in the deal service, and bound SQLite writes in the repository.

Recent Notebook UI work is session-only, and the notebook API plan is still a proposal; neither
has added notebook tables or changed the deal status codes. Check actual implemented schema work
when selecting the next marker instead of reserving a version based on another plan. Preserve
the uncommitted Notebook, frontend, native export, and documentation work.

## 4. Disposable-data schema reset

Existing data does **not** need to survive this change. The user will recreate it manually.
Remove migration mapping/reporting from scope: no old-to-new row conversion, `legacy_status`
column, provenance report, copy to another database, reverse migration, or multi-version upgrade
chain. New records are created directly with the nine canonical status codes.

The current shared schema marker is **11**. Current startup initializes a genuinely empty database
or validates marker 11; it does not support a 9 -> 10 -> 11 upgrade. Select the next unused marker
at implementation time (12 if no intervening schema change has landed). Update the central
fresh-schema DDL, schema validation, reset behavior, and marker assertions together. Preserve all
unrelated table definitions, indexes, foreign keys, and authentication constraints in the recreated
schema. Old or malformed layouts must still fail normal startup without deleting or repairing data.

Use the existing [offline reset entrypoint](../backend/src/bin/reset_development.rs) and central
`reset_development` implementation rather than an ad hoc `DROP deals` script. Dropping only `deals`
would conflict with dependent tables and leave inconsistent product state. The existing reset
recreates the shared schema in the **same selected database**; a second database and a data-transfer
process are unnecessary.

The accepted discard includes profiles/password hashes and sessions, deals and company directory
records, metadata/parties/scopes, chats/messages, documents/versions/blobs, reminders, imported
templates/assets/previews/classification/embeddings, and app memberships. Built-in templates are
reseeded by normal bootstrap; user-created content is not restored. If additional tables have
actually landed by implementation time, include them in the dependency-ordered reset too. Local
data-room source files and unrelated standalone databases are outside this reset and remain untouched.

Implementation cutover:

1. Stop/drain backend writers, document jobs, launchers, and clients using the selected development
   database. Resolve its exact absolute path when carrying out the implementation; never infer a
   deletion target from this plan or run the reset during ordinary startup.
2. Run the updated reset utility once with its required `--discard <absolute-database-path>`
   argument against that explicitly selected disposable file. It must drop dependent tables,
   recreate the final schema, validate foreign keys, advance the marker, and commit atomically;
   restore FK enforcement even on failure. Do not add a new automatic reset path.
3. Pair the empty SQLite store with a **fresh isolated Helix store**, as required by the current
   [authentication reset procedure](../docs/authentication.md#schema-11-and-development-reset).
   This prevents stale document projections referencing discarded users/files. Do not reuse the
   old projection or run a global `clear_helix` against an unrelated/shared graph.
4. Start matching backend/web/desktop builds, allow built-in template reseeding/reconciliation,
   clear/reload stale client state, and register/sign in again. Recreate company/deal data manually;
   no status remapping or re-upload automation is required.
5. Verify all nine new values can be saved/read and old codes fail. A later restart must retain
   newly created data; resetting is a deliberate one-time cutover, not recurring initialization.

A pre-reset backup is optional for this deliberately disposable dataset, not a prerequisite for
the requested implementation. There is no automatic downgrade or retained-data guarantee. If a
rollback is needed, use a matching old binary/schema with another explicit disposable reset, or
an intentionally retained compatible snapshot. Tests use temporary databases and isolated/fake
dependencies; this planning task performs no reset and starts no services.

## 5. Business rules and UI behavior

- Maintain an explicit, exhaustive frontend stage catalog for labels/order/lifecycle. Mirror codes
  and a terminal/current policy in Rust; cover parity with contract tests. Do not duplicate label
  arrays across table, form, sidebar, and Kanban components.
- Keep stage separate from transaction progress. Entering Closed must not automatically set
  `actualCloseDate`; a diligence project closing does not prove a transaction has closed. Stage
  changes also do not silently set `engagementCompletedDate` or clear dates on reopening.
- Require exactly one target for the acquisition types currently covered by the target rule in
  all six working stages and Closed. Keep Pipeline permissive. For Pencils Down, allow incomplete
  acquisition parties so an early cancelled opportunity can be recorded without inventing a target.
  Preserve existing rules for other transaction types and all date validation.
- Recreated Pipeline and Pencils Down deals may lack a target. Require a valid target when saving
  them into a working stage or Closed, and retain failed edit drafts. No legacy On hold exception,
  migration repair, or fake company is needed after the reset.
- Show the same exact status labels in the portfolio status column, deal header/overview,
  sidebar editor, create form, and search. Use short helper descriptions for the six phases if
  needed, without treating screenshot timelines as mandatory SLAs.
- Kanban uses the nine ordered lanes with readable wrapping for long titles and visible counts.
  Render empty lanes too when the portfolio has deals, so the process remains visible. Keep the
  existing no-deals/no-search-results experience when no deals match at all. Use stable IDs and
  horizontal scrolling rather than squeezing nine columns into the viewport.
- Update `.deals-kanban` status-ring selectors to the new IDs; keep semantic Quarry colors and
  Current/Historic counts. Keep cards keyboard actionable and read-only: moving cards to change
  status is a separate feature. The existing controlled-board collapse issue is also separate.
- Update shipped demo fixtures under `frontend/src/fixtures/` and test fixtures separately. Do not
  hardcode new fake deal records in pages or use fixtures to mask an API/schema failure.

## 6. API compatibility and rollout

Ship this as a coordinated frontend/backend/desktop release. Backend responses emit only the new
codes after the explicit reset, and the UI exposes only the nine new options. Reject unknown values at
the HTTP boundary with sanitized errors; keep services/repositories free of transport validation.
Test enum-deserialization rejections explicitly: existing Axum extractor failures do not universally
use the `AppError` JSON envelope. Normalize deal input rejections if a consistent sanitized 400
response is part of this change rather than assuming Serde errors already follow that contract.

Use a drained cutover for this development-stage product. Stop old clients/servers before the
reset, deploy matching web and desktop clients, then reopen writes. No overlapping-client window
or retired-code aliases are required: `active`, `on_hold`, `completed`, and `cancelled` are rejected
as deal-status inputs after cutover. Old clients cannot safely read/write the new vocabulary and
must be refreshed/rebuilt. Preserve document/job/chat statuses that independently use those strings.

Keep archive idempotent and status-preserving. No changes to `/api/v1/deals/{dealId}/archive`,
new archive listing/restore features, permanent deletion, or current identity/security posture
are implied. Password authentication and bearer sessions are already implemented: the token is
the identity authority, `X-User-Id` must match it, and deal routes enforce canonical ownership.
Preserve those controls; organization tenancy and production identity lifecycle remain incomplete.

## 7. Implementation sequence and validation

1. Define canonical codes/labels/policies and fresh-schema/reset fixtures before UI edits. Inventory every
   deal-status consumer, including completion badges, legacy adapters, extraction mappings,
   fixtures, and tests; leave document/job/chat statuses untouched.
2. Update central fresh-schema DDL/marker and explicit reset plus Rust model/repository/service
   changes. Test fresh creation, deliberate reset of populated disposable storage, idempotent
   reopen, rejection of old/malformed layouts without startup deletion, reset rollback on injected
   failure, restored FK enforcement, and target/date rules. Verify valid linked records and archive
   behavior using newly created test records; no upgrade/provenance/preservation suite is required.
3. Update TypeScript contracts/catalog/selectors and both transport tests. Test all nine values
   through create/update/read mapping; extend native tests only where native handling changes.
4. Update create/edit/header/list/search/Kanban consumers and demo data. Preserve saving/error
   states, view switching, per-deal navigation, and accessible form/menu behavior.
5. Run focused tests, then broad gates. Inspect long lane titles, empty stages, horizontal scroll,
   Current/Historic counts, closed versus Pencils Down, status changes/cache refresh, and archived
   exclusion in web and desktop. Do not infer schema/reset correctness from a visual preview.
6. Update canonical documentation and carry out the coordinated rollout only when separately
   implementing this plan against deliberately selected data.

Existing focused test paths (extend these and register new backend test modules as required):

- `frontend/tests/data/dealsView.test.ts`, `frontend/tests/data/dealExtraction.test.ts`
- `frontend/tests/pages/Deals.test.tsx`, `frontend/tests/pages/DealRoomPage.test.tsx`
- `frontend/tests/components/hub/sidebar/DealFieldsEditor.test.tsx`
- `frontend/tests/components/hub/sidebar/AddDealModal.test.tsx`
- `frontend/tests/api/httpQuarryApi.test.ts`, `frontend/tests/api/tauriQuarryApi.test.ts`
- `backend/tests/integration/app/migrations_tests.rs`
- `backend/tests/integration/domains/deals/repository_tests.rs`
- `backend/tests/integration/domains/deals/service_tests.rs`
- `backend/tests/integration/deal_company_http_tests.rs`

Implementation gates, not executed for this planning task:

```sh
# frontend/
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# backend/ — run actual focused test-name filters first
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

If Tauri Rust changes, run its fmt/clippy/tests from `frontend/src-tauri/` too. Do not run backend
startup as a routine check against local data. Record and stop only verification servers started
for this task; preserve existing listeners and uncommitted work.

Acceptance: the explicit reset discards the selected development dataset and creates the final
schema without copying/backfilling it; normal startup never deletes data; all nine stages roundtrip
consistently for recreated records; old codes fail; Closed and Pencils Down remain distinct;
archiving preserves the chosen stage; newly created links/ownership remain valid; subsequent
restarts retain new records; and search, counts, forms, table, and Kanban all agree.

## 8. Architecture-impact check

Implementation must update `docs/ARCHITECTURE.md` and `docs/DOMAIN_MODEL.md` for the status catalog,
terminal/archived distinction, target policy, fresh schema/marker and explicit reset, API compatibility,
Kanban lane behavior, and rollout/recovery procedure. Update schema-marker references in
`docs/authentication.md` and relevant recovery documents when the implemented marker changes.
`DOMAIN_MODEL.md` currently says archiving
sets status to Archived; that is stale relative to code and `ARCHITECTURE.md`, which correctly
describe `archivedAt`. Correct this drift as part of the implementation, without introducing an
Archived status value.

Architecture impact for this planning task: none — only this proposal is revised; current code,
schemas, status values, and canonical architecture are unchanged.
