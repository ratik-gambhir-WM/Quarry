# Quarry versioned file persistence transaction plan

Status: proposed  
Prepared: 2026-08-21  
Branch reviewed: `feature/save-and-view-file` at `d53f455c530940bd094fd2b287b1720cea100242`  
Primary scope: `backend/` (Axum 0.8.9, rusqlite 0.39.0, Tokio 1.53.1)

## Goal

Replace the single `quarry_file_blobs(file_id, file_bytes)` record with the three-level structure shown in the supplied diagram, with every logical file owned by a specific deal:

```text
deals 1 ─── * quarry_files 1 ─── * quarry_file_versions 1 ─── 1 quarry_file_blobs
```

Refactor `persist_file_blob` so it accepts a required `deal_id`, verifies that the deal belongs to the document's user/workspace, and persists the logical file, its version metadata, and its bytes with parameterized `SqlBuilder` queries executed inside one SQLite transaction. If any ownership read, insert, update, constraint, or commit fails, none of the three file tables may retain a partial change.

This transaction boundary covers the three SQLite tables only. Helix writes in `persist_document_and_chunks` cannot participate in the same SQLite transaction and must not be described as atomic with these writes.

## Baseline findings

- `backend/src/repository/document_repository.rs` currently inserts only `file_id` and `file_bytes`, using `SqlBuilder` and `SqliteClient::write_async`.
- `backend/src/core/clients/sqlite.rs` serializes a single rusqlite connection behind `Arc<Mutex<Connection>>` and moves asynchronous calls to `spawn_blocking`, but it has no transaction API.
- `backend/src/state.rs` currently creates only the two-column blob table. `CREATE TABLE IF NOT EXISTS` cannot transform that table into the new shape.
- `DocumentNode.document_id` is derived from `(user_id, content_hash)` and is also the Helix document identity. It is a content identity, not a stable logical-file identity.
- `DocumentNode` already provides the filename, source type, optional local path, byte count, content hash, token count, and optional rendered-PDF path required to build the proposed rows.
- `infer_supported_mime_type` already maps supported extensions to MIME types.
- `deals.deal_id` is the stable deal key and `deals.user_id` references `users.id`; the upload path currently sends only a user email and drops the active deal ID even though `DataRoomPage` already has it.
- The file persistence and schema changes visible in the working tree are uncommitted user work. Implementation must preserve and reconcile those changes rather than overwrite them.

## Design decisions

### Separate logical file, version, and graph identities

Use three distinct identifiers:

| Identity | Meaning | Source |
|---|---|---|
| `file_id` | Stable identity of one logical file across replacements | Add to `DocumentNode`; generate a UUID for a new file and reuse it when creating a later version |
| `version_id` | Stable, idempotent identity of one file version | Derive with a new helper from `file_id + "\0" + content_sha256` |
| `document_id` | Existing user-scoped, content-derived Helix document identity | Keep `document_id_from_content(user_id, content_hash)` unchanged |

Do not reuse `document_id` as `file_id` or `version_id`. Doing so would prevent a logical file from acquiring a new version and would prevent two logical files in the same workspace from containing identical bytes.

The current upload endpoints create new logical files for a known deal, so they should pass the route's `deal_id` through the handler/service path and assign `Uuid::new_v4().to_string()` once, before parsing constructs the `DocumentNode`. A future “replace file” or “upload new version” path must supply both the existing `file_id` and its `deal_id`; it must not identify a logical file or its deal by filename.

### Deal attachment and transitional workspace mapping

Require `deal_id` as an explicit function/service argument and store it on `quarry_files` as a foreign key to `deals(deal_id)`. A logical file belongs to exactly one deal; persisting a later version may not move it to another deal.

Prefer deal-scoped Axum routes—`/api/v1/deals/{deal_id}/documents/process` and `/api/v1/deals/{deal_id}/documents/process_file`—so `Path(deal_id)` is the authoritative attachment target. Do not also accept a conflicting multipart `dealId` field.

Until the backend has a first-class workspace/tenant model, persist `DocumentNode.user_id` as `quarry_files.workspace_id`. Inside the same SQLite transaction, join `deals` to `users`, confirm that `deal_id` exists, and require the owning user's normalized email to equal `DocumentNode.user_id` before upserting the file. Isolate that transitional email/workspace mapping in one helper so it can be replaced without rewriting repository queries. Do not derive deal attachment or ownership from a filename, local path, or client-controlled metadata.

### Physical table names

Use the repository's existing plural naming convention:

- `quarry_files`
- `quarry_file_versions`
- `quarry_file_blobs`

The relationships and columns mirror the singular table labels in the diagram. Foreign keys must reference these actual names, correcting the diagram's abbreviated `files(...)` and `file_versions(...)` references.

## Target SQLite schema

Create the tables in parent-to-child order:

```sql
CREATE TABLE quarry_files (
    file_id       TEXT PRIMARY KEY NOT NULL,
    deal_id       TEXT NOT NULL REFERENCES deals(deal_id) ON DELETE CASCADE,
    workspace_id  TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    source_uri    TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    deleted_at    TEXT,
    CHECK (length(trim(file_id)) > 0),
    CHECK (length(trim(deal_id)) > 0),
    CHECK (length(trim(workspace_id)) > 0),
    CHECK (length(trim(display_name)) > 0),
    CHECK (source_uri IS NULL OR length(trim(source_uri)) > 0)
);

CREATE TABLE quarry_file_versions (
    version_id        TEXT PRIMARY KEY NOT NULL,
    file_id           TEXT NOT NULL REFERENCES quarry_files(file_id) ON DELETE CASCADE,
    version_number    INTEGER NOT NULL CHECK (version_number > 0),
    original_filename TEXT NOT NULL,
    mime_type         TEXT NOT NULL,
    content_sha256    TEXT NOT NULL,
    byte_size         INTEGER NOT NULL CHECK (byte_size >= 0),
    is_current        INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
    created_at        TEXT NOT NULL,
    UNIQUE (file_id, version_number),
    UNIQUE (file_id, content_sha256),
    CHECK (length(trim(version_id)) > 0),
    CHECK (length(trim(original_filename)) > 0),
    CHECK (length(trim(mime_type)) > 0),
    CHECK (length(content_sha256) = 64)
);

CREATE UNIQUE INDEX uq_quarry_file_versions_current
    ON quarry_file_versions(file_id)
    WHERE is_current = 1;

CREATE INDEX idx_quarry_files_deal
    ON quarry_files(deal_id, deleted_at);

CREATE INDEX idx_quarry_files_workspace_deal
    ON quarry_files(workspace_id, deal_id, deleted_at);

CREATE INDEX idx_quarry_file_versions_file
    ON quarry_file_versions(file_id, version_number DESC);

CREATE INDEX idx_quarry_file_versions_hash
    ON quarry_file_versions(content_sha256);

CREATE TABLE quarry_file_blobs (
    version_id  TEXT PRIMARY KEY NOT NULL
        REFERENCES quarry_file_versions(version_id) ON DELETE CASCADE,
    file_bytes  BLOB NOT NULL
);
```

The partial unique index is required in addition to `CHECK (is_current IN (0, 1))`; the check validates the value but does not enforce one current version per file. `ON DELETE CASCADE` models the file as deal-owned: archiving a deal retains its files because archive only changes status, while a future physical deal deletion removes its file/version/blob records. Keep foreign keys enabled on every connection, as `SqliteClient::from_connection` already does.

Use an explicit migration/user-version step in `run_migrations`; do not rely on only `CREATE TABLE IF NOT EXISTS`. The committed code already uses `PRAGMA user_version`, so the file migration should extend that mechanism rather than replace it with a fresh-schema initializer.

The two-column blob table is currently uncommitted and cannot be losslessly upgraded because it contains no workspace, filename, MIME type, or logical-file identity. Treat it as unreleased during this branch and rebuild it in the new migration. Before implementation is merged, confirm that no database containing that experimental table must be preserved. If one must be preserved, stop the rollout and add an explicit recovery migration; do not invent ownership metadata or silently discard its bytes.

## `DocumentNode` to row mapping

| Target column | Value |
|---|---|
| `quarry_files.file_id` | New `document.file_id` |
| `deal_id` | Required `deal_id` function argument, after trimming and ownership validation |
| `workspace_id` | `document.user_id` (transitional mapping) |
| `display_name` | `document.file_name` |
| `source_uri` | `document.local_path` |
| `metadata_json` | Serialized object containing at least `documentId`, `sourceType`, `tokenCount`, and `renderedPdfPath` |
| `version_id` | New `file_version_id(document.file_id, content_hash)` helper |
| `version_number` | Latest version for the file plus one, selected inside the transaction |
| `original_filename` | `document.file_name` |
| `mime_type` | `infer_supported_mime_type(Path::new(&document.file_name))`; reject unsupported/mismatched input |
| `content_sha256` | Recomputed SHA-256 of `file_bytes`, after matching `document.content_hash` |
| `byte_size` | Checked `i64` conversion of `file_bytes.len()`; also require equality with `document.file_size_bytes` |
| `is_current` | `1` for the persisted version |
| `file_bytes` | Original byte vector |
| timestamps | One UTC RFC 3339 timestamp computed before entering the transaction and reused across the rows |

Keep the existing empty-byte, SHA-256, and `document_id_from_content` validation. Add validation for nonempty `deal_id`, nonempty `file_id`, nonempty/normalized ownership and filename fields, exact byte-size agreement, supported MIME type, valid metadata serialization, and checked integer conversions. Complete all deterministic validation and query-independent serialization before acquiring the database lock; perform the authoritative deal existence/ownership query inside the transaction so it cannot race the file insert.

## Transaction support in `SqliteClient`

Add a transaction abstraction in `backend/src/core/clients/sqlite.rs` rather than issuing separate `write_async` calls or manually building `BEGIN`/`COMMIT` strings in the repository.

The abstraction should:

1. Lock the connection once and run the entire operation in one `spawn_blocking` task.
2. Start `rusqlite::TransactionBehavior::Immediate` before reading the latest version number. `BEGIN IMMEDIATE` prevents two writers from observing the same latest version and both choosing the same next number.
3. Expose transaction-scoped `read_one` and `write` methods that accept `SqlQuery`, enforce `QueryKind`, bind `SqlValue` parameters, and operate on the same rusqlite transaction.
4. Commit only when the closure returns `Ok`.
5. Roll back on any query error, repository invariant error, panic/unwind, or commit failure by returning/dropping the uncommitted transaction.
6. Never hold a SQLite transaction across `.await`; only the outer async wrapper awaits the blocking task.

A transaction wrapper/closure is preferable to a `Vec<SqlQuery>` batch because the repository must read the current version number and build later statements from that value without releasing the transaction. Keep raw rusqlite transaction types inside the client layer so the repository continues to use `SqlBuilder`/`SqlQuery` only.

Add a structured transaction-aborted error variant if repository invariant failures need to escape the closure. Preserve the existing error chain and add context at the repository boundary; do not reduce every failure to an indistinguishable generic message internally.

## Refactor `persist_file_blob`

Change the function signature to require `deal_id` while keeping the function name and `Result<String, String>` return contract:

```rust
pub async fn persist_file_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    document: &DocumentNode,
    file_bytes: Vec<u8>,
) -> Result<String, String>
```

After validation, run this sequence inside one `transaction_async` closure. Every statement must be created with `SqlBuilder`; schema DDL remains in the migration layer.

1. Select the deal and owner with a `SqlBuilder::select` join from `deals` to `users` using the supplied `deal_id`.
   - Reject a missing or archived deal according to the product's upload policy; the recommended policy is to reject archived deals.
   - Require the owner's normalized email to match `document.user_id`.
   - Keep this authorization/integrity check in the same transaction as the file insert.
2. Upsert `quarry_files` by `file_id` with `ConflictUpdate`:
   - Preserve `created_at`.
   - Refresh `display_name`, `source_uri`, `metadata_json`, and `updated_at` from `excluded` values.
   - Set `deleted_at` to `NULL` only if persisting a version is defined to restore a soft-deleted file; otherwise reject the write. Choose and test one policy rather than changing it accidentally.
   - Reject an existing `file_id` attached to a different `deal_id` or owned by a different `workspace_id` instead of moving it through an upsert.
3. Select an existing version for `(file_id, content_sha256)`.
   - If present, verify its derived `version_id`, stored size, and blob bytes. A byte-for-byte match is an idempotent retry; do not create a duplicate version.
   - Make that version current in the same transaction if it is not current, after clearing the previous current row.
   - A mismatch is a collision/corruption error and must roll back the file upsert.
4. For a new version, select the latest `version_number` for the file with `ORDER BY version_number DESC LIMIT 1` and compute `checked_add(1)`, defaulting to `1`.
5. Update the existing current version(s) for the file to `is_current = 0` with `SqlBuilder::update` and `Condition` predicates.
6. Insert the new `quarry_file_versions` row with no conflict suppression. The uniqueness and foreign-key constraints should surface errors instead of hiding them.
7. Insert `quarry_file_blobs(version_id, file_bytes)` with no conflict suppression.
8. Commit and return `document.file_id` only after the commit succeeds.

Do not retain the current `ON CONFLICT DO NOTHING` blob behavior: it can convert an identity collision or corrupt existing row into false success.

## Integration with ingestion

Update the upload path end to end:

1. Add deal-scoped process routes in `backend/src/routes/deal.rs` and use `Path(deal_id)` in the Axum handlers.
2. Change `collect_document_upload` to collect only the user identity and files; the deal comes from the route, not multipart data.
3. Pass `deal_id` through `process_uploaded_documents`, `process_uploaded_document`, `process_document`, and `persist_document_and_chunks` into `persist_file_blob`.
4. Update the construction path in `backend/src/services/document_ingestion_service.rs` and the PDF/DOCX parser boundary so each new `DocumentNode` has a logical `file_id`.
5. Change the frontend `QuarryApi` methods to `processDocuments(dealId, userId, files)` and `startProcessFile(dealId, userId, file)`, update both HTTP and Tauri adapters, give `UploadFilesModal` a required `dealId` prop, and pass `deal.room.id` from `DataRoomPage`.
6. Include the persisted `file_id` in processed-document/job completion responses so the client can address this logical file when a later version-upload feature is added.

Keep handlers thin; identity assignment and deal ownership validation belong in the service/repository path rather than handler database logic.

Re-enable the `persist_file_blob` call in `persist_document_and_chunks` only after the new transaction tests pass. Do not keep a SQLite transaction open while awaiting Helix operations.

Choose and document the cross-store order:

- Recommended for the current flow: complete Helix persistence first, then commit the SQLite file transaction. This prevents a failed Helix parse/index operation from publishing a current SQLite file version, but a final SQLite failure can still leave Helix data requiring retry/reconciliation.
- A true all-store atomic guarantee would require an outbox/saga and idempotent Helix operations; it is explicitly outside this three-table change.

The existing early Helix content-hash skip should also consult/reconcile SQLite. Otherwise content already present in Helix but absent from the new tables will continue to skip byte persistence.

## Implementation sequence

1. **Reconcile schema migration work**
   - Preserve the branch's unrelated deal/schema edits.
   - Restore/retain versioned `run_migrations` behavior.
   - Add the three file tables, constraints, indexes, and a migration test from the immediately preceding schema version.
2. **Add stable identity to the domain path**
   - Add `file_id` to `DocumentNode` and all fixtures/constructors.
   - Add and unit-test `file_version_id(file_id, content_hash)`.
   - Pass the route `deal_id` through ingestion, assign a UUID for new logical uploads, and provide a path to reuse an existing ID for later versions of that same deal file.
3. **Add the SQLite transaction API**
   - Implement synchronous and async transaction entry points plus the transaction-scoped query executor.
   - Reuse existing query-kind validation, row mapping, and parameter binding.
4. **Build a validated persistence projection**
   - Convert `deal_id + DocumentNode + file_bytes` into typed, validated file/version/blob values before the transaction.
   - Centralize MIME inference, timestamp generation, metadata JSON serialization, and numeric conversion.
5. **Refactor `persist_file_blob`**
   - Build the upsert/select/update/insert statements with `SqlBuilder`.
   - Execute them in the exact transaction flow above and return only after commit.
6. **Reconnect ingestion and update lookup behavior**
   - Add the deal-scoped Axum routes and propagate `deal_id` through frontend contracts, adapters, modal props, handlers, and services.
   - Re-enable SQLite persistence at the chosen cross-store boundary.
   - Ensure retry/skip behavior cannot leave a permanently missing SQLite representation.
7. **Run focused and full verification**
   - Format, run targeted SQLite/repository/state tests, then run the backend test suite and Clippy.

## Test plan

### `backend/tests/core/clients/sqlite_tests.rs`

- Commits multiple `SqlBuilder` writes together on success.
- Forces the second or third statement to fail and proves the first write was rolled back.
- Proves `read_one` followed by dependent writes uses the same transaction.
- Proves an `IMMEDIATE` transaction serializes concurrent version writers.
- Preserves query-kind rejection within a transaction.

### `backend/tests/state_tests.rs`

- Creates all three tables and expected columns, indexes, checks, and foreign keys.
- Runs migration/schema initialization twice without changing data.
- Rejects a file with a nonexistent `deal_id`.
- Rejects an orphan version and an orphan blob.
- Rejects two current versions for one file.
- Physically deleting a deal cascades through files, versions, and blobs; archiving a deal retains them.
- Cascading file deletion removes versions and blobs; deleting one version removes its blob.
- Verifies `metadata_json` rejects invalid JSON.
- Covers the preceding-schema-to-new-schema migration policy.

### `backend/tests/repository/document_repository_tests.rs`

- Happy path creates exactly one row in each table and preserves binary bytes.
- Happy path stores the supplied `deal_id` on the file row.
- Returned ID is the logical `file_id`; the stored version ID matches the derivation helper.
- Empty/missing `deal_id`, nonexistent/archived deal, deal-owner mismatch, empty bytes, content-hash mismatch, document-ID mismatch, byte-size mismatch, unsupported MIME type, or invalid ownership leaves all three tables unchanged.
- A trigger or constraint that aborts the file-version insert rolls back the file upsert.
- A trigger or constraint that aborts the blob insert rolls back the file row, current-version update, and version row.
- Retrying identical input is idempotent and does not increment `version_number`.
- Persisting different bytes under the same `file_id` creates the next version and leaves exactly one `is_current = 1` row.
- Two concurrent new versions receive distinct sequential version numbers.
- The same bytes under two different logical `file_id` values produce distinct version IDs.
- Reusing a `file_id` under a different deal or workspace fails without changing its attachment or ownership.
- An existing blob that does not match the supplied bytes is reported as corruption/collision, not success.

### Axum/frontend contract tests

- Deal-scoped process routes reject a missing/unknown deal and pass `Path(deal_id)` into the service.
- `UploadFilesModal` passes its active deal ID to `startProcessFile`.
- HTTP and Tauri multipart adapters target the same encoded deal-scoped route and do not duplicate `dealId` in the form body.
- Completion results expose both the logical `fileId` and existing graph `documentId`.

### Verification commands

From `backend/`:

```bash
cargo fmt --check
cargo test core::clients::sqlite::tests
cargo test repository::document_repository::tests
cargo test state::tests
cargo test
cargo clippy --all-targets -- -D warnings
```

## Acceptance criteria

- The schema attaches every logical file to `deals(deal_id)` and has the three related file tables, correct foreign keys, version uniqueness, and a database-enforced single-current-version invariant.
- Upload requests carry the active deal ID from `DataRoomPage` through the Axum path, service calls, and `persist_file_blob`.
- The transaction rejects a missing, archived, or differently owned deal before it mutates any file table.
- All runtime CRUD statements in `persist_file_blob` are parameterized `SqlBuilder` queries.
- File, version, and blob mutations run on one connection inside one `BEGIN IMMEDIATE` transaction.
- Any failure before commit leaves the database exactly as it was before the call, including restoration of the prior current version.
- An exact retry succeeds without creating an extra version.
- A real replacement creates one new current version and preserves prior version metadata/blob history.
- The function returns only after commit and reports enough context to identify validation, query-build, transaction, or commit failures.
- No transaction is held across Helix/network awaits, and the separate cross-store consistency limitation is documented.

## Out of scope

- Making Helix and SQLite a distributed atomic transaction.
- Adding workspace tables or replacing the current transitional email-based ownership check with full authentication/authorization.
- Building version-list, restore-version, or delete-version HTTP endpoints.
- Deduplicating blob bytes across different logical files; the requested schema intentionally stores one blob per version.
