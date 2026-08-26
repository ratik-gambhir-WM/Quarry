# Quarry Helix versioned file graph ingestion plan

Status: proposed  
Prepared: 2026-08-26  
Branch reviewed: `feature/save-and-view-file` at `bd05f7188195707d4455404625e3ac22ff19ab53`  
Depends on: the in-progress SQLite file/version/blob transaction work described in `plans/quarry-versioned-file-persistence-transaction-plan.md`  
Primary scope: `backend/`, with the minimum route/frontend propagation needed to supply `deal_id`

## Goal

Activate SQLite file persistence in `persist_document_and_chunks`, read back the committed file identity, and use that identity to write this graph:

```text
(QuarryFile)-[:HAS_VERSION]->(FileVersion)
(QuarryFile)-[:CURRENT_VERSION]->(FileVersion)
(FileVersion)-[:HAS_CHUNK]->(FileChunk)
```

The Helix mutation for one file version must be exactly one `DynamicQueryRequest` containing one `WriteBatch`. File creation/update, immutable version creation, current-version edge replacement, current-version chunk replacement, and all `HAS_CHUNK` edges therefore commit or roll back as one Helix transaction.

When a later version is persisted for the same `file_id`, the transaction must retain every prior `FileVersion`, its `HAS_VERSION` edge, its `FileChunk` nodes, and its `HAS_CHUNK` edges. It replaces only the `CURRENT_VERSION` edge and the chunks belonging to the version currently being indexed. `QuarryFile` must never have a direct `HAS_CHUNK` edge.

SQLite and Helix are separate systems. The required ordering is SQLite commit first, then one Helix transaction. This is not a distributed transaction: a Helix failure leaves the valid SQLite version in place, and a retry must use the same returned `file_id`/`version_id` to rebuild the graph idempotently.

## Baseline findings

- The working tree contains substantial uncommitted, user-owned SQLite persistence work. Preserve and reconcile it; do not reset or overwrite it.
- `persist_file_blob` now writes `quarry_files`, `quarry_file_versions`, and `quarry_file_blobs` in one `BEGIN IMMEDIATE` SQLite transaction, but returns only `file_id`.
- `persist_document_and_chunks` still leaves `persist_file_blob` commented out and performs three Helix writes: file insert, one or more chunk inserts, and ingestion-complete update.
- The current Helix model is `QuarryFile(document_id, user_id, ...) -> HAS_CHUNK -> Chunk`; it deletes the prior file and chunks for a content-derived `document_id`, so it cannot preserve version history.
- `insert_chunk_batches` splits large payloads into multiple Helix requests. Each `WriteBatch` is atomic, but the sequence of requests is not one Helix transaction.
- `helix-db` 2.0.6 explicitly executes one `WriteBatch` sequentially in one transaction and supports `for_each_param` inside that transaction.
- Helix's dynamic-query route currently has a 2 MiB buffered request limit. There is no repository-local API for holding one Helix transaction open across multiple HTTP requests.
- `persist_file_blob` requires `deal_id`, but the current document routes, handlers, service functions, and frontend upload methods carry only `user_id`. `DataRoomPage` already has the active `dealId`.
- Parsers currently generate a new UUID `file_id` for every parse. Reusing a logical `file_id` can create another version at repository level, but the current UI has no replace-version contract.

## Fixed design decisions

### Rust domain types versus graph node types

Move the parser/service representations into `backend/src/services/document_ingestion_service.rs` and rename them so they are not presented as graph nodes:

```rust
// backend/src/services/document_ingestion_service.rs
pub struct Document { /* current DocumentNode service fields */ }
pub struct DocumentChunk { /* current ChunkNode parser/service fields */ }
```

Keep `backend/src/core/nodes/document_node.rs` exclusively for the DTOs that are actually projected into Helix:

```rust
// backend/src/core/nodes/document_node.rs
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub workspace_id: String,
    pub file_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileVersionNode {
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub mime_type: String,
    pub content_sha256: String,
    pub byte_size: i64,
    pub index_generation: String,
    pub indexed_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChunkNode {
    pub chunk_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub text: String,
    pub embedding: Vec<f32>,
    pub chunk_sha256: String,
    pub token_count: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub section_path: String,
    pub created_at: String,
}
```

Use the service-owned `Document` and `DocumentChunk` only through parsing, embedding, validation, and ingestion orchestration. Parsers should return these types from `document_ingestion_service`; repository functions that need them should import them from the service module rather than from `core::nodes`. Convert them to `FileNode`, `FileVersionNode`, and `FileChunkNode` only after SQLite returns the committed identity.

`document_node.rs` should no longer define or re-export `Document`, `DocumentChunk`, `DocumentNode`, or `ChunkNode`. This keeps the node module aligned with the persisted Helix schema and makes the service module the owner of the transient ingestion model.

The Helix node label remains `QuarryFile` to match the supplied graph model; `FileNode` is the Rust projection name.

### Canonical identities

- `FileNode.file_id = persisted.file_id`. `file_id` is a UUID and is the sole stable logical-file identity; do not add a duplicate graph key property.
- `FileVersionNode.version_id = persisted.version_id`. The current `file_version_id(file_id, content_sha256)` helper remains the canonical version derivation; do not add a duplicate graph key property.
- `index_generation = version_id` for the initial indexing model. This is deterministic across retries and leaves room for a later reindex-generation policy without changing content-version identity.
- Derive the persisted `FileChunkNode.chunk_id` from `workspace_id + "\0" + file_id + "\0" + version_id + "\0" + index_generation + "\0" + chunk_index + "\0" + chunk_sha256`. Do not copy the transient service chunk's current `chunk_id`, because identical bytes in two logical files in one workspace currently produce the same content-derived document/chunk IDs.
- Treat the content identity fields on `FileVersion` as immutable. A retry may refresh the operational `index_generation`/`indexed_at` fields and atomically replace that version's chunk set, but may not change `workspace_id`, `file_id`, `version_id`, `mime_type`, `content_sha256`, or `byte_size`.

### Nullable page fields

Represent `page_start` and `page_end` as nullable graph properties. For a PDF, use the minimum and maximum page numbers on the service chunk. For DOCX or a chunk without page information, write `Null`; do not invent page zero. Map `section_title` to `section_path`, defaulting to an empty string until hierarchical section parsing exists.

### One Helix transaction and the payload limit

Keep chunk batching inside the single `WriteBatch` with one `for_each_param("chunks", ...)` entry. Do not use the existing loop that returns and executes multiple `DynamicQueryRequest` values.

Serialize the complete request before sending it and require it to fit `HELIX_MAX_QUERY_BODY_BYTES`. If it does not fit, return an explicit preflight error containing the serialized size and configured limit; do not silently split it and weaken the transaction guarantee. Supporting larger atomic ingestions requires raising the Helix gateway limit or adding a server-side bulk/stored-query facility and is outside this repository-only change.

## Return the committed SQLite identity

Add a public repository result type in `backend/src/core/models/file_persistence.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedFileIdentity {
    pub file_id: String,
    pub workspace_id: String,
    pub display_name: String,
    pub version_id: String,
}
```

Change `persist_file_blob` to return `Result<PersistedFileIdentity, String>`.

After the new-version or idempotent-version branch finishes, but still inside the same SQLite transaction, select the persisted values by joining `quarry_files` to `quarry_file_versions` for `input.file_id` and `input.version_id`. Map `file_id`, `workspace_id`, and `display_name` from `quarry_files` and `version_id` from `quarry_file_versions`. Treat a missing row, duplicate row, null/type mismatch, or a value that differs from the validated input as a transaction error. Return the typed result from the closure; expose it to the caller only after the transaction commits.

This read-back is deliberate: the Helix graph is built from values verified in SQLite rather than from an uncommitted input echo.

## Service-to-graph mapping

Use one UTC RFC 3339 timestamp for `indexed_at` and every new chunk's `created_at`.

| Graph field | Source |
|---|---|
| `FileNode.workspace_id` | committed `workspace_id` |
| `FileNode.file_id` | committed `file_id` |
| `FileNode.display_name` | committed `display_name` |
| `workspace_id`, `file_id`, `version_id` | committed identity |
| `mime_type` | `infer_supported_mime_type(Document.file_name)`; it was already validated by SQLite persistence |
| `content_sha256` | `Document.content_hash` |
| `byte_size` | checked `i64` conversion of `Document.file_size_bytes` |
| `index_generation` | committed `version_id` in this phase |
| `indexed_at` | graph-ingestion timestamp |
| `FileChunkNode.chunk_id` | deterministic hash of the committed file/version identity, generation, chunk index, and chunk hash |
| `FileChunkNode.chunk_index` | checked conversion of the service chunk sequence number |
| `text`, `embedding` | service chunk; reject a missing embedding before either graph construction or Helix execution |
| `chunk_sha256` | service chunk content hash |
| `token_count` | checked integer conversion |
| `page_start`, `page_end` | min/max service chunk page number, otherwise null |
| `char_start`, `char_end` | checked conversion of service chunk offsets |
| `section_path` | service chunk section title or empty string |

Validate before the Helix call that every chunk belongs to the same service `Document`, offsets are ordered, chunk indices are unique, embeddings are present and have the expected consistent dimension, page ranges are ordered, and every graph identity equals the committed SQLite identity.

## Build the atomic Helix write

Refactor `backend/src/core/helix_queries/files/insert_quarry_file.rs` around one builder, for example:

```rust
pub fn insert_file_version_graph(
    file: FileNode,
    version: FileVersionNode,
    chunks: Vec<FileChunkNode>,
) -> Result<DynamicQueryRequest, String>
```

The registered route must construct one `WriteBatch` in this order:

1. Find a `QuarryFile` by the exact `file_id` and `workspace_id`.
2. Conditionally create it only when absent, then re-read the canonical node by the same identity and refresh `display_name`.
3. Find a `FileVersion` by the complete immutable identity, including `version_id`.
4. Conditionally create it when absent, then re-read the canonical version node. If a node with the same unique `version_id` but conflicting immutable values exists, the attempted create must hit the unique index and roll back instead of reusing corrupt data.
5. Remove only an existing `HAS_VERSION` edge between this file and this version, then add exactly one `HAS_VERSION` edge. This repairs a missing edge and makes retries edge-idempotent without touching historical versions.
6. Read the file's old `CURRENT_VERSION` target(s), remove only those `CURRENT_VERSION` edges, and add exactly one `CURRENT_VERSION` edge to the persisted version. Do not delete any old version node.
7. Delete only `FileChunk` nodes whose `workspace_id`, `file_id`, and `version_id` equal the version being indexed. Their incident `HAS_CHUNK` edges disappear with them. Do not select chunks by workspace or file alone.
8. Use one `for_each_param("chunks", ...)` body to create each `FileChunk` and add `FileVersion -[:HAS_CHUNK]-> FileChunk`.
9. Return projections for the canonical file, current version, inserted chunks, and edge operations in one response.

Remove the old whole-document cleanup, direct `QuarryFile -> HAS_CHUNK` creation, separate chunk request builder, and `mark_quarry_file_ingestion_complete` query. A successful atomic transaction is the completion marker; no partially ingested graph becomes visible.

## Indexes and graph readers

Update `create_document_indexes` with:

- unique equality indexes on `QuarryFile.file_id`, `FileVersion.version_id`, and `FileChunk.chunk_id`;
- equality indexes needed for workspace/file/version lookups on all three labels;
- an equality index on `FileVersion.content_sha256`;
- vector and text indexes on `FileChunk.embedding` and `FileChunk.text`, partitioned by `workspace_id` as required by the supplied model;
- a text index on `QuarryFile.display_name` if filename search remains supported.

Refactor `backend/src/core/helix_queries/files/search_quarry_file.rs`, its repository/service wrappers, and `backend/src/handlers/documents/search.rs` so graph reads have explicit domain return types instead of `serde_json::Value`.

Replace the legacy `ChunkVectorSearch`/`ChunkKeywordSearch` request types with graph-specific requests whose tenant field matches the new schema:

```rust
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChunkVectorSearch {
    pub workspace_id: String,
    pub query_embedding: Vec<f32>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChunkKeywordSearch {
    pub workspace_id: String,
    pub query_text: String,
    pub limit: usize,
}
```

The HTTP request field is therefore `workspaceId`, not the legacy `userId`. Validate the nonempty workspace, nonempty query/embedding, positive bounded limit, and finite embedding values before building the Helix request.

Define the public result models next to the Helix read queries:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HelixDocumentVersion {
    pub file: FileNode,
    pub version: FileVersionNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileChunkResult {
    pub chunk_id: String,
    pub workspace_id: String,
    pub file_id: String,
    pub version_id: String,
    pub index_generation: String,
    pub chunk_index: i64,
    pub text: String,
    pub chunk_sha256: String,
    pub token_count: i64,
    pub page_start: Option<i64>,
    pub page_end: Option<i64>,
    pub char_start: i64,
    pub char_end: i64,
    pub section_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    pub distance: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeywordFileChunkHit {
    #[serde(flatten)]
    pub chunk: FileChunkResult,
    pub score: f64,
}
```

Do not return `embedding` in a read/search result; it is an index input and would unnecessarily multiply response size. Keep private serde response-envelope types that match Helix's `{ variable: { properties: [...] } }` payload and map those envelopes into the public types above at the repository boundary.

Implement these typed graph reads:

- `find_current_helix_document_by_content_hash(workspace_id, content_sha256) -> Result<Option<HelixDocumentVersion>, String>` traverses `QuarryFile(workspace_id) -[:CURRENT_VERSION]-> FileVersion(content_sha256)` and returns both projected nodes. It replaces `find_existing_document_id_by_content_hash`; it must not claim that a Helix file/version ID is the service-layer `document_id`.
- `get_current_helix_document(workspace_id, file_id) -> Result<Option<HelixDocumentVersion>, String>` traverses the requested file's single `CURRENT_VERSION` edge.
- `get_helix_document_version(workspace_id, file_id, version_id) -> Result<Option<HelixDocumentVersion>, String>` traverses `HAS_VERSION` and verifies that the requested version belongs to the requested file/workspace.
- `get_helix_document_version_chunks(workspace_id, file_id, version_id) -> Result<Vec<FileChunkResult>, String>` traverses that exact version's `HAS_CHUNK` edges, projects the typed chunk result fields, and orders by `chunk_index` ascending.
- `search_document_chunks_by_vector(search: FileChunkVectorSearch) -> Result<Vec<VectorFileChunkHit>, String>` targets `FileChunk.embedding`, partitions by `search.workspace_id`, and maps `$distance` to the required `distance` field.
- `search_document_chunks_by_keyword(search: FileChunkKeywordSearch) -> Result<Vec<KeywordFileChunkHit>, String>` targets `FileChunk.text`, partitions by `search.workspace_id`, and maps `$score` to the required `score` field.

For every single-document/version get, return `Ok(None)` only when both file and version projections are empty. Return an integrity error for a partial response, more than one file, more than one version, mismatched workspace/file/version IDs, or a current-version edge that resolves to multiple targets. Search and chunk-list functions return `Ok(vec![])` for no hits and preserve the query's deterministic rank/order.

Update the service and Axum handlers to preserve these concrete request/result types end to end: vector handlers accept `Json<FileChunkVectorSearch>` and return `AppResult<Json<Vec<VectorFileChunkHit>>>`; keyword handlers accept `Json<FileChunkKeywordSearch>` and return `AppResult<Json<Vec<KeywordFileChunkHit>>>`; and any new get handler returns `AppResult<Json<HelixDocumentVersion>>` or a not-found error after translating repository `None`. The ingestion deduplication path should keep using its already computed `Document.document_id` in `ProcessedDocument`; do not reintroduce `document_id` onto graph nodes solely for the old response contract.

Existing Helix data uses incompatible `QuarryFile` properties and the old `Chunk` label. Before rollout, explicitly choose either a one-time graph migration or a controlled Helix clear/reindex. Do not silently mix the schemas, and do not run the destructive clear as part of normal startup.

## Orchestration and `deal_id` propagation

Change `persist_document_and_chunks` to accept `deal_id`, use `Document`/`DocumentChunk`, and execute this flow:

1. Validate service document/chunk relationships and required embeddings.
2. Call the now-live `persist_file_blob(sqlite, deal_id, &document, file_bytes).await?`.
3. Build `FileNode`, `FileVersionNode`, and `FileChunkNode` from the committed identity plus service data.
4. Build and serialize-check the complete Helix request.
5. Call `HelixClient::execute_document_query` exactly once.
6. Return a revised `PersistedDocumentGraph` containing the committed SQLite identity and the single Helix transaction response.

Remove or make private the separate `persist_quarry_file`, `persist_chunks_for_document`, and ingestion-complete orchestration functions so production code cannot accidentally reintroduce a multi-request ingestion path.

Propagate the required deal identity through the existing layers:

- make upload routes deal-scoped, such as `/api/v1/deals/{deal_id}/documents/process` and `/api/v1/deals/{deal_id}/documents/process_file`;
- extract `Path(deal_id)` in the Axum handlers and keep `Multipart` last because it consumes the request body;
- pass `deal_id` through `process_uploaded_documents`, worker tasks, `process_document`, and `persist_document_and_chunks`;
- add `dealId` to the frontend Quarry API method signatures and URLs;
- pass the existing `DataRoomPage` route `dealId` into `UploadFilesModal` and its upload calls;
- update HTTP, Tauri transport, component, and service tests for the route contract.

Do not accept a competing multipart `dealId` when the path already supplies the authoritative value. Keep the SQLite deal/workspace ownership check as the final integrity boundary.

The existing upload UI may continue to create a new UUID `file_id` for a new logical upload. Repository/service tests must also exercise a second ingestion with the same `file_id` and different bytes to prove historical graph preservation. A user-facing replace-version endpoint/UX may be added separately; do not infer logical-file identity from `display_name`.

## Implementation sequence

1. **Reconcile the SQLite dependency**
   - Finish or preserve the current transaction/migration work.
   - Add `PersistedFileIdentity` and the final transaction-scoped join/read-back.
   - Update repository tests before activating the call.
2. **Separate service data from graph DTOs**
   - Define `Document` and `DocumentChunk` in `backend/src/services/document_ingestion_service.rs`, using the current `DocumentNode`/`ChunkNode` fields.
   - Update parsers, repository functions, and tests to import those transient types from the service module.
   - Remove `DocumentNode` and `ChunkNode` from `backend/src/core/nodes/document_node.rs`.
   - Keep only the exact three graph node structs in `document_node.rs` and add the deterministic `chunk_id` helper alongside the graph-mapping code.
3. **Build the single Helix query**
   - Replace the old insert/chunk/complete builders with `insert_file_version_graph`.
   - Keep one in-request `for_each_param` chunk batch and enforce the serialized-size guard.
   - Update indexes.
4. **Update graph reads**
   - Replace the old document-ID lookup with typed current-file and specific-version traversals.
   - Add typed current-document, historical-version, and version-chunk repository return values with strict cardinality checks.
   - Move vector and keyword search to `FileChunk` properties and workspace partitioning.
   - Remove `serde_json::Value` from the document get/search repository, service, and Axum handler signatures.
5. **Activate orchestration**
   - Uncomment SQLite persistence, construct graph DTOs from its committed result, and execute Helix once.
   - Update `PersistedDocumentGraph` and remove obsolete three-step wrappers.
6. **Carry `deal_id` end to end**
   - Update Axum paths/handlers/services and frontend adapters/components.
7. **Verify migration and failure behavior**
   - Test fresh graph ingestion, retries, a second version, rollback, payload rejection, and route propagation.
   - Document the selected legacy Helix migration/clear procedure before deployment.

## Test plan

### SQLite repository tests

- A new version returns the exact committed `file_id`, `workspace_id`, `display_name`, and `version_id`.
- An idempotent retry returns the existing version identity.
- A second content hash under the same `file_id` returns a new `version_id` while preserving the first SQLite version/blob.
- A failure in the final read-back rolls back all aggregate changes.
- No result is returned when commit fails.

### Node mapping tests

- Every supplied graph field is populated from the documented source.
- Same content under different `file_id` values produces different `chunk_id` values.
- The same version/chunk input produces the same `chunk_id` on retry.
- PDF page ranges use min/max; DOCX page fields serialize as null.
- Missing embeddings, overflow, invalid offsets/ranges, duplicate chunk indices, or mismatched identities fail before Helix execution.

### Helix query tests

- The request is one write `DynamicQueryRequest` with one `WriteBatch` and one chunk `ForEach` entry.
- The JSON contains exactly `QuarryFile`, `FileVersion`, `FileChunk`, `HAS_VERSION`, `CURRENT_VERSION`, and `HAS_CHUNK` semantics.
- No `QuarryFile -> HAS_CHUNK` edge exists.
- No query drops a `QuarryFile`, a `FileVersion`, or chunks for another `version_id`.
- Retrying a version removes/recreates only that version's chunks and does not duplicate the two file/version edges.
- Switching current version removes only `CURRENT_VERSION`; historical `HAS_VERSION` and `HAS_CHUNK` relationships remain.
- An immutable identity collision fails the transaction through the unique index.
- A complete request below 2 MiB succeeds validation; one above it returns an oversize error and is not split.
- Index JSON uses `FileChunk.embedding` with `workspace_id` partitioning.

### Helix get and search tests

- Current-document lookup traverses only `CURRENT_VERSION` and deserializes to `Option<HelixDocumentVersion>`.
- Specific-version lookup traverses `HAS_VERSION`, rejects a version owned by another file/workspace, and returns the same typed result.
- Version-chunk lookup traverses only the requested version's `HAS_CHUNK` edges and returns `Vec<FileChunkResult>` ordered by `chunk_index`.
- A completely empty single-result envelope maps to `None`; a partial, duplicate, or identity-mismatched envelope returns an integrity error.
- Empty search envelopes map to empty typed vectors.
- Search request DTOs use `workspace_id` (`workspaceId` over HTTP), reject the legacy `userId` contract, and validate finite vector values plus a positive bounded limit.
- Vector hits require a numeric `distance`; keyword hits require a numeric `score`.
- Search projections include `chunk_id`, `workspace_id`, `file_id`, `version_id`, and the remaining `FileChunkResult` fields, but omit `embedding`.
- Repository, service, and Axum handler signatures contain no `serde_json::Value` for document get/search operations.

### Service, route, and frontend tests

- Exactly one Helix execution occurs per successfully SQLite-persisted version.
- A SQLite failure prevents the Helix call.
- A Helix failure reports failure while the SQLite version remains retryable.
- `deal_id` reaches `persist_file_blob` unchanged from the Axum path.
- Upload clients generate deal-scoped URLs, and `DataRoomPage` passes its active deal ID into the modal.
- Current-version content lookup returns `Option<HelixDocumentVersion>`; vector and keyword search return their respective typed hit vectors.

### Verification commands

From `backend/`:

```bash
cargo fmt --check
cargo test core::helix_queries::files
cargo test repository::document_repository::tests
cargo test services::document_ingestion_service::tests
cargo test
cargo clippy --all-targets -- -D warnings
```

From `frontend/`:

```bash
npm test -- --run
npm run build
```

## Acceptance criteria

- `Document` and `DocumentChunk` are defined in `document_ingestion_service.rs` and are used only for transient ingestion work.
- `document_node.rs` contains only `FileNode`, `FileVersionNode`, and `FileChunkNode` with the supplied graph fields.
- Graph identity uses only `file_id`, `version_id`, and `chunk_id`; no `file_key`, `version_key`, or `chunk_key` properties are created or indexed.
- `persist_file_blob` reads back and returns the four requested SQLite values only after commit.
- Production ingestion calls SQLite first and executes exactly one Helix request for the full version graph.
- The Helix request uses one atomic `WriteBatch` and one in-request chunk loop; it never splits one file version across requests.
- The graph has exactly the requested edge directions, with no direct file-to-chunk edge.
- Adding a new version retains all old versions and their chunks, and moves only `CURRENT_VERSION`.
- Retrying the same version is idempotent for nodes and edges and repairs that version's chunk set atomically.
- Workspace/file/version identity is denormalized onto child nodes exactly as supplied.
- Vector search uses `FileChunk.embedding` partitioned by `workspace_id`.
- Helix document get/current-version/version-chunk functions return `HelixDocumentVersion` or `FileChunkResult` collections with strict identity and cardinality validation.
- Vector and keyword handlers accept workspace-scoped typed requests and return typed hit arrays; document get/search paths do not expose raw `serde_json::Value`.
- Oversize atomic requests fail explicitly instead of degrading to partial multi-request persistence.
- `deal_id` is authoritative from the route and is checked against SQLite ownership.

## Out of scope

- A distributed transaction spanning SQLite and Helix.
- Silently splitting an oversized version graph into multiple Helix transactions.
- Inferring a logical file from its display name or path.
- A complete replace-version UI or restore-old-version endpoint; the persistence/orchestration API will support callers that reuse a stable `file_id`.
- Preserving legacy Helix data without an explicit migration decision.
- Multiple simultaneous index generations for one `FileVersion`; this phase keeps one current chunk set per version.
