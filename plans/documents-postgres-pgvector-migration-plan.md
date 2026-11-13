# Plan 1: document chunks, embeddings and search in PostgreSQL

Date: 2026-11-12. Status: design for a future implementation PR; no runtime changes made.
First of three migrations: [notebooks](notebooks-postgres-pgvector-migration-plan.md), then
[templates and final Helix retirement](templates-postgres-pgvector-migration-plan.md).
Reinspect code/manifests before implementation. Current code, not these proposed names, remains
authoritative. Relevant baseline: [architecture](../docs/ARCHITECTURE.md),
[storage](../docs/postgresql-storage.md), [hybrid search](../docs/hybrid-document-search.md).

## 1. Outcome and current behavior

Replace `helix.document_indexes.initialize` and every document graph read/write/delete with
PostgreSQL tables, pgvector cosine search and indexed keyword retrieval. This includes preview
text, chunk lookup, current-version selection, authorization snapshots and deletion; replacing
only vector search would leave Helix mandatory. Do not implement a second graph database in SQL:
file/version/chunk foreign keys and joins express the relationships already needed by the product.

Today original bytes, versions, identity manifests and accepted image text are PostgreSQL-owned,
but full chunks/vectors and search are Helix-owned. Ingestion prepares embeddings, commits the
canonical file, then publishes multiple Helix requests. Keyword hybrid retrieval currently transfers
canonical candidate chunks and ranks them in Rust. Document job state is process-local.

The replacement commits a complete document version, all chunks and embeddings, search metadata,
current pointer and activity in **one SQL transaction per document**. One failing chunk rolls the
whole transaction back. A multi-file upload may succeed for A and fail for B; there is no transaction
across unrelated documents. Provider calls, parsing, image interpretation and Office work precede
the transaction. The prior complete version remains visible until replacement commit.

## 2. Common storage and search decisions for all three plans

This section owns shared implementation choices; later plans reuse them rather than introduce
independent pools, extension installers, query languages or vector codecs. Keep repositories
feature-owned and share only proven PostgreSQL mechanisms.

### 2.1 Extension, schema and Rust boundary

- Use the existing bounded SQLx PostgreSQL pool. The inspected crate uses edition 2021, Rust
  1.97.1, Axum 0.8.9 and SQLx 0.8.6; preserve actual versions/edition unless required by a separately
  reviewed change. Evaluate/pin the official `pgvector` Rust crate's SQLx feature with that lockfile.
  [Rust integration](https://github.com/pgvector/pgvector-rust) documents the supported adapter.
- Provision the server's `vector` extension explicitly for Postgres.app 18 and deployment/test
  images. A Cargo dependency does not install a PostgreSQL extension. The migration role installs
  approved extension objects once per database in a controlled extension schema; application
  startup validates version/types/operators/index readiness and gives a sanitized actionable error.
  No automatic download, administrator escalation, server restart or dimension-changing rebuild.
- Keep extension objects separate from isolated product/test schemas. Qualify types/operators or
  use a tightly controlled search path. Verify SQLx codec type resolution in a small integration
  spike. Update catalog fingerprints to distinguish owned DDL from extension-owned objects; check
  extension version separately. Never bless arbitrary drift or install/drop the extension per test.
- Add narrow typed vector/ranking/bulk support under `adapters/postgres`; bind vectors and text.
  Do not put SQL in handlers/services, interpolate caller identifiers/operators or add an arbitrary
  raw-SQL escape hatch. New bulk helpers operate on the caller's retained transaction/connection,
  preserve read-only enforcement and cannot acquire/commit another connection.
- Use validated prepared-batch types, typed IDs/profile IDs and domain errors. Validate lengths,
  finite values and finite nonzero norms both before and after any f64-to-f32 conversion. Keep
  parsing/CPU-heavy export off Tokio workers, bounded provider fan-out and cancel-safe rollback.
  Follow `$rust-skills` subject to Quarry's stronger test-placement and layer rules; no unsafe
  vector codec, blanket trait framework, unreviewed `as` narrowing or production `unwrap`.
- Trace operation IDs, stages, bounded counts, durations and error categories. Skip document text,
  query text, vectors, accepted snapshots, credentials and full bound-parameter payloads in logs.

Shared mutation lock order for the SQL-only pipelines: sorted deal lifecycle rows; user-scoped
admission rows when allocating/retrying; sorted resource parents (file/notebook/folder/note or
template/thread); sorted operation/job rows; then sorted admission slots. Slot allocation uses
SKIP LOCKED after the relevant parent/job is protected; no path holds a slot while waiting for a
parent. This deliberately replaces the earlier continuation's slot-before-parent ordering so
parent deletion can cancel jobs and release their slots without inversion. Terminal transitions
only reduce per-user active counts and need not reacquire the admission row. Claim-only queue
transactions touch jobs, not parents/slots. Multi-parent deletion obtains parents in canonical
order, then its jobs/slots; prove the complete admission/retry/delete/finish protocol in tests.
Lease predicates use the database clock at the final mutation, not a timestamp frozen at
transaction start; a batch that outlives its attempt must fail its final fence and roll back.

### 2.2 Vector representation and retrieval policy

Persist full f32 `vector(D)` values, not opaque BYTEA, in dimension-pinned domain tables. An
immutable embedding profile records provider/model, dimensions, distance metric and preparation
version. Every batch/query declares its profile; equal dimensions alone do not mean compatible
models. Documents and notebooks share a validated profile policy; templates may use a separate one.
No automatic padding, truncation, model substitution or mixed-profile comparison.
Validate storage dimensions against the pinned extension's supported range (currently at most
16,000 for `vector`), independently from the smaller ANN indexing limits.

Upstream pgvector supports exact search and HNSW/IVFFlat; standard ANN `vector` indexes stop at
2,000 dimensions, while `halfvec` indexes support 4,000. Thus 3,072-dimensional storage is valid
but needs a different ANN representation. [pgvector reference](https://github.com/pgvector/pgvector).

Our choice: exact scoped cosine SQL is the correctness baseline and default for small/strongly
filtered corpora. Enable HNSW only after measurements; do not choose IVFFlat's additional training/
probe tuning initially. For D <= 2,000 use the full vector index; for 2,000 < D <= 4,000 evaluate an
HNSW expression over `embedding::halfvec(D)`, retaining full vectors and reranking candidates with
full-precision cosine. Reject overflow/zero-norm half representations. Above that ANN range, use
bounded exact search or an explicitly approved model/index redesign. Do not silently shrink vectors.

ANN filters can underfill results. Keep authorized scope, active generation and profile predicates
inside candidate queries, measure selective scopes, and use bounded iterative scans/oversampling
plus full-precision reranking. Exact fallback uses the same scoped SQL and deadline. Do not retrieve
global top-k then discard inaccessible rows, manufacture a full result set or claim exact recall.
Use transaction-local tuning; pooled sessions must not leak HNSW settings between requests.

Candidate index DDL and matching query casts must be fixed by migrations/profile configuration,
not request input. Benchmark partial/partitioned indexes only when measured scope skew warrants
them; do not create one table/index per deal or owner by default. Keep relational scope indexes.

### 2.3 Keyword design and evaluated alternatives

GIN on stored `tsvector` is PostgreSQL's native **inverted index**. GIN locates matching lexemes;
ranking is separate. [PostgreSQL index guidance](https://www.postgresql.org/docs/18/textsearch-indexes.html).
The native rankers consider frequency/proximity/field weights but are not corpus-statistics BM25.
[Ranking and query parsing](https://www.postgresql.org/docs/18/textsearch-controls.html).

| Candidate | Fit and decision |
| --- | --- |
| Native `tsvector` + GIN + `ts_rank_cd` | Recommended document/notebook baseline: no additional search engine, ordinary PostgreSQL transaction/backup path, field and phrase ranking. Must measure ranking/sort cost on common terms and large scoped corpora. |
| `pg_textsearch` BM25 | Serious benchmark alternative when native relevance/top-k cost is insufficient. Upstream supports PG17/18 but documents corpus-statistics leakage across RLS scopes; prove acceptable isolation, rollback/crash behavior and operational support before adoption. [Primary project](https://github.com/timescale/pg_textsearch). |
| ParadeDB `pg_search` | Alternative PostgreSQL extension with BM25 search capabilities, not a required sidecar. Evaluate pinned PG18/macOS/deployment support, authorization, WAL/restore, maintenance and licensing. Do not substitute its vector engine for the requested pgvector design. [Primary documentation](https://www.paradedb.com/docs/start/introduction). |
| Existing template postings/weighted BM25 | Already an SQL inverted posting design with captured parity. Preserve it in Plan 3; native GIN rank is not a drop-in equivalent to its fifteen weighted fields. |
| `pg_trgm` | Optional indexed filename/title typo or substring candidate path; not a replacement for linguistic search or BM25. [PostgreSQL trigram documentation](https://www.postgresql.org/docs/18/pgtrgm.html). |

Recommendation is an engineering choice, not a claim that one ranker is universally best. Before
merging search replacement, run a bounded comparison of native GIN and the two BM25 alternatives
on representative chunks/queries. Record NDCG@10, MRR, recall, filtered p50/p95, common-term cost,
write/rollback latency, index bytes, update/delete churn, crash/restore and extension installation.
Keep native GIN unless another candidate improves judged quality or meets a demonstrated scale need
without correctness/security/operational regressions. Do not ship multiple configurable lexical
engines or an automatic runtime switch. A changed decision must update all affected plans/tests.

For document/note text choose explicit, versioned `english` body parsing plus a separately weighted
`simple` technical-token channel. Test names, acronyms, C++, .NET, identifiers, diacritics, phrase,
prefix, punctuation and stopword-only inputs. Add a bounded domain normalizer only for demonstrated
misses; native `simple` parsing alone does not preserve every identifier. Language metadata permits
future dedicated configurations; do not claim multilingual quality from an English stemmer.

Store search text/vector on the chunk, maintained transactionally with explicit configuration,
not asynchronous triggers or whole-document tsvectors. Pin normalization; reject excessive lexemes/
input or deliberately split before persistence. [PostgreSQL FTS limits](https://www.postgresql.org/docs/18/textsearch-limitations.html)
include vector size/position bounds. Choose the normalized text used for both indexed text and
matches; never return unsanitized `ts_headline` HTML. Parse user text with bound safe query functions;
any prefix operators are built from validated tokens, never concatenated raw query syntax.

### 2.4 Atomic bulk rule

Several bound INSERT batches on **one transaction** are atomic; one SQL statement is not required.
Start with bounded multi-row inserts sized below parameter/memory limits. Use checked arithmetic
for row/parameter counts. Never `ON CONFLICT DO NOTHING` to conceal a missing chunk. Conflicts
must be verified exact replays or fail the complete batch. Count/ordinal/hash/profile checks precede
publication; verify affected counts and commit constraints. Index maintenance belongs to that commit.

Benchmark COPY FROM STDIN only if inserts are inadequate. It must use the same transaction,
fail on the first error, validate complete input/counts and finish/abort the protocol before commit/
rollback; discard an unusable connection. Never COPY to server paths, use error-skipping options,
commit partial batches or use savepoints to retain successful chunks. [COPY behavior](https://www.postgresql.org/docs/18/sql-copy.html).
Enforce document/chunk/byte/vector limits before holding locks. Oversized documents fail explicitly;
streaming preparation to private temporary storage may bound memory but cannot weaken atomicity.

Attempt fencing prevents stale SQL results; it does not promise exactly-once provider execution.
If a provider outcome is uncertain before durable result capture, use supported provider idempotency
or an explicit recorded retry policy that acknowledges possible repeated cost. Never infer that an
expired lease stopped the old HTTP call. A client disconnect after durable admission is not server
cancellation; keep work/status recoverable and distinguish local subscription cleanup from cancel.

## 3. Proposed document relational model

Proposed names are design targets, not migrations already present. Keep one authoritative identity
mapping and extend existing records instead of maintaining duplicate independently updated graphs.

| Record | Ownership and constraints |
| --- | --- |
| `quarry_files`, `quarry_file_versions`, blobs | Retain canonical IDs, current uniqueness, content hash, bytes, attribution and deal FK. Add/reuse a generation/profile readiness reference only if it is the sole searchable-current selector. |
| `document_identity_manifests` | Retain derivation/document IDs and expected chunk count; introduce profile/preparation fingerprint and complete search-generation state. Replace `graph_migrated` semantics with explicit SQL readiness; do not just mark incomplete rows ready. |
| `document_chunk_identities` | Retain stable chunk IDs, ordinal/hash/version/generation and uniqueness. This remains the authoritative identity relation. |
| Proposed `document_chunks` | One-to-one chunk-ID FK, text, section, page and character spans, vector(D), search tsvector and immutable profile identity. NOT NULL for required text/vector; finite/nonzero/profile and range checks. Add composite FKs to prevent mismatched deal/file/version/generation if denormalized for filtering. |
| Proposed document jobs/input | Durable bounded accepted input/hash/operation ID, canonical winning identity, attempt token/DB-time expiry, sequence, stage and terminal result. Accepted work is not a searchable document and is not an uploaded activity event. |

Preserve the existing product identity policy: an ordinary changed-content upload creates a new
logical file; do not merge by filename or invent a public file-replacement operation. Existing
explicit logical-file/version operations retain their own uniqueness/current rules.

Index text with GIN, scope/current/version joins with B-trees and vectors per section 2. Keep unique
ordinals and validate that expected count equals actual count, with no gaps or extra vectors. Retain
readiness so a partial legacy/backfill set is never searchable. Historical versions stay addressable
only by their existing authorized APIs; normal retrieval sees current, live, ready versions only.

## 4. Prepare once, commit once

1. Authenticate/validate transport and policy before protected work. Durable admission reserves
   stable operation/input and scoped identity coordination; return existing matching work on replay.
   Exact-content identity is selected with a unique claim, not only a process mutex or a lock on an
   absent row. Resolve simultaneous random parser IDs to one canonical winner.
2. Parse/chunk/describe images/embed outside SQL locks. Persist immutable accepted image text in
   the operation checkpoint before retry could call vision again. Checkpointing pending work is
   separate from committing the canonical document. Prepare all chunk IDs/spans/vectors/profile,
   counts/hashes and encoded payload bounds. Reject even one invalid/missing embedding.
3. Open one short mutation transaction. Acquire shared deal lifecycle authority, required sorted
   file/identity rows, then current job attempt. Recheck actor membership, archive/deletion policy,
   operation payload, current attempt/status/DB-time expiry and version identity at point of use.
4. Write file/version/blob/manifest/identity rows, every chunk/vector/lexical row, and activity through
   helpers on this transaction. Allocate version numbers under the logical file lock when applicable.
   New readiness/current selection is published only after the complete batch is verified. Update
   job terminal result and committed identity in the same transaction under its final attempt fence.
5. Commit once. A mid-batch error, index failure, count mismatch, constraint failure, cancellation,
   stale attempt, revoked access or commit failure rolls back canonical rows/current pointer/activity.
   The accepted job may remain failed/retryable, but never report `sqliteCommitted=true` from input
   admission alone. Retry uses immutable accepted input; after an uncertain commit query operation ID.
6. No Helix publication follows. The response/SSE is emitted after commit; reconnect on another
   process reads durable job state. Do not rerun providers because an acknowledgment was lost.

This intentionally eliminates the old canonical-commit-then-graph-failure state for new ingestion.
Keep existing public event names/identity fields; update partial-failure tests and documented wire
semantics together. An already committed document may fail later response delivery, so stable-ID
reconciliation still matters. If provider preparation fails, the previous canonical state survives.

## 5. SQL search, viewing and deletion

Replace materialized lists of every canonical chunk ID with scoped SQL joins. Both branches use
one authorized snapshot of live/current/ready versions; after query embedding (outside SQL), run
keyword/vector candidates in one read-only snapshot. Scope and profile restrictions precede branch
limits; no cross-user snippets/counts/vectors are returned. Recheck only bounded returned identities
and session policy immediately before delivery to retain existing revocation behavior.

Preserve `/api/v1/documents/search/{keyword,vector,hybrid}` and deal-scoped equivalents, hybrid opt-in,
query/result bounds, `{results}`, chunk identities, page spans, `matchSources`, explicit semantic
unavailability and keyword-only fallback behavior. Rank branches separately and retain deterministic
RRF/stable tie ordering; do not add raw cosine and lexical scores. Preserve empty/all-stopword behavior
explicitly and distinguish unavailable semantic search from an empty successful result.
Text-query services carry the server-selected embedding profile with their generated vector. The
existing raw-vector endpoints resolve to their documented server profile; a model cannot be inferred
from floats or dimensions. Supporting caller-selected profiles would be an explicit coordinated API
change, not an assertion that a same-size vector proves which model produced it.

Replace `DocumentGraphReader` consumers for raw text/previews/assistant context and `ResourceAccess`
checks with SQL repositories. Current authority comes from membership/session and canonical rows,
never a cached result or caller-supplied deal list. Keep PDF/page/filename/size/URL contracts intact.
SQL file/deal deletion cascades through chunks/search metadata under the existing lifecycle barrier.
Remove document remote fence/purge/verify calls and graph checkpoints only when no document reader/
writer needs them; retained historical phase strings remain wire-compatible and gain documented
SQL-only transitions. Late workers cannot recreate data after purge: check retained lifecycle and
current attempt inside their mutation; deleted IDs remain reserved.

## 6. File ownership and implementation sequence

| Existing area | Change |
| --- | --- |
| `backend/src/adapters/postgres/{client.rs,query/}` | Shared typed vector/bulk/ranked query support and transaction-bound execution. |
| `backend/src/app/{schema.sql,migrations.rs,config.rs,bootstrap.rs}` | Extension/profile readiness, owned SQL indexes, new repository wiring; replace document index initialization only. Keep notebook/template Helix startup until their plans land. |
| `backend/src/domains/documents/{ingestion,store,graph}/` | Durable admission/preparation/final commit; move reusable chunk DTOs out of Helix query modules into document-owned models. Introduce narrow SQL chunk/search repositories and remove document graph code when no consumer remains. |
| `backend/src/domains/documents/{search,viewing}/`, `domains/identity/access.rs` | SQL candidates/snapshots, text retrieval, bounded final authorization; retain fusion and preview contracts. |
| `backend/src/domains/deals/deletion/` | SQL chunk deletion/verification, durable attempts and lifecycle fencing; preserve receipts/worker recovery. |
| `backend/tests/` | Replace graph-only fixtures with SQL repository tests and failure injection, preserving format/image/exact retry/HTTP/auth coverage. |

Implementation order: (1) freeze API/identity fixtures and benchmark lexical/vector alternatives;
(2) provision extension and add typed/profile/schema helpers; (3) implement atomic repository with
injected failures; (4) connect durable jobs and provider preparation; (5) replace search/read/delete;
(6) cut document traffic to PostgreSQL and remove its Helix startup/call sites; (7) run gates and
update architecture/recovery guidance. No permanent dual-write/fallback architecture. A temporary
offline comparison tool must be explicit, read-only against old data and removable after acceptance.

## 7. Rollout, recovery and related concurrency work

The current database was previously reset at the user's direction; do not assume it is still empty.
At implementation choose an explicit fresh-database cutover (no import needed) or a preserving
migration after inspecting counts. Fresh mode selects a new empty DB/schema and a new version;
startup never drops old state. Preserving mode keeps schema-2 validation, introduces new DDL and
backfills complete documents in per-document transactions while writers are drained.

SQL identity manifests alone do not contain all old text/vectors. For populated data, export validated
Helix chunks while available, or reparse/re-embed canonical bytes with an explicit identity mapping.
Reuse accepted image text; do not silently regenerate descriptions/change IDs. Count/hash/span/profile
checks and atomic readiness precede switching readers. Mark unrecoverable records incomplete and
stop cutover with a report. Never acknowledge successful migration with missing documents.

Keep old stores intact until backups/readback/restore are verified. No automatic destructive reset,
Helix container/volume deletion or live re-embedding spend. Rollback drains workers and requires
schema/profile/API compatibility; do not restore an old binary that cannot read the new schema.

This supersedes document Helix projection-intent/remote-fence work in the
[concurrency continuation](durable-claims-remote-fencing-and-recovery-plan.md). Those remote writes
cease to exist; replace them with current-attempt/resource checks in the same SQL commit. Durable
admission, recovery, replica budgets and the global-writer removal acceptance gates still apply.
Do not remove global writer controls merely because this domain now uses one database.

## 8. Acceptance and verification

Required real PostgreSQL tests, manually wired under `backend/tests/`:

- Fail chunk 1, a middle chunk and final chunk, including SQL constraint/index errors after earlier
  INSERT batches succeeded. Verify zero new canonical rows, no activity, unchanged old current
  pointer and no lexical/vector hits. Include deferred failure at commit and request cancellation.
- Two connections pause before commit: readers see the old complete version; after commit they see
  all new chunks. No subset/mixed generation ever appears. Successful count/hash/profile match.
- Concurrent same-hash uploads with different parser IDs converge; exact retry preserves identity,
  accepted image text and one activity. Different documents commit independently.
- Expired/stolen job attempt, revoke/archive/delete while provider is paused, process death before/
  after commit and lost response: no stale write, no resurrection or duplicate provider replay on
  an already committed operation. Status/reconnect works across independent processes.
- Real vector codec/index tests for 1,536 and 3,072 dimensions, invalid f64/f32/norm values, profile
  mismatch, selective ANN filters and exact reference ranking; verify real query plans/index use.
- Keyword fixture quality/technical tokens/phrases, authorization before limits, stable RRF ties,
  page spans/match offsets and safe snippets. Backend and web/desktop contracts stay compatible.
- Explain-analyze and relevance harness: exact top-k is ground truth; proposed ANN gate recall@20
  >= 0.95 for every evaluated scope/selectivity bucket. Record NDCG/MRR, p50/p95, ingest rollback,
  WAL/index size and churn at 10k/100k/1m chunks with a skewed owner/deal distribution. These are
  proposed evaluation sizes/gates, not measurements or performance promises. Freeze latency budgets
  from baseline hardware/workload before tuning; no unbounded retries to meet recall.

From `backend/`: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
`cargo clippy --locked --all-targets -- -D warnings`, then full
`QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets`.
Use disposable schemas with the real vector extension and fake providers; never the development DB.
Run frontend typecheck/boundaries/tests/build and matching bundle gates if adapter/behavior contracts
change; native fmt/Clippy/tests if the relay changes. Commands must match manifests at implementation.

Update `docs/ARCHITECTURE.md`, storage/recovery, hybrid search, document identity/deletion guidance,
`AGENTS.md` and applicable skills with actual semantics. Review architecture, check placement,
security, cancellation, performance and scoped refactoring before handoff. Record skipped evidence.
Architecture impact of this plan: none — it describes future changes; no current runtime changed.
