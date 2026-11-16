# Hybrid document search

Updated 2026-11-15. Documents use PostgreSQL schema 3, exact pgvector cosine and GIN lexical
retrieval. Hybrid remains opt-in (`QUARRY_HYBRID_SEARCH_ENABLED=false` by default).
Notebook/template Helix infrastructure remains required. See [document storage](document-storage.md)
for profiles, durable jobs, atomic readiness and the explicitly fresh MVP cutover.

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

Bootstrap injects document repositories, optional OpenAI query embeddings and typed limits.
Embedding work finishes outside SQL. Bounded deal pipelines then run keyword and semantic
queries sequentially on one retained repeatable-read, read-only transaction per deal. Authorized
membership, live/current/readiness and profile predicates precede candidate LIMIT. Both branches
must succeed. Full canonical chunk-ID lists and remote header-verification scans are removed.

Defaults remain four deal pipelines, eight SQL read operations across the service (including
legacy endpoints), four provider requests, 16 active hybrid requests and 32 queued requests.
`QUARRY_SEARCH_SQL_READS` replaces the old Helix-read setting. Queue, embedding and operation
deadlines remain two, fifteen and thirty seconds. These are bounds, not measured latency targets.
Dropping a request drops scheduling and its SQL/provider waiters; no result/permission cache exists.

Cosine SQL distance is `1-cosineSimilarity`. Legacy flattened vector DTOs divide it by two,
retaining the existing 0/0.5/1 aligned/orthogonal/opposite contract. Hybrid's relevance threshold
is therefore similarity `1-pgvectorDistance`, equivalently `1-2*legacyDistance` after DTO conversion.
The configured floor applies before fusion; weak neighbors cannot fill an unrelated query.
Exact keyword hits remain eligible. Nonfinite/out-of-range distances are rejected.

English body parsing supports stemming and quoted natural-language phrases. A separate simple
technical-token channel distinguishes C++, C#, .NET and identifiers with bounded opaque lexemes.
Natural and technical requirements combine with AND. A single natural token ending in `*` uses
a validated bound prefix. Stopword-only and punctuation-only lexical requests return no hits.
Native rank intentionally differs from the historical Rust occurrence ranker; fixtures cover
these changes, while representative judged BM25 comparison remains a release gate.

Per-deal RRF remains `sum(1/(60+one-based source rank))`, deduplicated before ranking, using
`min(100,max(50,limit))` candidates per source. Global ordering retains fused score, best source
rank and full immutable identity ties. A final bounded current-result/member snapshot checks at
most 100 returned IDs; revocation can shorten the response without refill. Legacy keyword scores
remain reciprocal per-deal ranks. Native SQL rank is never added directly to cosine.

## Query embeddings

The private OpenAI path uses the same server-selected model/dimension profile as ingestion.
The repository selects matching immutable provider/model/dimension/metric/preparation identity;
equal dimensions do not infer a model. Provider vectors validate finite f64 and f32 values,
nonzero norms and profile dimensions. Stored values remain full f32 vectors without padding or
truncation. Model/dimension changes require an explicit compatible schema/profile operation.
Provider error bodies are never returned. The historical `HELIX_VECTOR_DIMENSION` name remains
the shared document/notebook dimension setting during the staged rollout.

Each hybrid search generates its own query embedding. Vectors are reused only across the
accessible deals within that request, then released. There is no embedding cache, retained query
key, eviction policy or sharing of in-flight provider work across requests. Existing provider
concurrency and deadline limits include waiting for a provider slot. Dropping a request drops
its provider future or queue wait and releases its slot. Authorization is repeated for every search.

## Shared UI and rollback

The Data Room panel uses hybrid search through both runtime adapters. Draft text does not
send requests. Enter or Search submits; editing, clearing, closing or changing actor/deal
invalidates prior results. Pending browser requests are aborted; the desktop JSON relay has
no request cancellation, so late completions are ignored and server work is bounded.
Focus revalidation repeats the authorized retrieval with a new query embedding. Legacy live
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

## Remaining measurement gates

Exact scoped cosine is enabled; ANN is not configured. Native English/simple GIN lexical
retrieval is the sole runtime lexical engine. Representative NDCG/MRR, filtered p50/p95,
common-term cost, WAL/index bytes, churn and 10k/100k/1m load comparisons remain to be collected.
The selected server has pgvector but no pg_textsearch/pg_search installation, so their comparative
BM25 evaluation has not run. No exact-recall/latency promise is inferred from source tests.
Hybrid remains disabled by default pending judged relevance and load acceptance.

## Verification

Real isolated PostgreSQL tests cover complete-set rollback, snapshots/cancellation, current and
foreign scopes, profiles, cosine normalization, English/technical/phrase search, RRF, final
revocation and explicit fallback/failure policy. They preserve web/desktop wire contracts.
Run the backend fmt/check/Clippy/full-test gates in [verification](ARCHITECTURE.md#14-verification-architecture).
No frontend implementation or transport changes are required for this storage replacement.

## Historical verification records

The following 2026-11-06 Helix measurements describe the retired implementation. They do not
measure current PostgreSQL relevance, latency, index readiness or operational behavior.

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

### User-selected cutoff update — 2026-11-16

The user subsequently requested a 0.25 cutoff. The ignored local configuration now sets
`QUARRY_SEARCH_MIN_SEMANTIC_SIMILARITY=0.25`; the repository default remains 0.4.
This supersedes the local 0.26 selection above and takes effect after backend restart.
The recorded relevance measurements remain historical; no new relevance evaluation was run.
