# Granular deal diligence statuses

Date: 2026-10-27. Status: proposed; planning only.

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
| [migrations.rs](../backend/src/app/migrations.rs) restricts `deals.status` with a SQLite CHECK. | A UI rename alone is insufficient. Replace the constraint through a data-preserving migration. |
| [dealsView.ts](../frontend/src/data/dealsView.ts) derives Current/Historic, lane order, and groups Completed/Cancelled. | Make `closed` and `pencils_down` terminal; replace lane definitions with the ordered status catalog. |
| [dealExtraction.ts](../frontend/src/data/dealExtraction.ts) derives `complete` from Completed/Cancelled. | Audit what `complete` means for each consumer. Preserve historic/stopped display behavior without claiming Pencils Down is a successfully completed engagement. Rename or split the derived flag if consumers need that distinction. |
| [DealFieldsEditor.tsx](../frontend/src/components/hub/sidebar/DealFieldsEditor.tsx) consumes the shared options. | Apply labels and grouping consistently to create/edit surfaces; Pipeline remains the new-deal default. |
| [Deal service](../backend/src/domains/deals/service.rs) requires one target for Active/Completed acquisitions. | Replace this enum match with explicit stage policy, described below; do not accidentally drop the invariant. |
| [WorkspaceProvider.tsx](../frontend/src/app/WorkspaceProvider.tsx) excludes archived deals. | Keep archive filtering independent of stage and keep existing cache updates after edits. |
| [DealsKanban.tsx](../frontend/src/components/deals/DealsKanban.tsx) and its scoped CSS use existing lane IDs. | Render the new IDs/order and update stage-ring styling without restoring broad Active/Historic grouping. |

The shared [QuarryApi contract](../frontend/src/contracts/quarryApi.ts) imports deal fields and
supports the existing create/update/list/get/archive operations. Keep those endpoints and camelCase
DTO fields. Raw status codes change, so this is still an API compatibility change across web,
desktop, and Axum, even if transport shapes do not change. Tauri mostly relays JSON: verify its
validators/mappings rather than assuming it has a matching Rust enum or needs a new command.
Do not modify Assistant message/run statuses with the same old strings.

Build roots: `frontend/`, `backend/`, and Tauri verification under `frontend/src-tauri/` when
native contracts/validation change. Use the project React/Vite, Axum, verification, and review
guidance, with current manifests/code taking precedence over older skill descriptions.

## 4. Existing-data migration

Use the following deterministic baseline, with a reviewed per-deal stage mapping for active
engagements when trustworthy stage information is available:

| Existing status | New status | Migration consideration |
| --- | --- | --- |
| `pipeline` | `pipeline` | Preserve exactly. |
| `active` | `prepare` | Conservative entry-stage default, not a claim that preparation has just started. Review existing active deals and assign their actual stage. |
| `on_hold` | `prepare` | Retired status has no exact new equivalent. Preserve its original value for review; do not silently equate paused work with Pencils Down. |
| `completed` | `closed` | Preserve all completion/transaction dates. |
| `cancelled` | `pencils_down` | Direct replacement requested by the user. Do not mark the engagement successfully complete. |

Preserve the original pre-migration status in an internal nullable `legacy_status` column for
retired codes, populated once by migration and never writable through normal deal requests.
Use it for a bounded migration review report; keep it out of normal status choices. This retains
the On hold distinction and original cancellation/completion values without introducing extra
active statuses. Report unmapped/unrecognized stored values and abort transactionally; never
guess on an unknown code. Preserve archive timestamps, IDs, ownership, dates, metadata, parties,
scopes, resources, documents/versions/blobs, and linked chats.

The current schema marker is 10 with a supported schema-9 additive template upgrade. Select the
next available marker at implementation time and compose with any intervening migration work
(including the proposed Notebook work); do not assume a particular next marker is unused.
Support fresh databases and upgrades from every currently supported starting schema. Keep schema
inspection and validation synchronized with the new CHECK and optional provenance column.

SQLite cannot directly alter this CHECK. Use a carefully tested table-rebuild migration through
the central migration owner, reconstructing indexes and all constraints. Inbound foreign keys
make naive rename/drop dangerous: test that metadata, parties, scopes, file rows, and assistant
thread references still point to the rebuilt `deals` table and that no cascade or FK rewrite
deletes/rebinds them. If foreign-key enforcement must be temporarily changed, do so only on the
isolated migration connection outside an active transaction, restore it reliably, and require
`foreign_key_check` before accepting the upgrade. Follow the actual SQLite client/migration API;
do not introduce request-time ad hoc SQL or globally disable foreign keys.

Test with disposable databases only. For real rollout, drain writes, back up the selected database,
upgrade once, verify counts/relationships/status mapping, and retain the backup. No reset utility,
global Helix clear, or deletion of valuable data is authorized by this plan. Rollback after new
stages have been written requires a reviewed reverse migration or restoring the full backup with
an explicit reconciliation of subsequent writes; old binaries cannot safely read new statuses.

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
- A migrated On hold deal might lack a target. Load/display it safely, flag the missing requirement
  when editing or advancing, and retain failed drafts; do not reject the migration or inject a
  fake company to satisfy a new write rule.
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
  hardcode new fake deal records in pages or use fixtures to mask an API/migration failure.

## 6. API compatibility and rollout

Ship this as a coordinated frontend/backend/desktop release. Backend responses emit only the new
codes after migration, and the UI exposes only the nine new options. Reject unknown values at
the HTTP boundary with sanitized errors; keep services/repositories free of transport validation.

Prefer a drained cutover for this development-stage product. Stop old clients/servers before the
migration, deploy matching web and desktop clients, then reopen writes. Do not silently accept
retired codes indefinitely. If overlapping clients are actually required, first define a bounded
compatibility window: explicitly map old request values using the table above, continue returning
canonical new codes, document the old clients' read incompatibility, and remove aliases on a
dated release. Request aliases alone cannot make an old client understand new responses.

Keep archive idempotent and status-preserving. No changes to `/api/v1/deals/{dealId}/archive`,
new archive listing/restore features, permanent deletion, or current identity/security posture
are implied. Existing caller-supplied profile identity is not production authentication.

## 7. Implementation sequence and validation

1. Define canonical codes/labels/policies and migration fixtures before UI edits. Inventory every
   deal-status consumer, including completion badges, legacy adapters, extraction mappings,
   fixtures, and tests; leave document/job/chat statuses untouched.
2. Implement and test the central SQLite migration plus Rust model/repository/service changes.
   Verify fresh/9/10 upgrades, rollback on injected failure, preservation of archived and linked
   records, provenance, and target/date rules using temporary databases.
3. Update TypeScript contracts/catalog/selectors and both transport tests. Test all nine values
   through create/update/read mapping; extend native tests only where native handling changes.
4. Update create/edit/header/list/search/Kanban consumers and demo data. Preserve saving/error
   states, view switching, per-deal navigation, and accessible form/menu behavior.
5. Run focused tests, then broad gates. Inspect long lane titles, empty stages, horizontal scroll,
   Current/Historic counts, closed versus Pencils Down, status changes/cache refresh, and archived
   exclusion in web and desktop. Do not infer migration correctness from a visual preview.
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

Acceptance: all nine stages roundtrip consistently; existing Cancelled deals become Pencils Down;
Closed and Pencils Down remain distinct; legacy data/links survive; archiving preserves the chosen
stage; and search, counts, forms, table, and Kanban all agree.

## 8. Architecture-impact check

Implementation must update `docs/ARCHITECTURE.md` and `docs/DOMAIN_MODEL.md` for the status catalog,
terminal/archived distinction, target policy, schema/migration/provenance, API compatibility,
Kanban lane behavior, and rollout/recovery procedure. `DOMAIN_MODEL.md` currently says archiving
sets status to Archived; that is stale relative to code and `ARCHITECTURE.md`, which correctly
describe `archivedAt`. Correct this drift as part of the implementation, without introducing an
Archived status value.

Architecture impact for this planning task: none — only this proposal is added; current code,
schemas, status values, and canonical architecture are unchanged.
