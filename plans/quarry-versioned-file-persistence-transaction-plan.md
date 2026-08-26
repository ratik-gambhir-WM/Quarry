# Quarry versioned file persistence transaction plan

Status: proposed  
Prepared: 2026-08-21  
Branch reviewed: `feature/save-and-view-file` at `bd05f7188195707d4455404625e3ac22ff19ab53`<br>
Primary scope: `backend/` (Axum 0.8.9, rusqlite 0.39.0, Tokio 1.53.1)

## Goal

Replace the single `quarry_file_blobs(file_id, file_bytes)` record with the three-level structure shown in the supplied diagram, with every logical file owned by a specific deal:

```text
deals 1 ─── * quarry_files 1 ─── * quarry_file_versions 1 ─── 1 quarry_file_blobs
```

Refactor `persist_file_blob` into the main and only coordinator for this SQLite unit of work. It must accept a required `deal_id`, verify that the deal belongs to the document's user/workspace, and save the logical file, its version metadata, and its bytes to `quarry_files`, `quarry_file_versions`, and `quarry_file_blobs`, in that parent-to-child order, using one SQLite transaction and one connection. The function must invoke the transaction API exactly once and must not use standalone `write_async` calls for any of the three tables.

Every fallible operation after the transaction begins must propagate its error to the transaction boundary. If deal validation, a query build, the `quarry_files` write, the `quarry_file_versions` write, the `quarry_file_blobs` write, an invariant check, or the final commit fails, the transaction must roll back and leave all three file tables exactly as they were before `persist_file_blob` was called. The function returns the logical `file_id` only after the commit succeeds.

This transaction boundary covers the three SQLite tables only. Helix writes in `persist_document_and_chunks` cannot participate in the same SQLite transaction and must not be described as atomic with these writes.

This phase stops at the schema, SQLite client, and repository-function boundary. Build and test `persist_file_blob`, but do not integrate it into the live ingestion path yet. In particular, keep its existing call in `persist_document_and_chunks` commented out.

## Baseline findings

- `backend/src/repository/document_repository.rs` currently inserts only `file_id` and `file_bytes`, using `SqlBuilder` and `SqliteClient::write_async`.
- `backend/src/core/clients/sqlite.rs` serializes a single rusqlite connection behind `Arc<Mutex<Connection>>` and moves asynchronous calls to `spawn_blocking`, but it has no transaction API.
- `backend/src/state.rs` currently creates only the two-column blob table. `CREATE TABLE IF NOT EXISTS` cannot transform that table into the new shape.
- `DocumentNode.document_id` is derived from `(user_id, content_hash)` and is also the Helix document identity. It is a content identity, not a stable logical-file identity.
- `DocumentNode` already provides the filename, source type, optional local path, byte count, content hash, token count, and optional rendered-PDF path required to build the proposed rows.
- `infer_supported_mime_type` already maps supported extensions to MIME types.
- `deals.deal_id` is the stable deal key and `deals.user_id` references `users.id`; the upload path currently sends only a user email and drops the active deal ID even though `DataRoomPage` already has it.
- The working tree was clean before this plan-only update. Implementation must still preserve and reconcile any unrelated user changes present when coding begins.

## Design decisions

### `persist_file_blob` owns the complete unit of work

Keep `persist_file_blob` as the public repository entry point even though it now persists the complete file aggregate rather than only the blob row. It owns the following lifecycle:

1. Perform deterministic input validation and construct the persistence projection.
2. Open one `BEGIN IMMEDIATE` transaction through `SqliteClient`.
3. Perform transaction-scoped ownership and existing-row reads.
4. Write `quarry_files` first.
5. Write or update `quarry_file_versions` second.
6. Write `quarry_file_blobs` third.
7. Return `Ok(file_id)` from the transaction closure so the client commits.
8. Return the ID to the caller only after that commit succeeds.

Repository helpers may keep the function readable, but they may only validate values, build `SqlBuilder` queries, or execute queries against the transaction handle supplied by `persist_file_blob`. They must not acquire the connection independently, start or commit a transaction, call `write_async`, or swallow an error. `persist_file_blob` must have one awaited database call: the outer transaction operation.

For an idempotent retry, the same coordinator still owns the transaction. It may read and verify the already stored version/blob instead of inserting duplicates, but it must commit or roll back as one unit and preserve the same aggregate invariants.

### Separate logical file, version, and graph identities

Use three distinct identifiers:

| Identity | Meaning | Source |
|---|---|---|
| `file_id` | Stable identity of one logical file across replacements | Add to `DocumentNode`; generate a UUID for a new file and reuse it when creating a later version |
| `version_id` | Stable, idempotent identity of one file version | Derive with a new helper from `file_id + "\0" + content_sha256` |
| `document_id` | Existing user-scoped, content-derived Helix document identity | Keep `document_id_from_content(user_id, content_hash)` unchanged |

Do not reuse `document_id` as `file_id` or `version_id`. Doing so would prevent a logical file from acquiring a new version and would prevent two logical files in the same workspace from containing identical bytes.

The repository function and its direct tests need a stable logical `file_id`. Add the minimum domain/fixture support needed to build and test that repository input, but do not propagate persistence through upload handlers or services in this phase. A later integration phase should assign `Uuid::new_v4().to_string()` once for a new logical upload and must supply both the existing `file_id` and its `deal_id` for a replacement; it must not identify a logical file or its deal by filename.

### Deal attachment and transitional workspace mapping

Require `deal_id` as an explicit `persist_file_blob` argument and store it on `quarry_files` as a foreign key to `deals(deal_id)`. Do not add it to ingestion service signatures in this phase. A logical file belongs to exactly one deal; persisting a later version may not move it to another deal.

For the later integration phase, prefer deal-scoped Axum routes—`/api/v1/deals/{deal_id}/documents/process` and `/api/v1/deals/{deal_id}/documents/process_file`—so `Path(deal_id)` is the authoritative attachment target. Route, handler, service, frontend, and multipart-contract changes are explicitly deferred from this plan.

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

Introduce an explicit, versioned migration path (for example, `run_migrations` backed by `PRAGMA user_version`); do not rely only on the current `initialize_schema` plus `CREATE TABLE IF NOT EXISTS`, because that cannot reshape the existing blob table. Route both file-backed and in-memory application initialization through the same migration path.

The two-column blob table currently exists on this branch and cannot be losslessly upgraded because it contains no workspace, filename, MIME type, or logical-file identity. Before choosing a rebuild migration, confirm that no persisted database containing this experimental table must be preserved. If one must be preserved, stop the rollout and add an explicit recovery migration; do not invent ownership metadata or silently discard its bytes.

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

Add a transaction abstraction in `backend/src/core/clients/sqlite.rs` so `persist_file_blob` can own one atomic unit of work without issuing separate `write_async` calls or manually building `BEGIN`/`COMMIT` strings in the repository. This abstraction is plumbing only; it must not decide which file tables to write or their order.

The abstraction should:

1. Lock the connection once and run the entire operation in one `spawn_blocking` task.
2. Start `rusqlite::TransactionBehavior::Immediate` before reading the latest version number. `BEGIN IMMEDIATE` prevents two writers from observing the same latest version and both choosing the same next number.
3. Expose transaction-scoped `read_one` and `write` methods that accept `SqlQuery`, enforce `QueryKind`, bind `SqlValue` parameters, and operate on the same rusqlite transaction.
4. Commit only when the closure returns `Ok`; expose commit failure to `persist_file_blob` as an error.
5. On any query error or repository invariant error, return `Err` from the closure and explicitly roll back when possible; dropping an uncommitted rusqlite transaction remains the fallback. On panic/unwind, dropping the uncommitted transaction must also roll it back.
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

After deterministic validation, call `sqlite.transaction_async(...)` exactly once; that client API must always start an `IMMEDIATE` transaction. Put all database reads, invariant checks, and writes described below inside that one synchronous closure. Every statement must be created with `SqlBuilder`; schema DDL remains in the migration layer.

1. Select the deal and owner with a `SqlBuilder::select` join from `deals` to `users` using the supplied `deal_id`.
   - Reject a missing or archived deal. Uploading a file must not reactivate an archived deal.
   - Require the owner's normalized email to match `document.user_id`.
   - Keep this authorization/integrity check in the same transaction as the file insert.
2. **First table — `quarry_files`:** upsert `quarry_files` by `file_id` with `ConflictUpdate`:
   - First read any existing row for `file_id` through the same transaction. If one exists, require its `deal_id` and `workspace_id` to match the request before issuing the upsert.
   - Preserve `created_at`.
   - Refresh `display_name`, `source_uri`, `metadata_json`, and `updated_at` from `excluded` values.
   - Reject an existing row whose `deleted_at` is non-null. Restoring a soft-deleted logical file requires a separate, explicit operation.
   - Reject an existing `file_id` attached to a different `deal_id` or owned by a different `workspace_id` instead of moving it through an upsert.
3. Select an existing version for `(file_id, content_sha256)` using the same transaction.
   - If present, verify its derived `version_id`, stored size, and blob bytes. A byte-for-byte match is an idempotent retry; do not create a duplicate version.
   - Make that version current in the same transaction if it is not current, after clearing the previous current row.
   - A mismatch is a collision/corruption error and must roll back the file upsert.
4. For a new version, select the latest `version_number` for the file with `ORDER BY version_number DESC LIMIT 1` and compute `checked_add(1)`, defaulting to `1`.
5. **Second table — `quarry_file_versions`:** update the existing current version(s) for the file to `is_current = 0`, then insert the new current `quarry_file_versions` row with no conflict suppression. The uniqueness and foreign-key constraints must surface errors instead of being hidden.
6. **Third table — `quarry_file_blobs`:** insert `quarry_file_blobs(version_id, file_bytes)` with no conflict suppression.
7. Return `Ok(document.file_id)` from the closure. The transaction abstraction commits only after this result; `persist_file_blob` returns the ID only when the outer transaction call confirms that commit succeeded.

Use `?` (with contextual error mapping) at each transaction-scoped read, query build, invariant check, update, and insert. Do not catch a failure and continue to a later table. Do not perform compensating `DELETE` statements: rollback is the sole partial-write cleanup mechanism.

The implementation should follow this ownership shape (exact types may vary with the final `SqliteClient` API):

```rust
pub async fn persist_file_blob(
    sqlite: &SqliteClient,
    deal_id: &str,
    document: &DocumentNode,
    file_bytes: Vec<u8>,
) -> Result<String, String> {
    let input = validate_and_build_file_persistence(deal_id, document, file_bytes)?;
    let file_id = input.file_id.clone();

    sqlite
        .transaction_async(move |transaction| {
            validate_deal_ownership(transaction, &input)?;

            // Parent first.
            upsert_quarry_file(transaction, &input)?;

            if let Some(existing) = find_existing_file_version(transaction, &input)? {
                verify_idempotent_version_and_blob(transaction, &input, &existing)?;
                make_existing_version_current(transaction, &input, &existing)?;
            } else {
                let version_number = next_file_version_number(transaction, &input)?;

                // Child metadata second.
                clear_current_file_version(transaction, &input)?;
                insert_quarry_file_version(transaction, &input, version_number)?;

                // Blob child last.
                insert_quarry_file_blob(transaction, &input)?;
            }

            Ok(file_id.clone())
        })
        .await
        .map_err(|error| format!("failed to persist file transaction: {error}"))
}
```

This sketch deliberately keeps transaction creation in `persist_file_blob`. The named helpers do not represent separate transactions or async database calls; all receive the same transaction handle. For a new file/version, the observable mutation order is always `quarry_files` → `quarry_file_versions` → `quarry_file_blobs`.

Do not retain the current `ON CONFLICT DO NOTHING` blob behavior: it can convert an identity collision or corrupt existing row into false success.

## Ingestion integration is intentionally deferred

Do not connect `persist_file_blob` to routes, handlers, document-processing services, frontend adapters, or job-completion responses in this phase. Leave the runtime behavior of `persist_document_and_chunks` unchanged and preserve the commented-out repository call:

```rust
pub async fn persist_document_and_chunks(
    sqlite: &SqliteClient,
    helix: &HelixClient,
    document: DocumentNode,
    chunks: Vec<ChunkNode>,
    file_bytes: Vec<u8>,
) -> Result<PersistedDocumentGraph, String> {
    for chunk in &chunks {
        validate_document_chunk_relationship(&document, chunk)?;
    }

    // persist_file_blob(sqlite, &document, file_bytes).await?;

    let quarry_file = persist_quarry_file(helix, document.clone()).await?;
    let chunks = persist_chunks_for_document(helix, &document, chunks).await?;
    mark_quarry_file_ingestion_complete(helix, &document).await?;
    Ok(PersistedDocumentGraph {
        quarry_file,
        chunks,
    })
}
```

The commented call intentionally shows the current, not-yet-integrated state and does not need to be updated to the new `deal_id` signature in this phase. Do not add `deal_id` to `persist_document_and_chunks` merely to make a commented example current. Necessary compile-time updates to shared domain types are allowed, but they must not activate SQLite file persistence from production ingestion.

Direct repository tests must call `persist_file_blob` with a seeded deal and explicit `deal_id`. Passing `deal_id` through Axum routes and services, deciding Helix-versus-SQLite ordering, reconciling the early Helix content-hash skip, and exposing `file_id` in API responses belong to a separate integration plan.

## Implementation sequence

1. **Reconcile schema migration work**
   - Preserve the branch's unrelated deal/schema edits.
   - Introduce versioned `run_migrations` behavior and call it from application/database initialization.
   - Add the three file tables, constraints, indexes, and a migration test from the immediately preceding schema version.
2. **Add stable identity to the domain path**
   - Add `file_id` to `DocumentNode` and make only the compile-time fixture/constructor updates required by that type change.
   - Add and unit-test `file_version_id(file_id, content_hash)`.
   - Do not pass `deal_id` through ingestion or activate repository persistence from those constructors in this phase.
3. **Add the SQLite transaction API**
   - Implement synchronous and async transaction entry points plus the transaction-scoped query executor.
   - Reuse existing query-kind validation, row mapping, and parameter binding.
4. **Build a validated persistence projection**
   - Convert `deal_id + DocumentNode + file_bytes` into typed, validated file/version/blob values before the transaction.
   - Centralize MIME inference, timestamp generation, metadata JSON serialization, and numeric conversion.
5. **Refactor `persist_file_blob`**
   - Build the upsert/select/update/insert statements with `SqlBuilder`.
   - Make one `transaction_async` call, execute the table writes in the exact flow above, and return only after commit.
   - Keep all helpers transaction-scoped and remove the function's standalone `write_async` blob insert.
6. **Preserve the integration boundary**
   - Leave the `persist_file_blob` line commented out in `persist_document_and_chunks`.
   - Do not change Axum routes, frontend contracts, adapters, modal props, job responses, or service signatures to carry `deal_id` for this function.
   - Verify the existing Helix ingestion flow does not invoke or indirectly depend on the new SQLite transaction.
7. **Run focused and full verification**
   - Format, run targeted SQLite/repository/state tests, then run the backend test suite and Clippy.

## Test plan

### `backend/tests/core/clients/sqlite_tests.rs`

- Commits multiple `SqlBuilder` writes together on success.
- Forces the second or third statement to fail and proves the first write was rolled back.
- Forces commit to fail and proves all writes remain uncommitted.
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
- A failure in the `quarry_files` write prevents either child-table write.
- A trigger or constraint that aborts the file-version insert rolls back the file upsert.
- A trigger or constraint that aborts the blob insert rolls back the file row, current-version update, and version row.
- Retrying identical input is idempotent and does not increment `version_number`.
- Persisting different bytes under the same `file_id` creates the next version and leaves exactly one `is_current = 1` row.
- Two concurrent new versions receive distinct sequential version numbers.
- The same bytes under two different logical `file_id` values produce distinct version IDs.
- Reusing a `file_id` under a different deal or workspace fails without changing its attachment or ownership.
- An existing blob that does not match the supplied bytes is reported as corruption/collision, not success.

### Non-integration regression checks

- `persist_document_and_chunks` retains the commented-out `persist_file_blob` line and does not call the function through another helper.
- Existing ingestion tests continue to exercise only the current Helix persistence behavior.
- No Axum route, service, frontend API, multipart payload, or completion-response contract changes as part of this work.
- Repository transaction tests invoke `persist_file_blob` directly with an explicit `deal_id`; production upload flow does not invoke it.

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
- The transaction rejects a missing, archived, or differently owned deal before it mutates any file table.
- All runtime CRUD statements in `persist_file_blob` are parameterized `SqlBuilder` queries.
- `persist_file_blob` is the sole coordinator: it invokes the transaction API once, writes new aggregate rows in `quarry_files` → `quarry_file_versions` → `quarry_file_blobs` order, and performs no standalone file-table writes.
- File, version, and blob mutations run on one connection inside one `BEGIN IMMEDIATE` transaction.
- Any failure before commit leaves the database exactly as it was before the call, including restoration of the prior current version.
- An exact retry succeeds without creating an extra version.
- A real replacement creates one new current version and preserves prior version metadata/blob history.
- The function returns only after commit and reports enough context to identify validation, query-build, transaction, or commit failures.
- `persist_document_and_chunks` still contains the commented-out `persist_file_blob` call and the live ingestion path never invokes the new SQLite transaction.
- No route, handler, service, frontend, multipart, or API-response integration is included in this phase.

## Out of scope

- Making Helix and SQLite a distributed atomic transaction.
- Enabling `persist_file_blob` from `persist_document_and_chunks` or any other production ingestion path.
- Propagating `deal_id` through Axum routes, handlers, services, frontend adapters, multipart requests, or job responses.
- Choosing cross-store ordering or reconciling Helix content-hash skips with SQLite file persistence.
- Adding workspace tables or replacing the current transitional email-based ownership check with full authentication/authorization.
- Building version-list, restore-version, or delete-version HTTP endpoints.
- Deduplicating blob bytes across different logical files; the requested schema intentionally stores one blob per version.
