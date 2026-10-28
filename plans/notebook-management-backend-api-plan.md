# Notebook management backend API implementation plan

Date: 2026-10-28

Status: Proposed design; no APIs, database changes, or frontend integration implemented by this document.

## 1. Goal and scope

Implement authenticated, durable CRUD for personal notebooks, their single-level folders, and rich-text notes. Every note belongs to exactly one notebook and optionally one folder in that notebook. A notebook optionally references one deal; standalone notebooks are first-class. Multiple notebooks may reference the same deal.

Use Quarry's existing SQLite client, domain composition, authentication, errors, and handwritten client contracts. Do not add an ORM, separate notebook database, general repository framework, collaboration protocol, background save queue, or Helix projection. Normal application bootstrap still requires Helix for existing features; notebook operations themselves do not require graph writes, embeddings, OpenAI, or Diligence Studio.

Initial exclusions: nested folders, cross-notebook moves, sharing, organization tenancy, source citations, attachments/uploads, soft deletion/trash, revision history, full-text/global search, manual ordering, and offline synchronization. Optimistic revision checking protects saves but does not implement collaboration or history.

## 2. Existing implementation reviewed

The current code takes precedence over older layout descriptions in agent guidance.

| Evidence | Observed pattern and consequence |
| --- | --- |
| [Backend manifest](../backend/Cargo.toml), [lockfile](../backend/Cargo.lock), [entrypoint](../backend/src/main.rs) | Axum 0.8.9, rusqlite 0.39.0, Tokio 1.53.1, Helix client 3.0.0; Rust edition 2021 and declared minimum 1.94. Preserve versions and edition. |
| [Bootstrap](../backend/src/app/bootstrap.rs), [domain registry](../backend/src/domains/mod.rs) | Domain services and repositories are constructed explicitly; routers bind private state before composition. No global `AppState` or horizontal `src/services`/`src/repository` roots. |
| [Notebook scaffold](../backend/src/domains/notebooks/mod.rs) | Comment-only, unregistered module; no existing persisted notebook, folder, or note contracts to migrate. |
| [Deal route](../backend/src/domains/deals/route.rs), [handler](../backend/src/domains/deals/handler.rs), [service](../backend/src/domains/deals/service.rs), [repository](../backend/src/domains/deals/repository.rs) | Thin handlers, domain services, concrete SQLite repository, parameterized queries, transactional aggregate changes, camelCase JSON. Deal updates currently use POST action routes; new notebook resources can use PATCH without changing deal APIs. |
| [HTTP composition](../backend/src/app/http/mod.rs), [middleware](../backend/src/app/http/middleware.rs) | Same assembled router mounted at `/api/v1` and temporary `/api`; CORS currently allows GET, POST, DELETE only. Add PATCH explicitly. |
| [Authentication middleware](../backend/src/app/http/auth.rs), [resource access](../backend/src/domains/identity/access.rs) | Bearer session plus matching `X-User-Id`; middleware checks ownership for paths named `deal_id`, not arbitrary notebook IDs or deal IDs in JSON/query. Notebook authorization must be explicit. |
| [Migrations](../backend/src/app/migrations.rs) | Shared schema marker 11; fresh database initialization or exact-version validation. Unsupported layouts fail without deletion. |
| [SQLite client](../backend/src/adapters/sqlite/client.rs), [query builder](../backend/src/adapters/sqlite/query/builder.rs) | One shared connection, blocking-pool async access, immediate transactions, bound queries, row-count results, joins, sorting, limit/offset. |
| [HTTP errors](../backend/src/app/http/error.rs), [shared errors](../backend/src/shared/error.rs) | `RepositoryError -> ServiceError -> AppError`; explicit `{ "error": "..." }`, sanitized internal failures; native extractor errors are not universally normalized today. |
| [Architecture tests](../backend/tests/integration/architecture_tests.rs), [HTTP tests](../backend/tests/integration/http_tests.rs), [migration tests](../backend/tests/integration/app/migrations_tests.rs) | Domain boundary enforcement; manually included test modules with `autotests = false`; temporary database/test application patterns. |
| [Session drafts](../frontend/src/data/notebook/notebookSession.ts), [editor schema](../frontend/src/components/notebook/notebookSchema.ts) | Current uncommitted UI has UUID drafts, blank titles, native BlockNote `blocks`, `contentSchemaVersion: 1`, and timestamps. Module-memory drafts are cleared on reload/authentication changes; there are no saved notebook/folder entities. |
| [Shared API contract](../frontend/src/contracts/quarryApi.ts), [web adapter](../frontend/src/api/httpQuarryApi.ts), [desktop adapter](../frontend/src/api/tauriQuarryApi.ts), [native API commands](../frontend/src-tauri/src/quarry_api/commands.rs) | REST contracts span both distributions. Desktop JSON relay currently offers GET/POST/DELETE, requiring a validated PATCH extension. |

Current architecture and domain documentation already distinguish the session-only UI from future notebook APIs. Preserve all existing uncommitted work. This plan does not rename draft types or change UI behavior.

## 3. Core decisions and invariants

1. `notebooks.user_id` is the sole notebook ownership authority, derived from the authenticated actor. Folder and note ownership is inherited through the notebook. Linking a deal never grants or changes access.
2. `deal_id = NULL` is valid on create and update. A supplied deal must exist and belong to the notebook owner. Existing deal archive/status does not prevent linking or editing; archived deals are retained records.
3. Folders belong to exactly one notebook and have no parent folder. Notes require a notebook even when `folder_id = NULL`.
4. A non-null note folder must belong to the note's notebook. Enforce this in both the write transaction and a composite foreign key.
5. Notebook and note IDs and folder IDs are immutable server-generated UUID v4 strings. No notebook owner transfer or notebook-ID changes in PATCH.
6. Notebook titles, folder names, and note titles need not be unique. IDs define identity; duplicate names avoid rename/move collisions and special case-folding rules.
7. Delete notebook: permanently delete its folders and notes atomically. Delete folder: preserve its notes by moving them to the notebook root atomically. Delete note: permanently delete only that note.
8. Use PATCH for partial metadata updates, note content saves, and note moves. No separate rename, move, or `/save` endpoint: each is a patch to the resource.
9. Every note PATCH requires its current `expectedRevision`. Every successful PATCH increments revision, including metadata-only changes and moves. Folder deletion increments the revisions of notes it relocates. Notebook/folder metadata uses last committed write wins in this initial release.
10. A resource's `updatedAt` tracks changes to that row, not activity in descendants. Creating/saving a note does not update notebook recency. This avoids an extra aggregate revision/activity model.

## 4. Proposed relational schema

All tables live in the existing shared product SQLite database and are owned by `app/migrations.rs`. SQL below specifies target DDL; runtime reads and writes must use `SqlBuilder` and the client, not ad hoc SQL. Persist snake_case; serialize DTOs as camelCase.

```sql
CREATE TABLE notebooks (
    notebook_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(notebook_id)) > 0),
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    deal_id TEXT REFERENCES deals(deal_id) ON DELETE RESTRICT,
    title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 300),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE notebook_folders (
    folder_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(folder_id)) > 0),
    notebook_id TEXT NOT NULL REFERENCES notebooks(notebook_id) ON DELETE CASCADE,
    name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 200),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (notebook_id, folder_id)
);

CREATE TABLE notebook_notes (
    note_id TEXT PRIMARY KEY NOT NULL CHECK (length(trim(note_id)) > 0),
    notebook_id TEXT NOT NULL REFERENCES notebooks(notebook_id) ON DELETE CASCADE,
    folder_id TEXT,
    title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 300),
    blocks_json TEXT NOT NULL DEFAULT '[]'
        CHECK (json_valid(blocks_json) AND json_type(blocks_json) = 'array'),
    content_schema_version INTEGER NOT NULL DEFAULT 1
        CHECK (content_schema_version = 1),
    revision INTEGER NOT NULL DEFAULT 1
        CHECK (revision BETWEEN 1 AND 9007199254740991),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (notebook_id, folder_id)
        REFERENCES notebook_folders(notebook_id, folder_id)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED
);

CREATE INDEX idx_notebooks_owner_recent
    ON notebooks(user_id, updated_at DESC, notebook_id);
CREATE INDEX idx_notebooks_owner_deal_recent
    ON notebooks(user_id, deal_id, updated_at DESC, notebook_id);
CREATE INDEX idx_notebooks_deal ON notebooks(deal_id);
CREATE INDEX idx_notebook_folders_notebook
    ON notebook_folders(notebook_id, folder_id);
CREATE INDEX idx_notebook_notes_notebook_recent
    ON notebook_notes(notebook_id, updated_at DESC, note_id);
CREATE INDEX idx_notebook_notes_folder_recent
    ON notebook_notes(notebook_id, folder_id, updated_at DESC, note_id);
```

The unique folder pair is required as the composite foreign-key target even though folder IDs are globally unique. A null `folder_id` bypasses only that composite relationship; the independent notebook FK remains enforced. Deferred `NO ACTION` allows a notebook's cascades to remove both notes and folders before commit, regardless of cascade order. Do not use composite `ON DELETE SET NULL`: that would also null the required `notebook_id`. Immediate `RESTRICT` on that composite FK could interfere with notebook cascades. The folder-delete operation explicitly relocates notes before deleting the folder; direct deletion of a nonempty folder without relocation fails at commit.

Same-owner deal association is a transaction-level repository invariant, following existing deal/company ownership checks. A simple deal FK proves existence, not ownership. Notebook/deal owners are immutable in the supported APIs. Any future owner-transfer feature must revisit this invariant before shipping.

Use UTC RFC 3339 timestamps generated by the server, with one timestamp per mutation transaction. Normalize timestamp precision consistently so text sorting is chronological. Empty note titles remain `""`; “Untitled note” is a client display label. Content is native BlockNote JSON, never a JSON string in API payloads. Do not store derived HTML, Markdown, previews, counts, or duplicated deal/owner data on notes.

### 4.1 Schema rollout and recovery

Recommend a narrowly scoped, additive **11 -> 12** upgrade, assuming 12 is still the next marker at implementation time. This deliberately extends current exact-marker-only startup behavior; it is a proposal, not an existing capability. It preserves authenticated accounts, documents, chats, templates, and other valuable records rather than requiring a reset for three new tables.

- Validate the supported marker-11 product and template layouts and foreign keys before upgrading. In the existing immediate startup transaction, create only notebook tables/indexes, validate the complete marker-12 layout and foreign keys, advance the marker, then commit.
- Fresh empty databases receive the full marker-12 schema directly. Reopening marker 12 validates and reuses it without recreating missing tables. Older, newer, forged, or malformed schemas continue to fail without deletion. Do not add generic migration discovery or backfill.
- Keep notebook DDL shared between fresh initialization and the additive migration to avoid drift. Update schema validation, marker assertions, and the explicit reset utility's dependency-ordered table drops/recreation. Do not execute the reset utility as part of implementation or tests against real data.
- Test that a failed upgrade rolls back DDL and marker advancement, preserving preexisting data. Exercise repeated/concurrent initialization with isolated test storage.
- Before operational rollout, drain writers and take a consistent backup using the documented recovery process. A previous binary will reject marker 12; rollback means restore the matching pre-upgrade backup and binary during a drained window. There is no automatic downgrade. Restoring a backup after new notebook writes would discard those later writes; export/preserve them first.
- No Helix schema/index changes or data clearing are required for this feature. Keep existing graph recovery requirements for unrelated data intact.

## 5. REST contract

All paths below are relative to `/api/v1`. Use the existing protected router and headers `Authorization: Bearer <session>` and `X-User-Id`. Caller-supplied `userId`, email, IDs, audit fields, and arbitrary metadata are not writable resource fields.

### 5.1 Endpoints

| Method | Path | Request / meaning | Success |
| --- | --- | --- | --- |
| POST | `/notebooks` | `{title, dealId?}`; missing/null deal means standalone | 201 Notebook |
| GET | `/notebooks` | Owner-scoped paginated list; filters below | 200 notebook page |
| GET | `/notebooks/{notebook_id}` | Notebook metadata, without embedded descendants | 200 Notebook |
| PATCH | `/notebooks/{notebook_id}` | One or both of `{title?, dealId?}` | 200 Notebook |
| DELETE | `/notebooks/{notebook_id}` | Delete notebook and all descendants | 204, no body |
| POST | `/notebooks/{notebook_id}/folders` | `{name}` | 201 Folder |
| GET | `/notebooks/{notebook_id}/folders` | Paginated folders | 200 folder page |
| GET | `/notebooks/{notebook_id}/folders/{folder_id}` | Folder metadata | 200 Folder |
| PATCH | `/notebooks/{notebook_id}/folders/{folder_id}` | `{name}`; rename is the current folder update | 200 Folder |
| DELETE | `/notebooks/{notebook_id}/folders/{folder_id}` | Relocate notes to root, then delete folder | 204, no body |
| POST | `/notebooks/{notebook_id}/notes` | `{title?, folderId?, blocks?, contentSchemaVersion?}` | 201 Note |
| GET | `/notebooks/{notebook_id}/notes` | Paginated summaries; optional folder/title filters | 200 note page |
| GET | `/notebooks/{notebook_id}/notes/{note_id}` | Full note including blocks and revision | 200 Note |
| PATCH | `/notebooks/{notebook_id}/notes/{note_id}` | `{expectedRevision, title?, folderId?, blocks?, contentSchemaVersion?}` | 200 Note |
| DELETE | `/notebooks/{notebook_id}/notes/{note_id}` | Delete note; no body or revision precondition | 204, no body |

POST responses should include the canonical relative `Location` header. Browser adapters can use response IDs; exposing `Location` via CORS is unnecessary unless a consumer actually reads it. Resource GET/PATCH/POST responses are direct objects, not `{success, data}` wrappers. Existing APIs use varied list shapes; the new bounded lists deliberately use entity-named arrays with pagination metadata.

DELETE is idempotent in effect: repeated deletion returns 404 after the first 204. Notebook/folder deletes apply to their state at commit; note DELETE deliberately removes the current note regardless of revision. Saving never recreates a deleted note. No separate PUT replacement contract is needed for partial saves.

### 5.2 Resource payloads

Notebook response:

```json
{
  "notebookId": "23a8b5ea-18ca-4bd7-973e-44e09b7c81dc",
  "title": "Research notes",
  "dealId": null,
  "createdAt": "2026-10-28T15:00:00.000Z",
  "updatedAt": "2026-10-28T15:00:00.000Z"
}
```

Folder response:

```json
{
  "folderId": "adf49cb7-e5db-4dde-8914-39e31c17c57f",
  "notebookId": "23a8b5ea-18ca-4bd7-973e-44e09b7c81dc",
  "name": "Site visit",
  "createdAt": "2026-10-28T15:00:00.000Z",
  "updatedAt": "2026-10-28T15:00:00.000Z"
}
```

Full note response (summary responses omit only `blocks`):

```json
{
  "noteId": "014d82fc-6d4d-42f8-9f42-95f41b8f0f72",
  "notebookId": "23a8b5ea-18ca-4bd7-973e-44e09b7c81dc",
  "folderId": null,
  "title": "",
  "blocks": [],
  "contentSchemaVersion": 1,
  "revision": 1,
  "createdAt": "2026-10-28T15:00:00.000Z",
  "updatedAt": "2026-10-28T15:00:00.000Z"
}
```

Representative mutation bodies:

```json
{"title":"Standalone notebook","dealId":null}
```

```json
{"dealId":"DEAL-000184"}
```

```json
{"dealId":null}
```

```json
{"name":"Architecture interviews"}
```

```json
{"title":"Interview notes","folderId":null,"blocks":[],"contentSchemaVersion":1}
```

```json
{"expectedRevision":1,"title":"Revised title","blocks":[],"contentSchemaVersion":1}
```

```json
{"expectedRevision":2,"folderId":"adf49cb7-e5db-4dde-8914-39e31c17c57f"}
```

```json
{"expectedRevision":3,"folderId":null}
```

Each example is an independent request body, not a bulk operation. An empty blocks array intentionally clears note content. PATCH with content must supply `blocks` and `contentSchemaVersion` together. Supplying neither preserves content; supplying only one is 400. Create defaults are title `""`, folder null, blocks `[]`, version 1. An explicitly supplied content schema must be supported even when blocks are empty.

### 5.3 Optional fields and pagination

For nullable PATCH fields, omission means preserve, explicit null means clear, and a string means replace. Empty strings are not null IDs. A plain Serde `Option<T>` collapses missing/null; use a small notebook-local `PatchField<T> = Missing | Null | Value(T)` with default `Missing` and custom deserialization, or an equivalently tested representation. Do not assume nested `Option` automatically preserves this distinction. Non-nullable fields reject explicit null. Reject empty patches and note patches containing only `expectedRevision`.

All list endpoints accept `limit` (default 50, range 1–100) and `offset` (default 0, range 0–100000). Reject malformed, negative, repeated, unknown, or out-of-range parameters. Use entity-named arrays, for example:

```json
{"notes":[],"limit":50,"offset":0,"hasMore":false}
```

The analogous keys are `notebooks` and `folders`. Fetch at most `limit + 1` summary rows, return `limit`, derive `hasMore`, and omit total counts. Notebooks/notes sort by `updatedAt DESC`, then resource ID ascending. Folders sort by `folderId ASC` for a simple stable order. Offset pagination can skip/repeat rows under concurrent edits; clients refresh from offset zero after mutations. Do not promise snapshot pagination across separate requests.

Additional filters:

- Notebook list: absent `dealId` includes all owned notebooks; `dealId=none` selects standalone; a valid deal ID selects notebooks linked to that owned deal. The `none` sentinel cannot collide with valid `DEAL-...` IDs. A missing or foreign deal reference returns 404 rather than revealing its existence.
- Note list: absent `folderId` includes all notebook notes; `folderId=none` selects root notes; a valid folder UUID selects that notebook's folder. Resolve the folder even if no notes exist; wrong-parent/missing folders return 404.
- Note list: optional `q` is a trimmed title substring, 1–200 characters after trimming, with documented ASCII case-insensitive matching using SQLite's current capabilities. Escape `%`, `_`, and the escape character so queries are literal substrings. Extend the typed query builder narrowly with bound LIKE/ESCAPE support if needed; do not interpolate SQL or add FTS for title search. Search the server-side filtered list, not only the current client page.

Existing parent plus no matching children gives 200 with an empty array. Missing/inaccessible parent gives 404. No implicit “default notebook” is created during GET.

## 6. Mutation and transaction behavior

### 6.1 Notebook CRUD and deal association

- Create: validate title/deal syntax in handler; generate UUID and timestamp in the application layer; atomically verify optional deal ownership and insert with authenticated `user_id`.
- Read/list: query only the authenticated owner; never accept ownership hints from request bodies. Retrieve notebook metadata separately from bounded folder/note collections.
- Update: resolve notebook by ID and actor; update title and/or deal link in one transaction. Associating/reassociating checks the new deal's existence and same owner. Clearing the link does not require the old deal to be loaded and does not alter any note/folder.
- Delete: owner-check and delete the notebook in one transaction; DB cascades delete all descendants. Failures roll back the whole operation.
- Deal archiving retains links. Use `ON DELETE RESTRICT` for linked notebooks, consistent with linked assistant threads and the absence of a current permanent deal-delete route. A future permanent-delete flow must either require explicit unlinking or define its own reviewed policy; it must not silently delete personal notes. This plan does not implement deal deletion.

### 6.2 Folder CRUD

- Create/read/list/rename always scope by notebook and owner, including when a folder ID is globally unique. A folder cannot change notebooks.
- Folder deletion transaction: establish parent ownership, verify folder membership, update every contained note to `folder_id = NULL`, increment each revision and assign one `updated_at`, delete folder, commit. Content and title remain intact. Empty folders follow the same operation with zero relocated notes.
- Prefer a bound set-based update; if the builder lacks arithmetic revision assignment, add a narrow typed increment operation with tests instead of per-note reads or string expressions. Revision overflow aborts the entire mutation.
- Folder deletion racing a save/move serializes through SQLite. A previously loaded note becomes stale after relocation, so an old revision cannot restore obsolete folder assignment or overwrite content.

### 6.3 Note CRUD, save, and move

- Create validates content and parent/folder syntax before the service call. The repository transaction checks notebook ownership and optional folder membership, then inserts revision 1. A note may start empty directly under the notebook.
- GET returns full native blocks; list queries never load the blocks column. Stored malformed JSON or unsupported stored versions are internal integrity failures, not a silent empty-note fallback.
- PATCH is one atomic save for any combination of title, folder assignment, and full block document. Check parent ownership and note membership before reporting conflicts; verify supplied target folder in the same transaction.
- Read current note and compare `expectedRevision`, or use an atomic conditional update with the expected revision in the predicate. Return 409 on mismatch. Increment with checked arithmetic within the database/JavaScript-safe integer bound and return the committed note with its new revision. Validation, FK, and stale-write failures leave all fields untouched.
- Moves within a notebook preserve note ID, title, blocks, creation time, and content schema. They change folder, updated time, and revision. Null moves to root. Foreign-notebook folder assignment is 404 and never causes an implicit notebook move.
- DELETE verifies owner and full parent path before deleting. Missing/deleted notes return 404; PATCH does not upsert.
- A 200 save acknowledges committed SQLite state, not request admission or an in-memory cache. If a timeout/network failure occurs after commit, retrying the old revision may yield 409. Client refetches and compares against its pending snapshot; it must not blindly retry with a newer revision.
- POST create is not automatically retry-safe with server-generated IDs. Prevent duplicate UI submissions and reconcile uncertain outcomes through list/refetch. A client operation-ID deduplication scheme can be added later if actual retry requirements justify it.

### 6.4 Atomicity implementation detail

Use `SqliteClient` async offloading for all SQLite work, with owned inputs moved into the blocking closure. Never hold a synchronous lock/connection across `.await` or construct infrastructure in services.

The existing `transaction_async` commits any closure result returned as `Ok(...)`; returning `Ok(Err(RepositoryError::...))` after writes would commit partial work. For notebook transactions, prefer a small async typed-error counterpart to the existing synchronous `transaction_result`, preserving `E: From<SqliteClientError>`, or use an existing helper only if it demonstrably propagates all failures before commit. Add a regression test that injects a domain failure after relocation/update and verifies rollback. Do not refactor unrelated repositories as part of this feature.

## 7. Validation, authorization, and errors

### 7.1 Boundary validation

Keep HTTP syntax validation in handlers or notebook-local DTO conversion helpers invoked by handlers. Services enforce trusted-state use-case rules; repositories enforce transactional relationships and storage invariants.

| Input | Proposed rule |
| --- | --- |
| Notebook/folder/note IDs | Canonical UUID strings; reject malformed paths before service calls. |
| Deal ID | Existing `DEAL-` prefix, maximum 64 bytes, ASCII alphanumeric/`-`/`_` convention. |
| Notebook title | Trim, 1–300 Unicode scalar values, reject control characters. |
| Folder name | Trim, 1–200 Unicode scalar values, reject control characters. |
| Note title | Trim; empty allowed, maximum 300 Unicode scalar values, reject control characters. |
| Ownership/audit fields | Not writable; reject unknown top-level JSON fields with `deny_unknown_fields`. |
| Revision | Required on note PATCH; integer 1–9007199254740991; checked increment. |
| Content version | Only integer 1; reject unsupported versions rather than reinterpret content. |
| Request body | Per-notebook JSON write limit 2 MiB, checked against actual streamed bytes, including requests without Content-Length. |
| Blocks | At most 1 MiB serialized UTF-8 content, array root; validate bounded tree shape before persistence. |

Bound content traversal to 32 nested block levels and 10000 total JSON nodes across blocks, inline content, tables, styles, and children. Accept only the current editor's paragraph, heading, bulletListItem, numberedListItem, checkListItem, quote, codeBlock, and table blocks. Validate required keys/types, allowed properties, inline text/link/style variants, table structure, and unique nonempty block IDs. Preserve supported values exactly; reject unsupported blocks and invalid shapes. Use representative serializations from the installed editor as contract fixtures so Rust validation neither rejects valid supported documents nor invents a different rich-text model. This does not require importing BlockNote into Rust.

Allow HTTP(S)/mailto links using URL parsing and the existing safe-URL policy; reject executable schemes, credentials where disallowed, attachment/file paths, raw HTML blocks, and image/audio/video/file blocks. Never fetch note URLs on the backend. Renderers must continue their own contextual escaping and URL checks; storing JSON is not HTML sanitization. Do not trim or normalize user body text.

Reuse existing crates and bounded parsing; no unsafe code, new runtime, schema-validation dependency, or nightly/edition upgrade is required. Use validated ID types where they prevent ID mixups, typed request records rather than maps of arbitrary fields, and explicit `Result`/`Option`/patch enums. Avoid panics, production unwraps, blanket defaults that hide corrupt records, or cloning whole block trees for convenience. These choices apply the requested Rust skill within Quarry's stricter local boundaries and test placement.

### 7.2 Ownership and failure mapping

Every route resolves `AuthenticatedUser` through existing middleware. Notebook paths have no `deal_id` parameter, so the existing middleware does not authorize them automatically. Pass actor ID explicitly through service and repository calls. Every child access must prove owned notebook plus correct parent membership; possession of a UUID is insufficient.

Check referenced deal ownership inside the notebook write transaction using canonical `deals` rows and bound queries; this follows existing repository relationship checks and requires no private deal-module import. Cross-domain service reads, if later necessary, must use public facades. Filtered deal-list reads also need ownership validation, independently of middleware path checks.

| Status | Meaning and representative envelope |
| --- | --- |
| 400 | Invalid path/query/body, null non-nullable field, unsupported content/version, empty patch: `{ "error": "invalid folderId" }`. |
| 401 | Missing/expired/revoked session or invalid auth headers; preserve existing `WWW-Authenticate`. |
| 403 | Existing session/header user-ID mismatch behavior. |
| 404 | Missing or foreign-owner notebook, wrong-parent child, or missing/inaccessible referenced deal/folder: `{ "error": "resource not found" }`. |
| 409 | Note revision mismatch: `{ "error": "note revision conflict; reload before saving" }`; future explicit restricted-deletion conflicts. |
| 413 | Actual body limit or declared note-content byte limit exceeded: `{ "error": "note content exceeds size limit" }`. |
| 415 | Unsupported request media type: `{ "error": "content-type must be application/json" }`. |
| 500 | Storage/integrity/worker failure: existing sanitized `{ "error": "internal server error" }`. |

Normalize `JsonRejection`, query/path extraction, and body-limit failures for notebook routes into these envelopes. Existing `AppError` has no 413/415 variants: add only the needed variants and mappings, or a narrowly scoped response mapping, with tests; preserve unrelated endpoint behavior. Schema/type mismatches should map to 400 for notebook routes rather than inheriting untested Axum defaults. Central timeout behavior remains HTTP 408 and is not claimed to share this JSON envelope.

Return generic 404 for cross-owner references, even if other existing domains use different conflict messages. Preserve internal error source chains without exposing SQL, notebook titles/content, credentials, user details, or upstream bodies. Log operation/request identifiers and safe error categories with tracing; skip content and credentials in instrumented arguments. Do not echo invalid note bodies into logs or responses.

## 8. Backend organization and transport integration

Start with one small domain service/repository for the aggregate; separate notebook/folder/note files only when actual size warrants it.

```text
backend/src/domains/notebooks/
  mod.rs          domain registration/facade and module declarations
  route.rs        resource routes, private NotebookHttpState { service: Arc<NotebookService> }
  handler.rs      authenticated extraction, DTO validation, status/Location mapping
  model.rs        records, summary/full DTOs, create/patch input types
  content.rs      bounded version-1 BlockNote validation
  service.rs      use cases, IDs, orchestration, typed errors
  repository.rs   owner-scoped SQLite reads and atomic mutations
```

`NotebookService` owns a concrete `NotebookRepository`, matching nearby domains. Keep fields private, visibility narrow, and expose only the facade needed by composition. No trait-per-entity framework, per-request database construction, `AppState`, ambient environment reads, or adapter imports of domain code.

Required edits during later implementation:

1. Register `notebooks` in `backend/src/domains/mod.rs`; build repository from the shared `SqliteClient` in `app/bootstrap.rs`; construct an `Arc<NotebookService>` and merge its state-bound router into the protected route group before authentication layering.
2. Add DDL/upgrade/validation/reset ownership under `app/migrations.rs`, keeping all new tables in the existing database.
3. Extend HTTP CORS methods to PATCH without broadening origins/headers unnecessarily. Verify OPTIONS preflight and authentication placement. The assembled compatibility mount will also expose these routes under `/api`; document this inherited behavior while targeting only `/api/v1` in clients.
4. Add handwritten Notebook/Folder/Note DTOs and methods to `frontend/src/contracts/quarryApi.ts`, then equivalent authenticated mapping in both API adapters. Preserve camelCase, encoded IDs/query values, optional-field omission vs null, 204 handling, and status-bearing errors including 409/413.
5. Add a narrow `quarry_api_patch` native command and matching service/client method following existing GET/POST commands. Validate main window/origin, auth, permitted API path, JSON payload size, and response errors. Register the command and update only the required native capability/permission declarations if the existing scheme requires them. Do not introduce arbitrary method/URL passthrough, broad filesystem permissions, or raw Tauri imports outside the runtime boundary.
6. Keep `VITE_API_BASE_URL` and `QUARRY_API_BASE_URL` roles intact; Tauri remains transport only. No notebook rules or persistence belong in Rust desktop code.

The API/transport work is distinct from replacing the current frontend session editor. A later UI integration must let users explicitly choose/create a notebook, handle folder selection and revision conflicts, and display saved status only after acknowledgement. Do not silently promote session drafts into server data, create a default notebook on read, or overwrite existing drafts. Preserve native BlockNote serialization and current disabled attachment behavior.

## 9. Implementation phases and acceptance gates

### Phase 1 — Lock contracts and test examples

- Recheck live notebook work, backend manifests/lockfile, schema marker, and nearest instructions before editing.
- Define models, patch-field semantics, endpoint/status matrix, validation constants, and representative supported/invalid BlockNote fixtures. Confirm examples round-trip through the installed editor.
- Capture decisions above in tests: standalone notebooks, optional links, root notes, safe folder deletion, permanent notebook deletion, same-notebook moves, and stale-save rejection.
- Acceptance: no ambiguous null/omission semantics, no assumed implicit tenancy, no competing save/move routes.

### Phase 2 — Schema and transactional repository

- Add schema and narrow marker upgrade; implement bounded owner-scoped reads and atomic notebook/folder/note mutations using the typed query builder.
- Add the smallest typed transaction/increment/LIKE builder capabilities actually missing; preserve parameter binding and rollback behavior.
- Acceptance: FK/ownership/deletion/revision invariants hold under direct constraint tests, failed writes, and concurrent mutation tests; marker-11 existing records survive the upgrade.

### Phase 3 — Service, handlers, and authenticated routes

- Add ID generation, use-case orchestration, validation, notebook-local rejection mapping, endpoint responses, and bootstrap registration.
- Add PATCH CORS support and test both canonical and inherited compatibility mounts.
- Acceptance: every endpoint enforces ownership and parent membership; saves acknowledge committed state; errors are sanitized; unknown fields cannot alter ownership.

### Phase 4 — Client and desktop transport parity

- Implement shared DTOs and adapter methods, PATCH native relay and registration, native bounds/security tests, and adapter contract tests.
- Acceptance: all planned methods can be invoked from web and desktop with matching payloads/status semantics and no transport boundary bypass. Editor/UI replacement is separately scoped and does not block independent API delivery.

### Phase 5 — Review, verification, and operational documentation

- Review correctness, security, architecture, simplicity, bounded query behavior, and rollback. Review expected tests before code and resolve required findings.
- Update canonical architecture/domain data documentation for routes, maturity, tables, schema marker, supported upgrade/recovery, CORS/PATCH relay, concurrency, limitations, and checks. Update authentication/recovery references whose exact-marker assertions change; do not rewrite unrelated documentation.
- Run applicable gates below; review `git diff --check`, final scope/status, and any generated artifacts. Preserve preexisting frontend/docs edits.
- Acceptance: behavior and transport parity complete, tests/gates pass or preexisting blockers are explicitly identified, operational rollback is documented, no real database/reset/live external calls used for verification.

## 10. Test requirements

Use temporary/in-memory SQLite and existing fake/test application construction, never `backend/data/`, `.env`, live providers, or real Helix mutations. Follow manually included unit/integration layout; adding a test file without an inclusion hook is incomplete.

### 10.1 Schema and repository

- Fresh marker-12 creation, marker-11 preservation/upgrade, reopen idempotency, concurrent initialization, malformed current layouts, unsupported markers, upgrade rollback and foreign-key checks.
- Required parent IDs and nullable relationships; no missing user/notebook/deal/folder references; composite FK rejects a valid folder from another notebook.
- Standalone/linked notebook CRUD; associate, disassociate, and reassociate; missing/foreign deal rejected atomically; duplicate titles/names allowed.
- All folder operations, empty/nonempty delete, root relocation preserving content/IDs, revisions updated, and rollback on injected failure after relocation.
- Notebook deletion with a mix of root and folder notes, including multiple nonempty folders, proving cascade/deferred-FK ordering; unrelated notebooks unaffected.
- Direct nonempty folder delete without relocation fails at commit; deal deletion restricted while linked; archive does not unlink.
- Note create/save/title-only update/content clearing/folder assignment/root move/delete; immutable notebook ownership; missing/deleted records never recreated.
- Two saves from one revision: exactly one succeeds and one conflicts. Race save vs folder deletion, move vs target-folder deletion, save vs notebook/note deletion, and move vs move. No partial content/title/folder changes.
- Revision overflow, DB decode failures, storage errors, and blocking-worker failure propagation; JS-safe numeric round-trip.
- Owner scoping on every read/mutation; query results never leak foreign notebook titles/counts or note content.
- Pagination limit/offset boundaries, equal timestamp tie-breaks, empty results, folder/root/all filters, literal search escaping, and summary reads excluding blocks. No per-note/folder N+1 loading.

### 10.2 Handler/service and HTTP

- Each endpoint: expected status, JSON casing/shape, POST Location, 204 empty body, GET/list behavior and deleted-resource 404.
- Missing/expired/revoked credentials, matching/mismatching `X-User-Id`, two independent owners, forged ownership fields, and wrong-parent resource IDs.
- Missing/null/value patch distinction for both optional relationships; reject null title/name/blocks, empty patch, unknown/duplicate JSON fields, revision-only patch, incomplete blocks/version pair, and unsupported version.
- Malformed IDs/queries, repeated query keys, empty/oversized/control-character labels, and boundary Unicode lengths. Invalid transport input does not invoke mutation services.
- Native BlockNote supported types round-trip losslessly, nested/table/list content, duplicate IDs, invalid props/links, forbidden media/raw HTML, depth/node/byte limits, malformed JSON, missing media type, and streaming body limit without Content-Length.
- Conflict sanitization/refetch behavior; internal SQL/worker/content details absent from response and captured logs. Revoked sessions cannot keep mutating.
- PATCH CORS preflight from allowed origins, disallowed origin behavior, request-ID propagation, canonical `/api/v1`, and intentional compatibility mount.
- Full integration lifecycle: standalone notebook -> folder/root notes -> content save -> move -> root move -> deal link/unlink -> folder deletion -> notebook deletion.

### 10.3 Test locations and execution

- Unit files under `backend/tests/unit/domains/notebooks/` mirror `handler.rs`, `service.rs`, `content.rs`, `model.rs`, and `repository.rs` as applicable; include them from their owning modules with `#[cfg(test)]`/`#[path]`.
- Add `backend/tests/integration/notebook_http_tests.rs`, included from `backend/src/lib.rs`; reuse the support module patterns. Extend existing integration migration/client/query tests for new schema/helper behavior and architecture tests if new enforceable patterns are introduced.
- Extend web/desktop adapter tests for all methods, omitted vs explicit-null payloads, URL encoding, 204 handling, and status preservation. Native relay tests cover PATCH auth/origin/path/size validation and error mapping without requiring product business logic in Tauri.
- Focused backend filter: name new tests consistently with `notebook_` so `cargo test --locked notebook_` runs the feature suite. Run existing migration/query/architecture filters according to actual test names when implementing; do not invent `cargo test --test <file>` targets.

Run these backend gates from `backend/` after focused tests:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

For shared contract/adapters, from `frontend/`:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

For native PATCH changes, from `frontend/src-tauri/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

No normal `cargo run`, destructive reset, graph clearing, release packaging, or Diligence Studio checks are needed for notebook CRUD. Manual UI inspection applies if UI integration is subsequently included; use disposable runtime data and stop only processes started for that verification.

## 11. Documentation-only completion record

This deliverable is a design input under `plans/`, not a claim of implemented API/schema behavior. Existing architecture still correctly describes notebook drafts as session-only and the backend notebook domain as a scaffold. Architecture impact: none — this change adds only a proposed implementation plan; production routes, runtime boundaries, schema, and current feature maturity remain unchanged. Re-read canonical sections on notebook UI, backend composition, errors, SQLite, and verification before handoff; implementation must update them in Phase 5.

Plan validation consists of requirement coverage, local evidence/link/command checks, review against the requested Rust skill and project skills, Markdown structure, `git diff --check`, and final status review. Compilation, runtime/API tests, and database migrations are intentionally not run for this documentation-only task. Future test commands above are required implementation gates, not reported test results.

Validation performed: a Python structural check resolved all 28 local evidence links, parsed every JSON example, checked code-fence balance, and executed the proposed DDL against an isolated in-memory SQLite connection. The DDL check confirmed creation, rejection of cross-notebook folder assignments, rejection of nonempty folder deletion without relocation, successful notebook cascades with root notes and multiple nonempty folders, preservation of another notebook's folder, and a clean foreign-key check. No required review findings remain; implementation behavior and cross-runtime compatibility remain to be verified during the phases above.
