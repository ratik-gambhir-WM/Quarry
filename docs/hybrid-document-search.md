# Hybrid document search

Current storage is fresh PostgreSQL schema 2 with retained Helix. See
[PostgreSQL storage and Helix](postgresql-storage.md) for setup, tests and recovery; former
SQLite marker upgrades are historical and are not executed by this runtime.

Updated 2026-11-06. The initial bounded-scan implementation is available behind
`QUARRY_HYBRID_SEARCH_ENABLED=false` (default). It requires no additional change to PostgreSQL schema 2,
Helix publication, stored embeddings, or the model used for ingestion.

## Request and result contract

`POST /api/v1/deals/{deal_id}/documents/search/hybrid` and
`POST /api/v1/documents/search/hybrid` accept only `{queryText, limit}`. The handler
trims text once, checks 1–2,000 UTF-8 bytes and limits of 1–100, rejects unknown fields,
and validates the path. Bearer membership determines scope. Unknown/inaccessible deals
retain the existing authorization errors. The global operation derives at most 200 deals;
an empty enabled scope returns `{results: []}` without a provider request.

The camelCase response is `{results: [{chunkId, dealId, fileId, versionId, text,
chunkIndex, pageStart, pageEnd, sectionPath, matchSources}]}`. Sources are `keyword` and/or
`semantic`; scores, vectors, query echoes and provider configuration are private. Legacy
keyword/vector endpoints and their snake_case result DTOs remain available.

## Ownership and scheduling

`DocumentSearchService` shares canonical preparation/freshness helpers with the legacy
operations. Bootstrap injects optional OpenAI query embeddings and typed limits from
`AppConfig`. Services never read environment variables. Each hybrid request starts one
embedding/cache lookup concurrently with bounded deal preparation. A prepared deal starts
keyword retrieval immediately; its vector retrieval awaits the same shared embedding.
Preparation verifies the entire canonical projection once per deal. Both branches must succeed.

Defaults are four concurrent deal pipelines per request, eight search Helix reads across the
service (including legacy endpoints), four provider misses, 16 active hybrid requests and
32 queued requests. Queue wait is two seconds, embedding work 15 seconds, and the hybrid
operation 30 seconds. These are trial limits, not measured performance targets. Existing
client timeouts/retry policy remain unchanged. Admission is nonblocking when full. Request
futures own all orchestration; error, disconnect/drop or timeout stops further scheduling.
Already accepted remote reads and blocking lexical ranking can finish after cancellation.

Vector retrieval preserves Helix-normalized cosine distance until the service applies a relevance floor:
`QUARRY_SEARCH_MIN_SEMANTIC_SIMILARITY` defaults to 0.4 (finite 0–1); the historical large/3072
local trial used the measured 0.26 override described below. The floor admits distances
in `0..=(1 - minimumSimilarity) / 2`. The pinned server normalizes cosine distance:
`distance = (1 - cosineSimilarity) / 2`, so similarity is `1 - 2 × distance`. Identical vectors
return 0, orthogonal vectors 0.5, and opposite vectors 1. The old `1 - distance` interpretation
incorrectly admitted unrelated candidates. Nonfinite/out-of-range distances are rejected. This prevents
nearest-neighbor retrieval from automatically filling no-match queries with weak candidates.
It is a tunable initial guardrail, not a confidence percentage or a calibrated relevance guarantee;
measure it against the configured embedding model and judged queries. Exact keyword hits are
retained even if their semantic distance fails the floor, with only the Keyword source badge.
Strong semantic-only paraphrases remain eligible. Legacy vector APIs retain their existing contract.

Both lists are filtered against a refreshed full immutable identity before per-deal RRF:
`sum(1 / (60 + one-based source rank))`. Each source is deduplicated before assigning ranks.
The source window is `min(100, max(50, limit))`. Combined deals are sorted by fused score,
best source rank and stable deal/file/version/chunk/generation/hash/index identity. A final bounded
canonical cross-deal membership/current-result snapshot checks the top 100 or fewer IDs;
revocation can reduce the response without refill. No results or permissions are cached.

## Embedding reuse

The private OpenAI path uses the ingestion model and the same `HELIX_VECTOR_DIMENSION`
injected into index initialization and document graph writes. The OpenAI adapter validates every
response against its requested dimension and finite f64/f32 representation; the graph writer
independently rejects dimension mismatch before the canonical PostgreSQL commit. Stored vectors are
copied into typed f32 array properties without truncation/padding. Configuration tests cover both
the default 1536 and an alternate dimension; disposable Helix tests read synthetic embeddings back
and verify exact values/dimensions before querying them. It rejects multiple/empty vectors, dimensional mismatch, nonfinite f64 values
and overflowing f32 conversion. Provider error bodies are not exposed. Historical embedding
model compatibility still depends on operating the index with its ingestion model; changing
that model requires a reviewed reindex and remains outside this feature.

Each process-local cache is immutable and bound to one provider client/model/dimension tuple.
The key is SHA-256 of a private random process-cache salt, provider/model/dimension/version
namespace, and the exact trimmed query; punctuation and case are preserved. Retained keys
contain no plaintext query. Successful vectors expire five minutes after insertion, never
extended on reads. Oldest-expiry eviction enforces 512 entries and 16 MiB of retained
vector/key/entry data with a conservative 256-byte per-entry allowance and an additional 128 KiB table-capacity reserve.

A weak registry coalesces up to 32 unique in-flight keys, at most 64 live waiters per key.
Shared futures are polled by their callers, without detached tasks or locks across awaits.
Dropping the last waiter cancels provider/queue work; another live waiter can keep shared
work running until its original fixed deadline. Errors and cancelled flights are not cached.
A new configuration/cache instance has a new salt and cannot reuse the old vectors.
Authorization is repeated for every search, including cache hits and different actors.

## Shared UI and rollback

The Data Room panel uses hybrid search through both runtime adapters. Draft text does not
send requests. Enter or Search submits; editing, clearing, closing or changing actor/deal
invalidates prior results. Pending browser requests are aborted; the desktop JSON relay has
no request cancellation, so late completions are ignored and server work is bounded.
Focus revalidation repeats the authorized retrieval using the embedding cache. Legacy live
keyword consumers retain their 250 ms debounce. Query, passage and response bodies are
omitted from the new HTTP activity-log entries; desktop API logs contain operation metadata.

The panel shows loading, empty, retryable error and explicit unavailable messages. A 503
exposes a **Use keyword search** action; fallback is an explicit user choice and updates the
source label. Source badges describe actual branches. Only literal whole-token matches are
highlighted with escaped React text. Page activation still requires the open stored-file ID
and a valid reported page; cross-document activation remains deferred.

All results, Keyword, and Semantic filter buttons split the returned hybrid passages by their
actual `matchSources`, with overlapping source counts for passages matched by both methods.
All results deduplicates those passages. Switching views preserves server order and performs
no new search/provider request; counts describe the returned window, not the entire corpus.
New submissions reset the filter, and keyword-only fallback hides these hybrid source controls.
The preview spells out Keyword, Semantic, or Keyword + Semantic match. Empty source views
distinguish absent source matches from an entirely empty search and offer Show all results,
returning focus to the query input. Other search consumers retain their existing presentation.

To roll back, disable `QUARRY_HYBRID_SEARCH_ENABLED`; the panel reports unavailability and
retains explicit keyword fallback. No data rollback or destructive reset is needed.

## Indexed capability decision and remaining release gates

The pinned `helix-db 3.0.0` re-exports `helix-ast 0.1.0`. Its text-search builders
(`text_search_nodes[_with]`) are root source operations accepting label/property/query/K and
an optional tenant. They do not accept the canonical current identity stream before top-K.
The maintained authorized lexical path therefore remains the bounded all-token scan.
Root BM25 followed by filtering or arbitrary overfetch is not an acceptable substitute.
No SDK upgrade, second lexical store, schema migration or ceiling increase is introduced.

This source inspection is not a client/server capability spike or performance proof. The
indexed milestone still requires an explicitly selected disposable Helix instance, eligible
matches below historical/deleted/unpublished distractors, analyzer parity (including C++/C#),
query-plan/index-readiness evidence and bounded text transfer. If unavailable, separately
review a narrow upstream extension or a separately designed PostgreSQL document lexical index with durability/backfill/recovery guarantees.
Template lexical parity already uses PostgreSQL; it does not provide a document lexical index.
The 5,000-current-document, 10,000-canonical-chunk and 200-deal ceilings remain explicit.
Large-deal indexed preparation/publication verification is a separate design milestone.

The feature stays disabled by default pending disposable baseline/load and judged relevance
review. Compare legacy sequential scheduling (a configured deal concurrency of one), overlap,
and cold/warm cache with identical candidate windows; isolate any future indexed improvement.
Measure p50/p95/p99, throughput, provider calls, transferred text bytes and memory for
1/10/200 deals and corpus sizes through and beyond the ceilings. Evaluate exact identifiers,
whole-token punctuation, paraphrases, duplicates across deals and mixed document formats with
Recall@K, MRR/nDCG and excerpt usefulness. No live provider/load measurements are claimed.
Set release targets and tune the trial queue/deadlines before enablement.

Structured phase logs record scope, queue waits, candidate snapshots, header verification,
embedding hit/miss duration, keyword transfer/ranking, vector reads, freshness, fusion,
final access and total duration, with counts/bytes/success only. They exclude query text,
passage text, vectors, keys, email and provider bodies.

## Verification

Deterministic tests under `backend/tests/unit/domains/documents/search/` cover overlap,
single embedding across ten deals, preparation bounds, drop/timeout/overload, stale generation
and final access filtering, cross-actor cache reuse, cache coalescing/cancellation/eviction,
DTO validation and fusion ties. The existing ignored shared-search compatibility suite also
exercises hybrid via a synthetic embedding provider; run it only against an explicitly
selected disposable Helix instance under the established compatibility procedure.

Frontend tests cover HTTP/Tauri mapping, explicit submission, stale responses, actor/deal
reset, source labels, literal highlights, unavailable fallback and page activation. Native
relay code/capabilities are unchanged: its existing validated `/api/v1/` JSON transport carries
the additive routes. Use the existing frontend and backend gates from the architecture.

## Historical verification records

The dated November 6 records below describe the former SQLite deployment and its local model
trial. They do not describe the current PostgreSQL database or authorize repeating any cleanup.
Use [PostgreSQL storage and Helix](postgresql-storage.md) for the current deployment and recovery.

## Implementation verification — 2026-11-06

| Build root | Command | Observed outcome |
| --- | --- | --- |
| `backend/` | `cargo test --locked domains::documents::search --quiet` | 21 focused tests passed |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `backend/` | `cargo test --locked --all-targets --quiet -- --test-threads=4` | 502 passed, ten intentionally ignored |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test -- --silent` | 580 tests in 92 files passed |
| `frontend/` | `npm run build:web` | Passed; Vite emitted its large-chunk advisory |
| `frontend/` | `npm run check:web-bundle` | Passed immediately after the web build |
| `frontend/` | `npm run build:desktop-ui` | Passed; same large-chunk advisory |
| repository root | `git diff --check` | Passed |

The first restricted backend run could not bind local mock-server ports; rerunning with
loopback permission resolved those transport failures. A later unrestricted parallel run
failed two unchanged QuickLook tests (`quicklook_accepts_uppercase_png_extension_and_honors_byte_limit`
and `quicklook_enforces_independent_stdout_and_stderr_buffer_limits`). Both had passed in an
earlier full run and passed again with four test threads. No unrelated preview code was changed.

Manual inspection used a temporary synthetic browser harness, without server/database/provider
requests. Observed explicit-submit idle/loading/results, literal-only highlights, semantic badges,
unavailable error and explicit keyword fallback/empty state, keyboard selection, disabled
cross-document activation, successful valid-page activation, Escape/focus restoration, light/dark
surfaces and a 360 px viewport. The browser console had no warnings/errors. The task-owned preview
server and temporary harness were removed after inspection.

Native Tauri checks and a packaged desktop runtime inspection were not run because native relay,
commands and capabilities were unchanged; desktop adapter tests, typecheck and UI build passed.
Live provider calls, quality/load benchmarks
and indexed/large-deal release gates remain unexecuted. The document compatibility tests were
subsequently run on a separate disposable instance as described below. No local product database, environment
file or live graph was changed.

Architecture impact: documented additive routes/DTOs, scheduling/cache/configuration, private
logging, submitted UI/source/fallback states and the retained indexed/capacity limitations in
`ARCHITECTURE.md`. Review verdict: approve the initial disabled rollout; no unresolved required
code findings or newly unreachable code identified. The live rollout gates above remain open.


## Helix query diagnosis — 2026-11-06

A separate, memory-only container using the repository-pinned Helix image reproduced the issue
without reading or changing the live product graph or calling a provider. The production
`current_vector_candidates` query correctly nests canonical deal/chunk-ID predicates inside
`vector_search_nodes_within`, binds the supplied embedding, and projects `$distance` directly.
The error was the interpretation of that returned distance in the service.

Known-vector query results proved the server's normalized scale: aligned/scaled vectors 0,
oblique cosine similarity 0.6 gives distance 0.2, orthogonal 0.5, opposite 1. A constrained
candidate test excludes closer distractors before top-K. The default 0.4 similarity floor now
requires distance at most 0.3, rather than the erroneous 0.6. The configured floor is unchanged.
Legacy vector response distances remain raw for compatibility; only the hybrid policy conversion
changes. Query-structure, metric-conversion and 30-candidate no-match regressions cover this path.


The dimension check also found a missing provider-response validation in document ingestion.
OpenAI responses now have to match the requested dimension and finite f32 storage range;
`DocumentGraphWriter` requires the expected dimension at construction and uses it for both index
initialization and preflight. A mismatch is rejected before any SQLite document row is committed.
The test-only mock provider now explicitly requests and returns the same two dimensions.
Cosine remains the metric, following [official OpenAI embedding guidance](https://developers.openai.com/api/docs/guides/embeddings#which-distance-function-should-i-use);
OpenAI embeddings are unit-normalized, while the Helix regression additionally verifies that scaling
a stored vector does not alter cosine ranking. Existing production data is not rewritten.


Verification after the Helix-scale and dimension fixes:

- `cargo fmt --all -- --check`, `cargo check --locked --all-targets` and
  `cargo clippy --locked --all-targets -- -D warnings` passed.
- `cargo test --locked --all-targets --quiet -- --test-threads=4`: 502 passed,
  ten opt-in tests ignored by the normal gate.
- `QUARRY_HELIX_COMPAT_URL=http://127.0.0.1:16979 cargo test --locked v3_ -- --ignored --test-threads=1`:
  all three document compatibility tests passed on the separate pinned-image container,
  including saved-vector value/dimension readback, normalized metric behavior and candidate
  filtering before top-K. No live provider call was used.
- Focused provider-response and pre-persistence dimension-rejection tests passed.
- The temporary container was stopped/removed and its port was verified free. The original
  development Helix container remained running. No frontend/native source changed in this
  follow-up; the earlier frontend build/test evidence remains the applicable baseline.

The running development backend needs a restart to load the corrected conversion and dimension
checks. The live document's scores were not inspected or recalibrated; the demonstrated defect
is the normalized distance interpretation, and the similarity floor remains 0.4.

`cargo build --locked --bin quarry-backend` also passed at that stage.

## Selected local 3072-dimensional trial (2026-11-06)

At the user's explicit request, the selected local profile was drained and its SQLite database,
Helix store, and configuration were backed up together under ignored local maintenance storage.
The user then authorized removal of all uploaded Data Room documents from SQLite and Helix.
The cleanup removed the SQLite file/version/blob/current-content/identity/recovery records and
all Helix document nodes, including an orphan projection. All seven document tables and all
three document graph labels were verified empty. Account, deal, membership, template, notebook,
and note counts were unchanged; the 14 unrelated Helix nodes were retained. A disposable copy
first verified the SQLite cascade and preservation behavior, followed by a foreign-key check.

The empty document and notebook vector indexes were explicitly rebuilt at 3072 dimensions.
Notebook projections were already empty; canonical notebook content was preserved. The ignored
local configuration now selects `OPENAI_EMBEDDING_MODEL=text-embedding-3-large` and
`HELIX_VECTOR_DIMENSION=3072`. Template vector dimensions and repository defaults remain unchanged.
New document ingestion and query embeddings use this same model/dimension pair. Configuration
now rejects dimensions exceeding the known small/large model maxima (1536/3072).

A small live provider check returned finite 3072-dimensional vectors. A synthetic vector was
written to Helix, read back with all 3072 values unchanged, and queried through the actual
within-candidate vector path; the returned normalized distance agreed with local cosine
calculation. The synthetic document was removed and both document stores were checked empty
again. The temporary cleanup source, executable, and scratch SQLite copy were removed afterward;
no reset endpoint or startup deletion path was added. Matching backups remain private and ignored.

This is a model trial, not a relevance guarantee. A synthetic Java/Angular passage scored about
0.2371 cosine similarity for `programming language`, below the unchanged 0.4 cutoff. The trial
therefore does not establish that higher dimensions alone resolve conceptual recall. Actual
documents need to be uploaded again and judged after this reset. The proposed broader-match
UI/request/configuration option was removed at the user's request; search remains embedding-only
with the original keyword branch and no relevance-model call.

Rollback requires another drained window and restoration of the matching SQLite, Helix, and
configuration backups together. Preserve any newly uploaded content before restoring the old
snapshot; do not mix the old 1536-dimensional graph with 3072-dimensional query configuration.

Post-cutover verification:

- `cargo test --locked app::config::tests --quiet`: 15 passed.
- `cargo test --locked --all-targets --quiet -- --test-threads=4`: 503 passed, ten opt-in
  tests ignored; no live canonical stores are used by this suite.
- `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, and
  `cargo clippy --locked --all-targets -- -D warnings`: passed.
- After removing the proposed broader-match option, `npm test --
  tests/components/data-room/document-search/DataRoomDocumentSearch.test.tsx
  tests/api/dealMembershipApi.test.ts`: 25 passed; `npm run typecheck` passed for both targets.
- Live maintenance checks: matching backups, disposable SQLite cascade/preservation check,
  foreign-key check, zero document rows/projections, unchanged unrelated counts, and a real
  3072-dimensional provider/Helix save/read/search round trip passed. Probe records were removed.
- The standalone embedding demonstration now uses the configured model, avoiding a hardcoded
  small-model request with the selected profile's 3072-dimensional setting. It was not executed
  because it prints full vector values.
- `cargo build --locked --bin quarry-backend` passed. Quarry was restarted with the selected
  local configuration; API health and the web UI both returned HTTP 200. This restores the user's
  application session rather than leaving a temporary test server running.
- `git diff --check` passed; temporary cleanup source/artifacts are absent and matching backups
  are ignored local files. Architecture configuration/recovery notes were re-checked and updated.

## Source-filter verification (2026-11-06)

- Focused Data Room and generic search component tests: 28 passed.
- `npm test`: 582 passed across 92 files.
- `npm run typecheck`, `npm run check:boundaries`, `npm run build:web`,
  `npm run check:web-bundle`, and `npm run build:desktop-ui`: passed. The web bundle check ran
  after the web build. Both builds retain the existing JavaScript bundle-size advisory.
- Synthetic browser checks covered source counts/overlap, filtered selection/preview,
  keyboard activation, loading, empty source views, explicit unavailable keyword fallback,
  light/dark themes, and the 360-pixel viewport. No console errors/warnings were observed.
- The temporary preview HTML/fixture and the dedicated Vite server were removed; port 1421
  was verified free. The user's application and document data were not used for these checks.
- Review found no blocking issues or newly unreachable code. The API, native transport,
  embedding model/dimensions, semantic cutoff, and chunking behavior did not change in this
  UI follow-up, so backend/native checks were not repeated. Architecture notes were updated.

The current DOCX parser flattens canonical document text and applies contiguous ranges capped
at 800 embedding tokens. Cuts are UTF-8 safe but do not follow sentence/section boundaries;
there is no overlap and DOCX chunk section titles are unset. Each passage is sent as a separate
embedding input even when several are batched in one provider request. This is a potential
relevance limitation, not evidence that it caused a particular query failure. A smaller,
section-aware/overlapping experiment would require separately versioned ingestion and reindex
planning; no parser change or live re-ingestion was performed in this UI work.

## Measured local cutoff correction (2026-11-06)

The user subsequently identified additional indexed Rome and COBOL history documents, beyond
the three initially supplied test documents. A temporary read-only diagnosis confirmed six
current files and 22 canonical chunks in the selected local deal, including three current
chunks each for Rome and COBOL. It used a read-only SQLite connection, bound query builders,
the existing Helix candidate-filtered retrieval path, and a small batch of query embeddings.
No bootstrap, migration, ingestion, document deletion, or graph write was performed.

| Query | Best expected-file cosine | Effect of 0.4 | Effect of selected 0.26 |
| --- | --- | --- | --- |
| Royal dynasties | Rome: about 0.36949 | Discards every semantic hit | Retains three Rome passages |
| User's legacy-technology/core-foundation question | COBOL: about 0.39251 | Discards every semantic hit | Retains three COBOL passages |
| programming language | COBOL: about 0.28321 | Semantic hits rejected; keyword hits still eligible | Semantic hits become eligible too |
| sports | Best across deal: about 0.12835 | Empty | Empty |
| navigating between screens in a mobile app | Submersible: about 0.25390 | Empty | Empty |

Football scoring, diesel starter motors, and database restoration controls had best similarities
of approximately 0.17573, 0.14036, and 0.22503, respectively, with no keyword hits. They stay below
0.26. Bread still has exact keyword hits and a best semantic similarity of about 0.31707.
The proposed 0.25 would admit the unrelated mobile-navigation/Submersible hit; after seeing
this comparison, the user explicitly selected 0.26.

The ignored selected-profile configuration now explicitly sets
`QUARRY_SEARCH_MIN_SEMANTIC_SIMILARITY=0.26`. Repository defaults remain 0.4 because the
measurement used this specific large/3072 corpus, not every supported model/profile. The
normalized Helix conversion remains unchanged; a 0.26 floor admits distances at most 0.37.
No re-embedding, dimensional change, rechunking, or broader-match UI option accompanies this
correction. A deterministic policy regression covers the measured positives and controls.
These measurements explain the reported false negatives, but do not guarantee relevance for
unjudged queries or a changing corpus. The temporary diagnostic source/executable was removed.

Cutoff correction verification:

- `cargo test --locked domains::documents::search::hybrid --quiet`: ten passed.
- `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, and
  `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked --all-targets --quiet -- --test-threads=4`: 504 passed, ten opt-in
  tests ignored by the normal gate.
- `cargo build --locked --bin quarry-backend`: passed.
- The read-only live-index diagnostic was repeated with the selected 0.26 configuration:
  three semantic candidates each for the exact reported Rome/COBOL queries, zero for sports
  and mobile-app navigation. This checks retrieval/filter eligibility rather than claiming an
  authenticated browser/API response was observed. No index or document content changed.
- Quarry was restored in the existing web launch mode with the selected local override;
  API health and the UI both returned HTTP 200. The six uploaded documents were retained,
  and no re-upload is required for the cutoff change. Architecture notes were re-checked.
