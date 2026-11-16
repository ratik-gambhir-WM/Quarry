# Document PostgreSQL storage

Documents use PostgreSQL schema 3 for original bytes, immutable identities, chunks,
full-precision pgvector embeddings, lexical indexes and recoverable processing jobs.
The document runtime has no Helix reader, writer, index initialization or deletion call.
Notebooks and templates retain their existing Helix integrations and normal startup dependencies.

## Ownership

`documents/store/repository.rs` owns canonical file aggregates and exact-upload reconciliation.
`search_repository.rs` owns prepared chunk batches, embedding profiles and scoped retrieval.
`identity_repository.rs` persists immutable identity models from `identity.rs`.
`access_repository.rs` owns bounded final-result SQL; the identity policy facade delegates to it.
`documents/ingestion/repository.rs` owns accepted input, queue capacity, attempts, checkpoints
and terminal results. Feature SQL is composed in these repositories with the shared typed
`SqlBuilder`. The PostgreSQL adapter owns codecs, builders, pooling and transaction execution.

The explicitly provisioned `vector` extension is version 0.8.6 in `quarry_extensions`.
The official Rust `pgvector` crate is pinned at 0.4.2 with SQLx 0.8.6. Types and cosine
operators are qualified; extension objects are outside isolated product/test schemas.
Startup validates extension/version/type/operator availability and owned index readiness.
Product catalog fingerprints exclude the separate extension schema. See
[provisioning and reset](postgresql-storage.md).

## Identity and atomic publication

Admission stores immutable bytes/name/hash and chooses a winning logical file through the
unique `(deal_id, input_hash)` claim in `document_input_identities`. Per-actor matching input
replays the same job, including after a lost response. Ordinary changed bytes create a new
logical file; filename matching never creates a public replacement operation. Version and
chunk identities remain deterministic after selection of the winning file ID.

Parsing, vision, embedding and CPU-heavy batch preparation run before canonical SQL locks.
Image interpretation is checkpointed durably before embedding, without vectors, so a failed
embedding attempt retries accepted text. Provider calls can repeat after an uncertain result
that was never durably checkpointed; an expired lease does not cancel an already issued call.

One retained mutation transaction writes file/version/blob, manifest and chunk identities,
all text/vector batches, lexical metadata, current-content selection, upload activity and the
job's committed result. Deferred constraints validate before the final database-clock attempt
mutation. Each chunk INSERT binds up to 128 rows in one statement; all statements
use this transaction. No chunk conflict is silently skipped. Exact replays verify immutable
identity and preserve existing chunks/activity. Every vector is checked for profile dimensions,
finiteness and nonzero f64/f32 norms before/after conversion. The pinned cosine implementation
accumulates squared norms in f32; numeric overflow/underflow is rejected in handlers, codecs
and a database self-distance check. Profiles pin provider, model,
dimensions, cosine metric and preparation version; matching dimensions alone are insufficient.

Batches allow at most 10,000 contiguous one-based chunks, 128 KiB of text and normalized technical
text per chunk, and 128 MiB of accumulated text/technical-text/section/full-vector payload. Sections are bounded
to 8 KiB. Original transport bounds
remain 50 MiB. Stored vectors use `vector(D)` with D rendered from server configuration into the
checksummed schema. The historical `HELIX_VECTOR_DIMENSION` setting still supplies the common
document/notebook dimension during their staged migration. Changing D requires a compatible
fresh schema; changing models does not silently reinterpret existing profiles.

The deferred manifest constraint and startup integrity view check exact counts, contiguous
ordinals, generations, text hashes, profile consistency and vector dimensions. Search joins
only current, live, ready sets. Readers see the prior complete version until commit. A failing
batch, deferred constraint, lost attempt, revoked access, deletion or cancellation rolls back
new canonical rows, activity and current selection. `sqliteCommitted` remains the historical
wire field for canonical PostgreSQL commit; input admission never sets it true.

## Jobs and recovery

Document jobs retain the existing 202 response and SSE event names `processing`, `completed`,
`skipped`, `failed`. SSE is a watch of polled authoritative SQL snapshots, not a process-local
job registry. Any API process with the current writer policy can read status and re-authorize
member access. Sequences increase durably on transitions; streams stop on access loss.

The queue admits at most 64 active jobs globally, 8 per actor, 4,096 retained jobs and 1 GiB
of retained input bytes. The retained global SQL write serialization makes admission counts
atomic; no multi-writer claim is made. One background worker per service claims due jobs with
`FOR UPDATE SKIP LOCKED`, retaining no parent/slot locks while preparing. Attempts have unique
tokens and a 600-second database-clock lease; processing has a 570-second timeout. Expired
background attempts are rediscovered by polling without a local notification. Final mutation
checks the attempt token, identity, profile and database-clock expiry, under current deal/file
and membership authority. An old attempt cannot publish over its replacement.

Synchronous batches use the same admission and atomic publication repository. Their active work
is request-owned; dropping the request drops its futures. Maintenance marks expired synchronous
attempts failed, clears their attempt tokens and releases their active queue slots. Unclaimed
synchronous jobs expire after ten minutes without an update. Both transitions run under the same
mutation and row locks as claims and background promotion; live leases and background jobs remain
untouched. Failed results, accepted bytes and image checkpoints retain the existing 24-hour retry
window. An explicit matching retry reuses the job and input with a new attempt token; abandoned
work never automatically becomes background work.
An explicit asynchronous replay promotes queued or processing work to durable background execution.
Promotion preserves the current attempt token and lease; a processing job can be reclaimed only
after lease expiry. Status polling selects metadata and terminal results without loading accepted
input bytes or image checkpoints.
A completed synchronous replay reports a skipped document without repeating provider work.

Terminal successes/skips retain the configured `QUARRY_COMPLETED_JOB_RETENTION_SECONDS`
(default ten minutes); failed inputs/checkpoints retain 24 hours. Pruning runs every minute.
Exact-content canonical reconciliation remains available after terminal job retention expires.
A process death after commit can replay the durable terminal result without rerunning providers.
After a death before durable provider capture, retry may repeat provider cost.

## Search and deletion

Search uses exact scoped SQL cosine and GIN-indexed lexical candidates. Both hybrid branches
execute on one retained repeatable-read, read-only transaction per deal after query embedding.
Membership, deletion state, current generation and selected profile precede candidate limits.
There is no global nearest-neighbor result followed by authorization filtering. Final bounded
result authorization remains immediately before delivery. Hybrid requires both branches; the
client's keyword fallback remains an explicit separate operation.

English body terms support stemming; a separate simple technical channel maps exact technical
tokens to bounded opaque lexemes, distinguishing C++, C#, .NET and identifiers. Both channels
use AND semantics. Whole quoted natural-language phrases use safe phrase parsing. A single
natural-language token ending in `*` uses a validated, bound prefix expression. Punctuation-only
and English stopword-only requests return no lexical hits. This deliberately changes the former
whole-token Rust ranker; fixture coverage is not a claim of measured corpus relevance.

Legacy vector responses keep flattened snake_case chunks and `(1-cosine)/2` distances.
Hybrid's floor therefore corresponds to similarity `1 - pgvectorDistance`; RRF remains per deal,
constant 60, deduplicated sources, candidate window `min(100,max(50,limit))` and deterministic
immutable identity ties. Legacy keyword scores remain reciprocal per-deal ranks. See
[hybrid search](hybrid-document-search.md) for client behavior and request/provider concurrency bounds.

Deletion cascades SQL chunks, manifests, input claims and jobs while retaining the deal lifecycle
reservation and receipt. Historical phase strings remain accepted; new cleanup passes through
`sqlite` to `verifying` and `completed`, with no remote document fence/purge. Existing source bytes
still supply stored viewing/conversion; snippets never reconstruct originals. Old Helix document
nodes are ignored and are not claimed erased by a SQL receipt or reset. Schema-2 receipts are not
imported in this explicitly fresh MVP cutover.

## Document Helix cleanup inventory

The prior `DocumentGraphReader`, `DocumentGraphWriter`, `DocumentGraphSearch`,
`PreparedDocumentGraph`, graph-only file/version/publication responses and the entire document
`graph/` module were removed. The fourteen document index specifications and operation families
`helix.document_indexes.*`, `helix.file_version.*`, `helix.file_chunk.*`, `helix.deletion.*` have
zero live source callers. Stable flattened chunk DTOs moved to `store/chunk.rs`. Canonical file
aggregate tests remain under `store/repository_tests.rs`; obsolete graph serialization/live-runtime
tests were replaced by SQL rollback, read/search, lease, identity and deletion behavior tests.
An architecture guard rejects document Helix dependencies and feature SQL in the shared adapter.

Bootstrap no longer initializes document indexes. Search/full-catalog graph scans and remote
deal document fences/purges are gone. Notebook/template startup, SDK/config, Docker/launcher
setup and retained generic `embedding`, `helix_query`, `clear_helix` utilities remain. Those
utilities were inspected, not executed; none is a live document storage caller. A reset or SQL
deal receipt does not certify old remote document erasure.

## Verification and limits

Real SQL tests cover batch/commit failures, old/new snapshots, cancellation, exact retries,
profile/ordinal/vector rejection, accepted image text, attempt replacement, archive/deletion,
queue bounds, current authorization, technical terms/phrases and unchanged search DTOs/fusion.
There are no live provider calls in these tests. The full backend gate also retains notebook,
template, transport and authorization suites.

Exact cosine is the default; no ANN index or recall promise is enabled. The plan's comparative
BM25 evaluation and representative 100k/1m relevance/load/WAL/index-size measurements remain release
evidence to collect. The initial synthetic 10k run below is not representative relevance acceptance. This server exposes pgvector but not pg_textsearch/pg_search; those alternatives were
not installed or benchmarked. No measured latency, NDCG/MRR or scale guarantee is asserted.
Global writer ownership and serialization remain required across all domains.

### Initial synthetic 10k measurement — 2026-11-15

The manually invoked `measure_native_document_search_at_ten_thousand_chunks` test uses fake
1,536-dimensional vectors, one authorized file/deal and ten reads per case. It is not a skewed
multi-owner relevance corpus. A recorded maintained-index run, after VACUUM/ANALYZE of the owned
fixture table, produced these observed values (milliseconds):

| Operation | p50 | p95 |
| --- | ---: | ---: |
| Selective lexical hit | 2.842 | 25.532 |
| Common-term lexical top 20 | 41.730 | 42.618 |
| Lexical no hit | 1.878 | 2.359 |
| Exact vector top 20 | 50.990 | 58.970 |

Ingest took 3.537 seconds; GIN occupied 1,998,848 bytes. EXPLAIN ANALYZE confirmed a bitmap GIN
scan for the selective term. Immediately after bulk ingestion with the GIN pending list intact,
the planner instead chose canonical identity/chunk-primary-key probes: a preceding run measured
selective p50 9.796 ms and common-term p50 48.891 ms. This is a maintenance/planner observation,
not a reason to force an index or run VACUUM in product requests. Autovacuum/GIN pending-list cost,
filtered scopes, representative ranking, write/rollback/WAL/churn/restore, other dimensions and
larger corpora still require operational measurement. No ANN implementation or recall gate is claimed.

### Implementation verification — 2026-11-15

Commands ran from their respective build roots. Database tests used isolated schemas in
`Quarry_test`, shared extension objects and fake providers; they did not reset `Quarry`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --locked --all-targets` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets` | 522 passed, 9 ignored |
| `QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --lib measure_native_document_search -- --ignored --nocapture` | Passed; measurements above |
| `npm run typecheck` | Web and desktop passed |
| `npm run check:boundaries` | Passed |
| `npm test` | 99 files, 652 tests passed |
| `npm run build:web` | Passed |
| `npm run check:web-bundle` | Passed after web build |
| `npm run check:bundle-size:web` | Passed after web build |
| `npm run check:notebook-bundle:web` | Passed after web build |
| `npm run build:desktop-ui` | Passed |
| `npm run check:bundle-size:desktop` | Passed after desktop build |
| `npm run check:notebook-bundle:desktop` | Passed after desktop build |
| `git diff --check` | Passed; untracked text checked separately |

An earlier full run exposed a deletion HTTP test timing assumption: SQL cleanup can complete
before a subsequent archive request, yielding 404 rather than pending-deletion 409. The assertion
now accepts both valid rejection states; retained lifecycle tests cover the pending drain guard.
A sandboxed database test invocation was denied local socket access; the authorized isolated
database run above passed. No live provider, Chromium or notebook/template Helix integration was
invoked. Eight retained interactive/external-integration tests remain ignored; the ninth ignored
test is the synthetic measurement explicitly run above. Native Tauri Rust and manual UI inspection
were outside this backend-only source change. Vite emitted its vendor chunk warning; all repository
bundle budgets passed. The skill validator lacked PyYAML; Ruby YAML validated the edited skill
frontmatter instead.

Review covered atomic rollback, cancellation, identity/profile compatibility, database-clock
attempt fencing, authorization before limits, final authorization, bounded memory/queue/query
work and preservation of notebook/template behavior. Feature SQL placement and repository naming
were corrected during review. No unresolved correctness finding or additional scoped refactor
was identified. UI accessibility/interaction and visual consistency were not applicable to this
backend change. Architecture, storage, hybrid search, deletion and project guidance were updated.

The initial local MVP reset refused an active API writer without changing product tables. After
explicit approval on 2026-11-15, the verified Quarry API listener was stopped gracefully with
SIGINT and the reset completed using, from `backend/`:

```sh
QUARRY_DATABASE_URL=postgresql://localhost/Quarry cargo run --locked --bin reset_postgres -- --confirm-reset
```

Read-only SQL checks confirmed schema version 3, 46 product tables, configured
`quarry_extensions.vector(3072)` embeddings, vector extension 0.8.6 and zero invalid/unready
indexes. Users, deals, files, chunks and document jobs were empty. The extension prerequisite
remains provisioned in `Quarry` and `Quarry_test`. Port 3001 was verified without a listener;
the API was left stopped as requested. No old Helix/SQLite store was cleared, and no server
was started for validation.
