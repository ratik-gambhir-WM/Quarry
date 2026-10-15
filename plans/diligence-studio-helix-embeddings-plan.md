# Diligence Studio Helix embeddings and semantic retrieval plan

## Status, scope, and decision

This is a proposed implementation plan, not the current product contract. The
live implementation and [architecture](../docs/ARCHITECTURE.md) remain
authoritative until the plan is implemented.

Move Diligence Studio's **slide embedding vectors** out of its in-memory
SQLite catalog and into the Helix instance already configured and owned by the
Axum backend. Add server-side semantic similarity retrieval for the two current
embedding roles (`subject` and `capability`) while retaining Studio's existing
metadata, FTS, filters, app scoping, and reciprocal-rank fusion behavior.

The final phrase of the request ended at “and then also do it so we can”; this
plan covers the complete, stated vector-storage and semantic-search work only.
Do not infer an additional user-facing feature from that incomplete sentence.

The implementation must **not** give the Node service a direct Helix client or
the Helix API key. Axum already owns Helix initialization, typed SDK use,
write serialization, graph query validation, and safe error handling. Diligence
Studio will call a narrow, private Axum capability;
browser and Tauri code will continue to call only Axum.

## Current-state findings

| Area | Current behavior | Required change |
| --- | --- | --- |
| Vector generation | `OpenAISlideEmbedder` produces one vector for the subject document and one for the capability document. | Keep generation in Studio for this refactor; send validated vectors to Axum for indexing. |
| SQLite | `slide_classifications` stores documents, fingerprints, model/dimension, Float32 BLOBs, and embedding job state. `SlideVector` serializes BLOBs and computes cosine similarity in Node. | Remove all persisted vector BLOBs and vector serialization/deserialization from SQLite and Node ranking. Retain only the small state needed to classify/retry/report indexing. |
| Semantic search | `SlideRetrievalService` loads every app-visible vector, computes cosine similarity in process, then fuses subject/capability rankings. | Ask Axum to issue Helix cosine-vector searches for each role and fuse the returned ordinal rankings in Studio. |
| Keyword search | SQLite FTS5 stores classification metadata and ranks lexical matches. | Keep it unchanged for the initial refactor; hybrid search remains FTS ranking plus Helix semantic ranking fused with RRF. |
| Helix | Axum currently indexes only the document graph (`QuarryFile` → `FileVersion` → `FileChunk`) and initializes its 13 document indexes at startup. | Add a separate Diligence template-vector node/index family. Do not overload `FileChunk`, document routes, or document identities. |
| Catalog lifetime | Studio's SQLite database is `:memory:`. Built-ins reseed on startup; imported templates and classification rows disappear on restart. Helix persists independently. | Add an app-session lifecycle so stale Helix vectors from a previous Studio process cannot become searchable or accumulate indefinitely. |
| Public inspection | `GET /api/v2/templates/:id?embeddings=true` currently returns raw vectors. | Remove the raw-vector option. Return only a bounded index-status/model/dimension summary when requested; vectors never leave the server-to-server capability. |

The existing retrieval service is currently an in-process future-agent adapter,
not a mounted public search API. This plan makes that adapter use Helix; it does
not by itself add a browser search surface or expose a new public search route.

## Target architecture and invariants

```text
Diligence Studio import / future agent tool
  -> classify template and build two canonical embedding documents
  -> OpenAI embedding request (existing server-only key)
  -> private internal Axum template-vector capability
  -> Axum validates bounded vectors and model space
  -> Axum Helix template-vector repository
  -> Helix DiligenceTemplateEmbedding nodes + cosine index

Diligence Studio semantic query
  -> create one query embedding (or request similar-to-template search)
  -> private internal Axum capability
  -> Helix role-scoped vector search
  -> ranked template IDs only
  -> Studio validates current app-visible metadata, applies filters, and RRF-fuses results
```

The following contracts must remain true:

1. Axum is the only runtime that holds `HELIX_URL` and `HELIX_API_KEY`, creates
   Helix indexes, or talks to the Helix SDK. Studio receives neither value.
2. A vector is scoped to one Studio `appId`, one current catalog session, one
   template ID, one role, and one embedding model/dimension space. No
   cross-app result is returned or post-filtered from a global top-K.
3. Helix stores the vectors and only the metadata needed to address and safely
   validate them. It does not receive PPTX bytes, preview bytes, template JSON,
   raw OpenAI responses, or user query text.
4. SQLite remains the process-local source for template JSON, assets,
   classification metadata, FTS, app membership, and visible template
   identity. Helix is a derived, recoverable template-vector projection, not
   a second canonical template store.
5. Semantic and hybrid responses never expose vector values or raw Helix query
   data. Ranking is based on Helix ordering/RRF rather than treating a
   distance number as a portable product score.
6. When semantic indexing is configured, a failed Helix write is a real
   retrieval failure. Do not silently retain or fall back to SQLite vectors.
   Text-only mode remains available through SQLite FTS.

## Helix graph and index design

Create a new label and repository under the Axum templates domain, for example
`DiligenceTemplateEmbedding`. It is independent of the document graph:

```text
DiligenceTemplateEmbedding
  embedding_id        deterministic: app + session + template + role
  app_id              validated Studio app identity
  catalog_session_id  UUID generated by the active Studio process
  template_id         Studio template identity
  role                "subject" | "capability"
  embedding           Float32 vector
  embedding_model     exact configured provider model name
  embedding_dimension positive integer matching Helix configuration
  input_fingerprint   deterministic embedding-document fingerprint
  indexed_at          RFC 3339 timestamp
```

Index names should be distinct from `DOCUMENT_INDEX_NAMES`, such as:

- `diligence_template_embedding_id_unique` — unique `embedding_id`;
- `diligence_template_embedding_scope` — equality lookup for the exact
  app/session/role/model-space scope when needed; and
- `diligence_template_embedding_vector` — cosine vector index partitioned by
  a canonical `scope_id` property that combines app ID, catalog session, role,
  model, and dimension.

Use the existing `HELIX_VECTOR_DIMENSION` as the only accepted vector size.
Axum validates every incoming vector's finite values and exact length before a
write or a query. The Studio OpenAI embedding dimension must be configured to
that same value; reject the capability handshake or write on a mismatch rather
than allowing two incompatible vector spaces into one index. Include the model
in `scope_id` so a model change cannot compare vectors from different models
even if their dimensions happen to match.

Use a deterministic ID for each current role. Re-indexing the same template
role updates/replaces that node rather than creating another match. A successful
write must be idempotent for identical input and replace the old node when its
fingerprint, model, or vector changes.

Before implementation, add an ignored, disposable-runtime Helix v3
compatibility test that proves the pinned `helix-db = 3.0.0` SDK can:

1. initialize these new indexes alongside the document indexes;
2. insert, replay, replace, and remove one role-scoped node;
3. return only the requested `scope_id` from a cosine search; and
4. return finite, deterministically mapped results for both roles.

Do not run that test against the developer's normal Helix container or use
`clear_helix` as part of this work.

## Internal Axum capability

Add a private router mounted outside `/api` and `/api/v1`, for example
`/internal/v1/diligence-template-vectors`. It is for the Studio process only;
it must not be added to `QuarryApi`, browser adapters, Tauri commands, or
public API documentation.

The initial refactor deliberately adds no server-to-server credential or
authentication middleware. Keep the router out of public clients and validate
the configured internal base URL as HTTPS or explicit loopback HTTP, with no
credentials, query, or fragment. Treat route authentication and production
network policy as a follow-up security design; the absence of a credential
must be documented rather than implied to be secure.

Provide the following typed operations:

| Operation | Input | Output | Purpose |
| --- | --- | --- | --- |
| `POST /sessions` | valid `appId`, fresh session UUID | accepted session summary | Atomically make one Studio catalog session current for that app and remove only prior `DiligenceTemplateEmbedding` nodes for that app. |
| `POST /upsert` | app/session/template IDs, model, dimension, two role vectors, fingerprints | stored role summary | Validate and idempotently replace both current role nodes. |
| `POST /query` | app/session, role, model/dimension, query vector, bounded candidate limit | ordered `{templateId, role}` candidates | Run one exact-scope Helix vector search. Do not return vector values. |
| `POST /similar` | app/session, source template ID, requested roles, model/dimension, bounded limit | independent ordered candidate lists per role | Read the source vector inside Axum and search its role scope, without sending source vectors back to Studio. |
| `DELETE /{appId}/templates/{templateId}` | validated path IDs plus active session | no content | Delete both role nodes for exactly that app/session/template before Studio removes app visibility. |

Body schemas need explicit maximums for IDs, model names, two roles only, exact
vector count/length, and a conservative serialized body cap. Validate all
caller-controlled values in the handlers; services receive already typed,
trusted inputs. Map Helix transport and integrity failures to sanitized 503 or
500 responses and log only operation name, opaque IDs, counts, duration, and
error category.

Implement this as a focused Axum layer, not as additions to
`documents::search`:

- `backend/src/domains/templates/index/` owns node models, Helix writers,
  query builders, response validation, and index bootstrap;
- `backend/src/domains/templates/vector_service.rs` owns lifecycle, upsert,
  deletion, and query/similarity orchestration;
- `backend/src/domains/templates/vector_handler.rs` and
  `vector_route.rs` own private transport validation; and
- `backend/src/app/bootstrap.rs` constructs this service from the existing
  shared `HelixClient` and initializes the new indexes before accepting
  requests.

Keep the existing `DocumentIndexWriter`, document graph, and `/documents/search/*`
routes untouched except for any small shared Helix bootstrap abstraction that
prevents duplicate index-readiness polling. Read operations remain read-only;
all Helix writes continue through the shared process-serialized client.

## Studio refactor

### 1. Add a narrow index client

Create a `SlideVectorIndex` interface in Diligence Studio with methods for
session activation, dual-role upsert, role query, similar-to-template query,
and app/template deletion. Its only production implementation should be an
HTTP client for the private Axum routes. Give it strict request/response Zod
validation, request timeouts bounded by the existing Studio request budget,
abort-signal forwarding, no retry of ambiguous writes, and one stable
`slide_index_unavailable` error family.

The Studio server creates a fresh `catalogSessionId` once at startup, activates
the default app's session before accepting retrieval-enabled imports, and passes
the client/session provider to `SlideClassificationService`,
`SlideRetrievalService`, and `ImportTemplateService`. Do not import a Helix
package or use `HELIX_*` configuration in Node.

### 2. Change classification/indexing flow

Refactor `SlideClassificationService.process` to keep the generated subject
and capability documents/vectors in memory only:

1. classify and normalize the template as today;
2. construct the two deterministic embedding documents and fingerprints;
3. request and validate both OpenAI vectors;
4. call the Axum `upsert` operation with the two vectors, model, dimension,
   fingerprints, app ID, session, and template ID; then
5. atomically publish local classification metadata/FTS and an
   `embeddingStatus: ready` summary without writing vector BLOBs.

If the Helix write fails, record a sanitized failed embedding status and fail
the synchronous v2 import. If local publication fails after a confirmed Helix
write, attempt the exact scoped delete as compensation, log only opaque
identities, and leave the local template non-searchable. Retrieval must always
join Helix candidate IDs to current app-visible SQLite records, so an orphaned
projection cannot be rendered even if compensation is interrupted.

Preserve deterministic fingerprints and the model/dimension summary so retries
can safely replace a prior projection. Do not persist the canonical embedding
documents merely to rehydrate a background worker; rebuild them from the
already stored normalized metadata, title, and kind when the worker is enabled
again. Update `SlideEmbeddingJob` accordingly.

### 3. Remove SQLite vector storage

Bump the in-memory catalog schema from version 5 to version 6. Replace the
`slide_classifications` table with a version that retains classification
metadata, FTS inputs, embedding status, model/dimension, fingerprints, retry
lease/error fields, and no vector or embedding-document columns. In particular,
remove:

- `subject_embedding` and `capability_embedding` BLOBs;
- their Float32 serialization/deserialization in `SlideVector.ts`;
- `subject_embedding_document` and `capability_embedding_document`; and
- public types containing `subjectVector` or `capabilityVector`.

The migration may discard only derived classification/indexing state if that is
the repository's established in-memory migration policy; it must not remove
template JSON, assets, previews, app memberships, or template metadata. Rebuild
the FTS rows from surviving normalized metadata in the same transaction. Add a
migration test that starts from a v5 fixture with vectors, verifies the old
vector columns are gone, metadata/templates remain, and FTS search still works.

Delete `serializeEmbedding`, `deserializeEmbedding`, and
`cosineSimilarity` once no code uses them. Retain a small `validateEmbedding`
helper only if it remains the shared provider-input validation; otherwise move
that validation next to the OpenAI/index client contracts.

### 4. Redirect retrieval to Helix

Refactor `SlideRetrievalService` as follows:

- `text`, `filter`, `by_id`, and `facets` retain their current SQLite behavior.
- `semantic` creates one query embedding, calls the index client once per
  requested role, joins returned template IDs to the app-visible/filter-matched
  records, and returns the Helix ordinal ranking.
- `semantic_target: both` runs subject and capability lookups concurrently and
  uses the existing deterministic RRF helper on their rank lists.
- `hybrid` RRF-fuses the SQLite FTS rank and the Helix semantic rank. A semantic
  outage yields the existing explicit `semantic_unavailable` warning for hybrid
  mode and an error for semantic-only mode; it never computes cosine locally.
- `similar` invokes the Axum `similar` operation for the source template and
  applies the same two-role RRF. It no longer reads source vectors from SQLite.
- `findForFinding` creates its two query vectors and asks Helix for the
  corresponding role rankings before applying the existing exact metadata boost.

Ask Helix for a bounded candidate pool larger than the requested result count
(for example `min(100, max(50, limit * 5))`), then apply Studio's current
metadata filters and app-visible-ID validation. Retrieval remains approximate;
if a constrained filter eliminates candidates, return fewer results rather than
claiming an exhaustive top-K. The compatibility spike must establish the pinned
SDK's safe per-query limit and whether a predicate can be applied before ANN
candidate selection. Do not add an unbounded all-vector read as a workaround.

### 5. Preserve lifecycle correctness

Studio's catalog is not durable, so simply retaining vectors in Helix would
make old imported templates searchable after a restart. On each Studio boot,
`POST /sessions` replaces the prior session only for that app and only for the
new Diligence template-vector label. This is intentionally scoped cleanup, not
`clear_helix`, and is safe only under the existing single-process catalog
assumption. Document and enforce that Diligence Studio cannot run multiple
catalog processes for the same app without a shared catalog/lease design.

For deletion, make `ImportTemplateService.delete` asynchronous:

1. verify the template is visible to the requested app;
2. delete its exact app/session vector nodes through Axum;
3. remove the `app_templates` membership and cascade local assets only after
   the vector delete succeeds; and
4. return a sanitized unavailable error if indexing is unreachable so callers
   can retry without creating a stale semantic result.

Template updates/reclassification use the same upsert identity and therefore
replace both roles. Seeded and v1-imported templates remain unclassified until
an explicit, separately approved backfill is designed; this refactor must not
silently classify or send all built-in slides to OpenAI at startup.

## Public contract, configuration, and documentation changes

- Preserve `/api/v2/import`'s success condition: it returns `201` only after
  both vectors are accepted by Helix and local retrieval metadata is published.
  Update its error mapping for private-index timeout/unavailability.
- Change v2 template inspection from `embeddings=true` to a non-sensitive
  `indexStatus=true` summary (status, model, dimension, and fingerprints if
  needed for diagnostics). Reject `embeddings` as an invalid query parameter;
  never proxy raw vectors from Helix.
- Add validated, server-only configuration for the private Axum base URL in
  Studio. Require the Diligence embedding dimension when this mode is enabled
  and verify it against Axum's `HELIX_VECTOR_DIMENSION` during
  startup/session activation. Do not put this value in `VITE_*`, Tauri IPC,
  browser storage, or API responses.
- Update `diligence-studio-server/README.md` to describe Helix as a derived
  template-vector projection, remove the claim that v2 inspection returns raw
  vectors, and document the new configuration and single-process lifecycle.
- Update `docs/ARCHITECTURE.md` to distinguish document and template-vector
  Helix projections; document the private service boundary, storage ownership,
  session cleanup, configuration, verification, and the remaining absence of
  production identity/tenant authorization.

## Implementation sequence

1. **Prove the graph contract.** Add template-vector model/query/writer tests
   and the ignored disposable Helix v3 compatibility test. Confirm labels,
   index names, partition scope, replacement, exact-session isolation, delete,
   and similarity ordering before wiring HTTP.
2. **Build the Axum internal capability.** Add private-base-URL validation,
   DTO/handler validation, templates-domain index repository/service, bootstrap
   index readiness, and focused route/service tests. Keep it unreachable from
   public and frontend contracts.
3. **Add the Studio index adapter.** Implement configuration validation,
   startup session activation, typed HTTP client, error translation, and mocks
   for deterministic unit tests. No direct Node-to-Helix dependency.
4. **Refactor the write path.** Change synchronous v2 processing to upsert
   both Helix roles before publishing local ready state; implement compensating
   deletion and retry-safe status behavior.
5. **Migrate SQLite types/schema.** Remove BLOB/vector/document fields and
   local vector math; preserve metadata/FTS and prove v5 migration behavior.
6. **Refactor semantic retrieval.** Replace local cosine ranking in every
   `SlideRetrievalService` semantic path with the index adapter, preserving
   filters, app joins, deterministic RRF, warnings, and bounded candidate
   behavior.
7. **Close lifecycle and public-contract gaps.** Make deletion async and
   projection-first; remove raw-vector inspection; update tests, READMEs, and
   architecture documentation.
8. **Run verification and review the final diff.** Do not start the normal
   backend against a valuable SQLite/Helix environment as a substitute for
   tests.

## Acceptance and verification matrix

### Diligence Studio tests

- Schema migration retains templates/metadata/FTS while removing vector BLOB
  columns and public vector fields.
- v2 import writes two exact-role vectors through a mocked index client and
  reaches `ready` only after both succeed.
- index write timeout/error marks the retrieval status failed, returns the
  current sanitized error, and stores no vectors in SQLite.
- semantic, hybrid, similar, and finding retrieval use ranked index-client IDs,
  retain app boundaries and filters, and preserve deterministic RRF/tie breaks.
- raw-vector inspection is rejected; the index-status response reveals no
  vector values.
- deletion refuses to remove local app visibility when exact projection deletion
  fails; successful delete removes both role vectors.
- startup session replacement prevents a prior session's vector from being
  returned.

Run from `diligence-studio-server/`:

```sh
npm run typecheck
npm test
```

### Axum tests

- query-builder tests validate nonempty/finite/exact-dimension vectors, scoped
  deterministic IDs, role/session/model isolation, payload limits, and no
  document-label reuse;
- repository tests map only matching app/session/template identities and reject
  malformed Helix responses;
- handler tests reject malformed JSON before service calls and never log vector
  data; and
- service tests cover idempotent replace, scoped deletion, source-template
  similarity, and sanitized index failures; and
- the ignored compatibility suite passes only against an explicitly supplied
  disposable Helix URL.

Run from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Finally run `git diff --check`, review status for generated artifacts or
secrets, and re-read the applicable Helix, Diligence Studio, configuration,
security, and verification sections of `docs/ARCHITECTURE.md` before handoff.

## Explicit non-goals and open decisions

- This plan does not make Studio templates or their classification metadata
  durable. The in-memory catalog's restart behavior remains unchanged.
- This plan does not add a general cross-product search UI, public slide-search
  endpoint, generated OpenAPI client, backend SQLite template catalog, or
  direct browser/desktop connection to Helix.
- This plan does not migrate document `FileChunk` embeddings or alter the
  existing document search routes.
- A future durable/multi-instance Studio catalog needs a separately designed
  canonical template store and distributed session/lease model; do not solve
  that opportunistically in this refactor.
- Before implementation, confirm whether the intended next feature after the
  truncated request is a public template search API, agent-tool integration,
  finding-to-template matching UX, or something else. That choice changes the
  public contract and should be planned separately.
