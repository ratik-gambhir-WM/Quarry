# HelixDB v3 SDK and `/v2/query` migration plan

## Decision

Yes: migrate Quarry from the locked `helix-db` 2.0.6 Rust SDK to the current
Rust SDK v3 release (currently `3.0.0`), then adapt Quarry's Helix facade and
document-index queries to the v3 request and result contracts. This is not a
base-URL-only change. The v3 client resolves an instance base URL to
`/v2/query`; `HELIX_URL` must remain the instance base URL and must not itself
gain `/v2/query`.

HelixDB's current README distinguishes the two version numbers: product/SDK
generation **v3** uses HTTP endpoint **`POST /v2/query`**. The
[current README](https://github.com/HelixDB/helix-db#writing-queries-with-the-sdks)
and [Rust SDK documentation](https://docs.rs/helix-db/latest/helix_db/)
identify Rust `helix-db` 3.0.0 and `/v2/query` as the current pairing.

## Goal and boundaries

Move Quarry's Helix document graph, vector search, and keyword search to a
pinned v3-compatible runtime and the v3 Rust SDK without changing the public
Quarry API, SQLite ownership, or document-ingestion lifecycle.

The following contracts must remain stable:

- SQLite is canonical. Ingestion still commits the file/version/blob aggregate
  before attempting the Helix projection, and an exact-content re-upload can
  retry a failed projection.
- The `QuarryFile`, `FileVersion`, and `FileChunk` graph labels; the
  `HAS_VERSION`, `CURRENT_VERSION`, and `HAS_CHUNK` edges; graph identity;
  workspace partitioning; current-version semantics; and graph chunk ordering
  remain unchanged unless a separately approved graph-shape migration says
  otherwise.
- All existing `/api/v1` document, job, and search routes and their client
  DTOs remain unchanged. This work is confined to Axum's server-side Helix
  adapter and document-index repository.
- Write errors remain sanitized at the HTTP boundary. A response with an
  unknown or explicitly non-retryable write outcome must never be replayed.
- The `plans/document-ingestion-xlsx-pptx-plan.md` work is unaffected. Its
  byte parsing, upload UI, and PDF-conversion scope can proceed independently;
  its normal SQLite-to-Helix projection will use this migrated adapter when it
  lands.

Out of scope: changing document parsing, embeddings, SQLite schema, graph
labels/properties, frontend/Tauri transports, or performing a production
Helix reindex. Do not call `clear_helix` as part of development or validation.

## Current migration seams

| Area | Current implementation | v3 migration implication |
| --- | --- | --- |
| SDK and transport | `backend/Cargo.toml` locks `helix-db = "2.0.6"`; `backend/src/adapters/helix/client.rs` builds `DynamicQueryRequest` values and calls the v2 client's dynamic-query API. | Upgrade the crate and lockfile, replace obsolete request types/execution calls, and let v3 construct `/v2/query` from the base URL. |
| Query builders | `backend/src/domains/documents/index/query.rs` and `writer.rs` use v2 DSL/macros, serialized request internals, and v1-specific tests. `backend/src/bin/clear_helix.rs` also creates v2 requests. | Port all compiled query definitions and diagnostics to v3 `QueryRequest`/query-helper APIs; keep the graph's business semantics intact. |
| Writes and retries | The adapter infers writes from `DynamicQueryRequestType`, serializes writes with one mutex, and retries only a substring of v2 `HelixError::RemoteError`. | Make read/write intent explicit at the facade boundary and use v3's structured remote retry classification; retain serialized writes and bounded backoff. |
| Index startup | `DocumentIndexWriter::initialize` issues `create_index_if_not_exists` and treats a successful query response as ready. | Handle the v3 index DDL receipt/lifecycle contract and prove required equality, vector, and text indexes are active before bootstrap accepts traffic. |
| Result mapping | `DocumentIndexReader` expects v2 projection envelopes with `properties` arrays and validates identities after deserializing. | Capture actual v3 responses and adapt only the repository-side wire structs/mappers; preserve the existing identity, empty-result, and rank validation. |
| Runtime image | `backend/helix.toml` selects `ghcr.io/helixdb/enterprise-dev` with `tag = "latest"`. The root `quarry` launcher appends that tag only when creating a missing named container and otherwise reuses any existing container. | Test one concrete compatible image, pin it, and prevent the launcher from silently using the old container after the configuration changes. |

## Implementation plan

### 1. Establish the exact compatible SDK/runtime pair in isolation

1. Keep `HELIX_URL` as an origin such as `http://127.0.0.1:<test-port>`.
   Do not configure an endpoint path, because the v3 Rust `Client` joins its
   base URL with `/v2/query` itself.
2. Upgrade the dependency to the published v3 Rust SDK version selected for
   this change (start with `helix-db` 3.0.0), regenerate `backend/Cargo.lock`
   using Cargo, and inspect every changed transitive package. Do not add a
   second HTTP client or hand-write a `/v2/query` transport.
3. Select the exact `enterprise-dev` image release that is demonstrably
   compatible with that SDK. Record both its human-readable release tag and
   immutable image digest from the successful pull. Do not guess a tag from
   the SDK number: the image's release scheme is independently published.
4. Start that image in a separately named, disposable container on an unused
   loopback port, with no mounted Quarry data, no Docker volume from the
   normal container, and a dedicated `HELIX_URL`. Wait for the runtime's
   documented readiness endpoint before querying it. The existing
   `helix-quarry-dev` container and any of its graph data are out of scope.
5. First prove a minimal v3 SDK write and read reaches `/v2/query`. Capture
   sanitized request/response shapes, HTTP statuses, authentication behavior,
   retry fields, index DDL receipts, and the response shape for a projection.
   These observations—not assumptions based on the v2 request JSON—are the
   input to the adapter port.

### 2. Port the narrow Helix facade before moving document code

Update `backend/src/adapters/helix/client.rs` and its unit tests as the only
place that knows the v3 client execution API.

1. Replace `DynamicQueryRequest`/`DynamicQueryRequestType` and the old
   `client.query().dynamic(...).send()` call with the v3 `QueryRequest` and
   `Client::query(request).send()` surface. Migrate `#[register]` helpers to
   the v3 query-helper macro/API where needed, returning the v3 request type
   and propagating its construction error without panics.
2. Split the facade into explicit read and write execution methods (or an
   equally clear typed operation enum), rather than inferring write intent
   from an obsolete request field. Keep current structured timing logs and
   filename/byte-count context, never document text or API keys.
3. Preserve the single write mutex and the five-attempt exponential backoff
   only for explicitly designated write requests. Replace substring matching
   against a private error body with the v3 `HelixError`/remote-error
   inspection API. Retry only errors the new protocol explicitly marks as
   retryable; terminal, unknown-outcome, validation, unique-constraint, and
   non-retryable errors must be returned once.
4. Add adapter tests with a tiny capture server to prove the facade uses
   `/v2/query` for a base URL, sends an API key only through the SDK's bearer
   behavior, and makes the intended retry/no-retry decisions from structured
   status/code/retryability data. Include the lost/unknown write-outcome case
   explicitly so a future SDK error-string change cannot cause an unsafe
   replay.
5. Retain a single shared client constructed only in bootstrap. Do not move
   ambient configuration reads, query construction, or retry policy into a
   handler, service, or repository.

### 3. Port every index query while preserving graph semantics

Update `backend/src/domains/documents/index/query.rs`, `writer.rs`,
`repository.rs`, and `backend/src/bin/clear_helix.rs` together.

1. Rebuild the six graph operations with v3's supported DSL equivalents:
   graph insertion, current-document lookup, exact-content lookup, historical
   version lookup, ordered version-chunk lookup, and workspace-partitioned
   vector/keyword search. Port index creation in the same pass.
2. Preserve the fields in every `FileNode`, `FileVersionNode`, and
   `FileChunkNode`, all deterministic IDs, the old-current edge replacement,
   version-scoped chunk replacement, chunk ordering, and atomic graph-write
   intent. Do not use an SDK migration as an excuse to change versioning or
   duplicate existing graph nodes.
3. Replace tests that inspect v2-only `BatchQuery`, dynamic parameters, or
   v1 JSON envelope details with semantic serialization assertions against
   v3: labels/edges/projections, bound input values, workspace filtering,
   bounded limit validation, and the absence of legacy `user_id` and
   `document_id` fields. Keep malformed identity/embedding/range tests.
4. Re-evaluate `HELIX_MAX_QUERY_BODY_BYTES` using the selected v3 runtime's
   documented and observed `/v2/query` request limit. Rename the v1-specific
   comment/test, preserve a conservative preflight limit, and reject an
   oversized atomic graph request rather than silently splitting it. Do not
   increase the limit merely because the new endpoint compiles.
5. Port `clear_helix` so all binaries compile against the one SDK version, but
   leave its existing destructive authorization and backup requirements
   unchanged. It is a post-rollout recovery utility, not a migration test.

### 4. Make index readiness and result parsing explicit

1. Use v3's index DDL receipt and operation-status contracts to make
   `DocumentIndexWriter::initialize` idempotent and readiness-aware. A newly
   accepted index build must be waited/polled to a usable active state before
   Axum starts; an already-active index may proceed; a blocked, aborted,
   conflicting, or timed-out index operation must fail bootstrap with a
   sanitized error and useful internal context.
2. Use the exact v3 response captured in the disposable test to revise
   `ProjectionEnvelope`, document-version, chunk, vector, and keyword result
   mappers. Keep strict checks for one file plus one version, matching
   workspace/file/version/hash identities, unique/sorted chunk indices, and
   workspace-scoped search hits. Do not loosen validation solely to absorb a
   new wire shape.
3. Add unit fixtures for valid v3 empty, single-result, ranked-search, and
   malformed/mismatched responses. The fixtures should be synthetic and
   contain no user documents, paths, API keys, or real embeddings.
4. Confirm the SDK's rank numeric type and missing/null-field behavior before
   keeping `f64` score/distance DTOs. If the wire representation differs,
   normalize it in the repository and preserve Quarry's existing DTO contract
   or deliberately coordinate an API contract change in a separate plan.

### 5. Add a disposable runtime compatibility test

Create a backend integration test that is opt-in through a dedicated,
explicit disposable `HELIX_URL` (and skips or is ignored when it is absent).
It must never default to port 6969 or invoke Docker itself. The operator
starts and removes the isolated pinned container; the test only exercises the
URL supplied to it.

Against that instance, test the complete lower-layer sequence:

1. Initialize equality, vector, and text indexes and assert they reach usable
   readiness.
2. Insert a synthetic file/version with two finite, equal-dimension embeddings
   and read it back as the current document, by exact content hash, as a
   historical version, and as chunks ordered by `chunk_index`.
3. Re-submit the same graph and prove it has no duplicate logical nodes,
   current-version edges, or chunks; then exercise the version-scoped
   replacement behavior.
4. Insert a second workspace and prove both text and vector searches never
   return its chunks. Assert rank values parse and the returned chunk fields
   still satisfy Quarry's repository validation.
5. Exercise representative invalid input and index-lifecycle failure handling
   where the test runtime can produce it. Keep no test data after the
   container is removed.

Run this compatibility test once before changing the default local runtime
and again against the final pinned image. It supplements, rather than
replaces, the normal mocked/unit backend suite.

### 6. Pin runtime selection and make old-container reuse visible

1. Replace `tag = "latest"` in `backend/helix.toml` with the successful
   release tag. If the registry permits digest references, extend the config
   and launcher representation so the full `repository@sha256:...` reference
   is used without appending `:<tag>`; otherwise record the tested digest in
   architecture/rollout documentation alongside the immutable release tag.
2. Update the root `quarry` launcher to compare an existing named container's
   configured image reference/digest with the expected pinned reference. It
   must fail with clear instructions to choose a new
   `QUARRY_HELIX_CONTAINER_NAME` or deliberately replace the old container;
   it must not delete, recreate, or start a mismatched existing container.
3. Keep the loopback port mapping and process-ownership rules intact. The
   launcher may create only a missing container; it must not mutate data or
   destroy a previous graph as part of an SDK upgrade.
4. Update `docs/ARCHITECTURE.md` and the root README to describe the pinned
   image, v3 SDK and `/v2/query` relationship, startup readiness behavior,
   existing-container mismatch handling, and the current recovery limits.
   Keep the terminology precise: HTTP API v2 is not product/SDK v2.

### 7. Stage rollout without assuming graph-storage compatibility

1. Land the code, lockfile, pinned runtime config, launcher guard, tests, and
   documentation only after the isolated compatibility test passes.
2. Before any environment with valued graph data changes runtime images,
   drain ingestion, retain a backup of SQLite and Helix under the existing
   operational procedure, and test the exact pinned runtime against a
   disposable copy or fixture. SQLite remains the authoritative recovery
   source.
3. If the v3 runtime cannot read the existing graph or requires a destructive
   graph reset, stop the rollout. Quarry currently has no bulk SQLite-to-Helix
   reindex tool; `clear_helix` alone does not make this migration safe. Design,
   approve, and test export/reindex/recovery tooling separately before any
   destructive action.
4. After rollout, smoke-test health, bootstrap index readiness, one synthetic
   document projection, exact-content retry behavior, and keyword/vector
   search. Record the tested SDK version, image tag, image digest, and runtime
   compatibility result in the release notes/change record.

## Required verification after implementation

Start with focused tests for the changed adapter, query/writer, repository
mapping, index lifecycle, and opt-in disposable runtime test. Then run from
`backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

The disposable test is a separately reported integration result because it
uses Docker and an explicitly supplied temporary Helix URL. Do not use
`cargo run` for ordinary verification: it opens/migrates SQLite and invokes
the normal configured Helix runtime. Do not use `clear_helix` as a test.

Before handoff, inspect the lockfile for intended SDK-only dependency churn,
run `git diff --check`, inspect the final diff, re-run `git status --short`,
and state whether `docs/ARCHITECTURE.md` changed. The implementation has
architecture impact because it changes an external integration protocol,
runtime image policy, bootstrap readiness behavior, and operational rollout.

## Acceptance criteria

- Quarry uses one locked `helix-db` v3 SDK and its documented `/v2/query`
  client path; `HELIX_URL` remains a validated base URL.
- The adapter has explicit read/write intent, structured retry safety, bounded
  writes, sanitized errors, and no error-string protocol coupling.
- Every existing document-index operation preserves graph identity, workspace
  isolation, current-version behavior, embeddings, result ordering, and
  public Quarry API behavior.
- Bootstrap does not declare success until all required v3 index operations
  are usable, and failures leave SQLite canonical data untouched.
- A separate, pinned, disposable Helix runtime passes insertion, retrieval,
  idempotent retry, keyword search, vector search, and response-mapping tests
  without reading or mutating the normal Helix container or graph.
- The local launcher cannot silently reuse an incompatible old image, and the
  architecture documentation records the tested runtime image and migration
  constraints.
