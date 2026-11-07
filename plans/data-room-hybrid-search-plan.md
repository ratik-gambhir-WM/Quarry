# Concurrent hybrid document search implementation plan

Updated: 2026-11-06. Status: initial bounded-scan delivery implemented (shared preparation,
concurrent hybrid, query cache and shared UI). Rollout defaults to disabled pending the
Phase 0 disposable-load/relevance gates. Indexed lexical retrieval and large-deal preparation
remain gated milestones, with no schema/index/model changes. See
[the maintained implementation and release gates](../docs/hybrid-document-search.md).

This refresh replaces the older schema-9/workspace-based proposal in this file with a plan
based on the current working tree. Code and [the architecture](../docs/ARCHITECTURE.md) remain
authoritative. Existing uncommitted backend and frontend work must be preserved.

## Goal and scope

Provide one Axum operation that accepts question text, overlaps authorized keyword retrieval
with query embedding, retrieves semantic candidates when that embedding is ready, and merges
small ranked lists without a second model call. Embed once per question, reuse that vector
across accessible deals, and cache compatible query embeddings briefly on the server.

Start with uploaded, persisted, current/live document passages in the selected Data Room.
Implement the same backend orchestration for the existing all-accessible-deals scope; this is
not a new project/organization scope. SQLite remains canonical, Helix remains the search
projection, and the browser/Tauri webview calls only Axum through `@quarry/runtime`.

Keep passage results and current page-navigation behavior in the first delivery. Advanced
All/Any/None/phrase controls, filename retrieval, document-level grouping, cross-document
activation, importing local/SharePoint files, answer generation, reranking, parser/chunk changes,
and embedding-model migration are separate work. These were broader ideas in the previous
proposal, not prerequisites for this optimization. Do not silently implement them here.

## Verified pre-implementation baseline

| Area | Current source and behavior | Implication |
| --- | --- | --- |
| UI | [DataRoomDocumentSearch](../frontend/src/components/data-room/document-search/DataRoomDocumentSearch.tsx) calls `searchDealDocumentChunksByKeyword` with limit 30. [useLiveDocumentSearch](../frontend/src/components/data-room/document-search/useLiveDocumentSearch.ts) debounces 250 ms and suppresses stale completions. | Live keyword search already works without selecting a preview; replace its request only after hybrid integration is ready. |
| Routes | [search/route.rs](../backend/src/domains/documents/search/route.rs) exposes global and deal-scoped `/documents/search/keyword` and `/documents/search/vector` variants. | Add text-in hybrid routes; keep existing endpoints compatible. |
| Service | [search/service.rs](../backend/src/domains/documents/search/service.rs) visits deals sequentially. Each method selects canonical chunks, verifies projection completeness, retrieves, reloads current identities, then batch-checks final result IDs. | Share initial preparation across hybrid branches and retain both freshness and final access checks. |
| Embedding | The vector endpoint accepts a caller-provided embedding. `DocumentSearchService` does not currently create an embedding per deal or at all. | One embedding per question is a new service responsibility, not removal of an existing per-deal provider call. |
| Keyword retrieval | [graph/repository.rs](../backend/src/domains/documents/graph/repository.rs) `keyword_current` fetches candidate text then uses `spawn_blocking(rank_keywords)`. [graph/query.rs](../backend/src/domains/documents/graph/query.rs) fetches at most 10,001 candidates, with overflow failing explicitly. | Overlap reduces serialized waits; indexed lexical retrieval separately removes corpus-text transfer and ranking in Rust. |
| Lexical semantics | Every distinct case-insensitive whole token must occur in the same passage; `+` and `#` preserve C++/C#. | Preserve existing keyword behavior; semantic hits may legitimately lack literal terms and must be labeled accordingly. |
| Additional ceilings | [ResourceAccess](../backend/src/domains/identity/access.rs) limits a deal to 5,000 current documents and 10,000 canonical chunks. Global service scope is capped at 200 accessible deals. | Indexed keyword retrieval alone does not make larger deals searchable. Candidate preparation and header verification remain O(corpus). |
| Index capability | `graph/writer.rs` creates deal-partitioned chunk text/vector indexes; `graph/query.rs` has a root text-search builder. The current authorized path avoids it because the pinned SDK lacks within-stream text ranking. | A root top-K followed by filtering is not a safe substitute for ranking within eligible candidates. Prove an indexed solution first. |
| Runtime | Backend uses Rust edition 2021/MSRV 1.94, Axum 0.8, Helix SDK 3.0.0, Tokio and futures-util. Frontend lockfile has React 19 canary and Vite 7.3.6. | Follow live manifests/locks, not stale skill version examples; no dependency upgrade is assumed. |

## Stable contracts and security invariants

- Authenticate through the existing bearer actor and derive membership access server-side.
  A path deal is authorized before search; global scope derives accessible deals. Never accept
  client workspace IDs, user IDs, models, dimensions, or embeddings on the hybrid operation.
- Both branches rank only eligible current/live chunk identities. Preserve the canonical
  file/version/generation/hash checks and complete-projection failure behavior.
- Membership removal, deletion, version replacement, or generation change during retrieval
  must remove stale candidates before response. Preserve a final bounded cross-deal
  authorization snapshot immediately before exposing results.
- Keep old endpoint DTOs and matching behavior stable. Existing chunk response fields are
  snake_case despite camelCase request DTOs; the new product DTO uses camelCase explicitly.
- Preserve HTTP/Tauri encoding, error handling, restrictive relay validation, accessible panel
  behavior, focus restoration, and current valid-page navigation rules.
- Keyword-only search continues to work without OpenAI. Hybrid fails explicitly when either
  retrieval branch or embedding fails; do not present a partial keyword list as hybrid success.
  Authorization, incomplete-index, capacity, and timeout failures remain explicit.

## Target orchestration

```text
Validate input → authorize scope and obtain eligible deals
                  ├─ embed question once (or compatible cache hit) ──────────┐
                  └─ bounded per-deal preparation                           │
                       current chunks + verify canonical projection headers │
                         ├─ keyword retrieval ─────────────────────┐        │
                         └─ wait for shared embedding → vector ────┤←───────┘
                                                                  ↓
                     refresh canonical identities → fuse → final access → response
```

Start embedding and preparation concurrently after initial scope authorization. Each prepared
deal can start keyword retrieval immediately; vector retrieval waits for both that deal's
prepared scope and the single shared embedding. Publish preparation and embedding outcomes
through a small request-owned coordinator; do not call embedding from each deal future.
Preparation is computed once per deal and shared immutably between branches.

Use structured futures (`tokio::join!`/`try_join!` and bounded streams as appropriate) with
request-scoped cancellation, rather than detached tasks. Explicitly define first-error cleanup
and wake all waiters if embedding/preparation fails. Drop/timeout must stop scheduling more
work; already accepted remote work may still finish. Never hold a cache lock across an await.

For one deal, the target critical path is approximately:

`scope + max(preparation + keyword, max(preparation, embedding) + vector) + freshness + fusion + final access`.

This is more precise than claiming all shared work disappears. Multi-deal queueing, process
limits, network overhead, and the current corpus-size preparation costs remain measurable.

Start with four active deal pipelines per request, a process-wide cap of eight active search
Helix reads, and four active embedding cache misses. Treat these as trial defaults to load-test,
not measured optimal values. Also bound request admission/queue wait and total request deadline;
a semaphore alone does not bound the number of waiting requests. Inject typed limits from
`AppConfig`/bootstrap. Reuse existing client timeout policy; do not multiply retries across layers.
Choose phase deadlines and queue capacities from Phase 0 measurements before enabling the UI.

## Additive API and ownership

Add these routes under the existing v1 mount:

- `POST /api/v1/deals/{deal_id}/documents/search/hybrid`
- `POST /api/v1/documents/search/hybrid` for all accessible deals.

Request: `{ "queryText": "What are the customer renewal risks?", "limit": 30 }`.
Validate nonblank trimmed text, the current 2,000-byte query ceiling, path identifier and
`limit` in 1..100 in the handler. Reject unknown fields. The deal path is the scope authority.
Use the same trimmed string for embedding, lexical intent, and cache identity; do not lowercase
or normalize punctuation in embedding input. Empty authorized global scope returns an empty
result without an embedding request.

Proposed response shape:

```json
{
  "results": [{
    "chunkId": "chunk-id",
    "dealId": "deal-id",
    "fileId": "file-id",
    "versionId": "version-id",
    "text": "bounded matching passage",
    "chunkIndex": 1,
    "pageStart": 4,
    "pageEnd": 4,
    "sectionPath": "Renewals",
    "matchSources": ["keyword", "semantic"]
  }]
}
```

Return the existing bounded stored passage text needed by excerpts; do not imply that it is an
LLM summary. Include no query echo, vector, provider details or confidence percentage. Define
DTOs in `documents/search/` and explicitly map verified graph results. Keep internal fusion
scores out of the public contract. Use existing authorization/error mapping and sanitized
unavailable errors for absent embedding capability or branch failure.

`DocumentSearchService` owns orchestration, branch policy, request limits, and fusion. Graph
repositories own typed retrieval. `ResourceAccess` owns canonical eligibility. Bootstrap injects
the optional shared `OpenAiClient`, ingestion-compatible model/dimensions, cache, and limits;
services must not read environment variables or construct infrastructure. Use `ServiceError`,
`RepositoryError`, and `AppError` at their established layers.

## Embedding reuse and cache

Create one query-embedding collaborator used by the search service. Reuse the private/redacted
embedding path (`OpenAiClient::gen_private_embeddings`, adapted/named appropriately for search)
so provider error bodies cannot reach diagnostics. Validate exactly one nonempty vector,
configured dimension, finite f64 values and finite f32 conversion. Use the same model and
`HELIX_VECTOR_DIMENSION` as ingestion; validate compatibility, not just vector length. Do not
mix historical models under one index; model changes need a separate reviewed reindex plan.

Trial cache policy: process-local memory, 5-minute TTL, at most 512 entries and 16 MiB of
retained key/vector data, whichever bound is reached first. Include provider identity, exact
trimmed query, model, dimensions, and preprocessing version in the key. Use a process-keyed
digest of the query so retained keys do not store plaintext; never log even digest keys.
Account for vectors, keys, expiry and entry overhead in the memory limit.

Coalesce concurrent misses for the same key, with bounded in-flight keys, waiters, and provider
concurrency. Cache successes only. Cancellation/error removes in-flight state and wakes waiters;
shared work survives only while another live waiter needs it, within a fixed deadline. Cache
hits do not extend TTL indefinitely. Eviction and model/config changes cannot reuse stale
vectors. Keep this cache server-side only; it contains no results, permissions, or document
content. Authorization runs on every retrieval including hits. Shared reuse across actors is
valid only for identical input/configuration; expose no hit status or cache timing telemetry to
clients. Test user-specific access on hits explicitly.

## Fusion and candidate budgets

Use unweighted reciprocal rank fusion (RRF):

`rrf(chunk) = Σ 1 / (60 + one_based_rank_in_source)`.

This combines ranks without comparing BM25 scores to cosine distances. See the
[Elastic RRF reference](https://www.elastic.co/docs/reference/elasticsearch/rest-apis/reciprocal-rank-fusion)
for the formula and candidate-window tradeoff. This is a local pure function, not a new
Elasticsearch dependency or model call.

For the default 30 results, initially retrieve 50 hits per source per deal. For larger limits,
use `min(100, max(50, limit))`; never request fewer candidates than the public limit. Deduplicate
each source by the complete immutable chunk identity before assigning rank; the same chunk in
both branches contributes once per branch and produces one passage result. Retain distinct
passages from the same file; document grouping is deferred. Tie-break by best source rank then
stable deal/file/version/chunk IDs, independent of future completion order.

Fuse per deal using that deal's keyword and vector ranks, then merge per-deal fused results
using the same deterministic ordering. This preserves the current lexical policy of local ranks
across deal corpora and avoids inventing comparable BM25 scores. It is a deliberate global
ranking policy, not RRF over a single global BM25 list; evaluate deal-size bias and recall.
Keep global candidate accumulation bounded (at most 200 deals × 2 sources × 100 hits), and
retain only bounded passage payloads needed for fusion. A small final list does not itself
bound total preparation work.

Before fusion, refresh current identities once per prepared deal and filter both branches
against the full identity tuple. Select the top `limit`, then use the existing
`current_result_ids` final snapshot (maximum 100). If concurrent revocation shrinks the list,
return fewer results; no automatic refill or permission-free fallback. A later refill design
would need its own bounded atomic verification contract.

## Indexed lexical retrieval: required capability gate

First spike the pinned Helix client/server combination on disposable synthetic data. Prove that
indexed text retrieval ranks within the authorized deal/current-version/generation candidates
**before** top-K, preserving all-token matching, punctuation, case behavior, and deterministic
ties. Prove correctness with high-scoring historical/deleted/unpublished distractors and a
relevant eligible passage below their unfiltered top-K. Check query plans/transfer size and
index readiness, rather than assuming the root text builder can consume a filtered stream.

If the SDK cannot express this, prefer a narrow upstream/client/server extension supporting
predicate-aware indexed text ranking. Record the exact capability/version and compatibility
tests before selecting an upgrade. Do not replace `keyword_current` with root BM25 top-K plus
Rust filtering, even with arbitrary overfetch: eligible matches can disappear.

If that path is unavailable, write a separate design decision for a SQLite FTS5 chunk-text
projection using the existing bound SQL client/builder. It requires durable text/checkpoints,
index publication and retry invariants, additive migration from the then-current schema marker,
backfill/recovery, analyzer parity, and data-preserving rollback. Template FTS is not a document
index. Do not silently add a second lexical store in the latency change.

The concurrent hybrid endpoint can ship with the current bounded scan first, explicitly
retaining capacity errors. The indexed milestone is complete only when Helix returns bounded
best eligible hits without transferring all candidate text.

A separate large-deal milestone must replace `ResourceAccess::current_chunks`' full identity
materialization and `verify_candidates`' full header scan with a proven indexed scope and
publication/completeness mechanism. Preserve canonical current/live eligibility and failure
on incomplete publication, final identity checks, and recovery invariants. Design and benchmark
this before raising either the 10,000-chunk or 5,000-document ceiling; no ceiling increase is
part of the first hybrid delivery.

## Delivery sequence and acceptance gates

| Phase | Work | Exit evidence |
| --- | --- | --- |
| 0 — Baseline | Instrument existing search, inspect Helix capabilities, build synthetic relevance/load cases, choose deadlines and queue bounds. | Per-stage timings and quality baseline; indexed feasibility recorded; no claim about the dominant bottleneck without measurements. |
| 1 — Shared preparation | Extract prepared deal scope and freshness helpers; retain old keyword/vector behavior. Add bounded deal scheduling and process read limits. | Legacy route/authorization tests pass; deterministic ordering and bounds under concurrent requests; no duplicated preparation in hybrid design. |
| 2 — Concurrent hybrid | Add DTOs/routes, injected query embedder, one embedding, overlapping branches, RRF, strict failure policy, and final checks. Initially use the existing lexical scan. | Barrier-based tests prove keyword starts while embedding is pending; one embed across deals; stale or unauthorized text never returned. |
| 3 — Query cache | Add bounded TTL cache, miss coalescing, cancellation, eviction and redaction tests. | Repeat queries skip provider work; access is rechecked; bounded memory/in-flight work under load. |
| 4 — Shared UI | Add typed HTTP/Tauri adapter methods and switch Data Room requests to hybrid on explicit submit. | Both distributions show truthful sources and all async states; old keyword APIs remain usable. |
| 5 — Indexed lexical | Implement the proven filtered-index approach, with readiness/parity checks and reviewed rollout if versions/indexes change. | Correctness/relevance parity plus bounded text transfer; indexed speedup measured separately from concurrency/cache gains. |
| 6 — Large deals | Independently redesign preparation/completeness and remove ceilings only after proof. | Search beyond 10,000 chunks without full corpus transfer/materialization, preserved access/publication guarantees, documented recovery. |

## Frontend and desktop integration

Add `searchDealDocumentChunksHybrid` and the global counterpart to
[QuarryApi](../frontend/src/contracts/quarryApi.ts),
[HTTP documents adapter](../frontend/src/api/http/documents.ts), and
[Tauri adapter](../frontend/src/api/tauriQuarryApi.ts). Inspect the relay allowlist in
[quarry_api/client.rs](../frontend/src-tauri/src/quarry_api/client.rs) and its service/model
validation; do not assume arbitrary new paths already pass. Make only a narrow addition if
required. No embedding or fusion belongs in Tauri or React.

Keep keyword search's debounce where it remains in use. For hybrid in the Data Room, separate
draft and submitted text and submit on Enter or a Search button to avoid paid requests on every
keystroke. Extend `DocumentSearch`/its request owner deliberately so other consumers retain their
existing behavior. Reset submitted scope with a deal/user-keyed owner. Invalidate stale results
on new submission, editing the submitted query, closing, or scope change; keep retry bound to the
submitted query. Preserve focus revalidation without multiplying embedding calls through rerenders.
Use request callbacks/reducer events, not effect-body state writes. Carry cancellation through
adapters where supported; otherwise suppress late completions and rely on bounded server work.

Show loading, empty, retryable error and unavailable semantic capability distinctly. Offer an
explicit keyword-search action when hybrid is unavailable; never silently change retrieval mode.
Label hybrid search accurately and show Keyword/Semantic source evidence. Highlight only literal
matches as escaped React text; semantic-only passages need no invented highlight. Preserve
current result navigation: only the open file with a valid reported page range can activate.
Cross-document navigation remains deferred, with the existing explanation visible.

## Measurement and test plan

Record structured durations for scope authorization, queue wait, candidate snapshot, header
verification, embedding/cache lookup, keyword retrieval/transfer/ranking, vector retrieval,
freshness, fusion, final access, and total request time. Record counts/byte totals, hit/miss,
error categories and active work without query text, document text, vectors, user email, keys,
or provider bodies. Use tracing spans that skip sensitive arguments. Do not log responses.

Compare sequential and overlapped orchestration with identical data and candidate budgets,
cache cold/warm separately, and indexed retrieval separately from both. Measure p50/p95/p99,
throughput, provider calls, queueing, transferred bytes, and peak memory for 1/10/200 deals,
empty/no-match cases, and increasing chunks up to current ceilings. Beyond-ceiling cases must
remain explicit errors until Phase 6. Test concurrent users so per-request speedups do not
hide process-wide overload. Set numeric latency/relevance release targets after baseline review.

Use synthetic judged exact identifiers, C++/C#, multiword queries, paraphrases, mixed formats,
image-description passages, long/short documents, and duplicate text in distinct deals. Compare
keyword/vector/hybrid Recall@K, MRR/nDCG and excerpt usefulness at the UI limit. Protect exact-term
quality and evaluate per-deal fusion bias; do not promise all-token semantics for semantic hits.

Required automated coverage:

- Handler/route: authenticated scope, path authority, blank/oversized query, unknown fields,
  limit bounds, camelCase DTO, missing provider, sanitized branch timeout/failure.
- Service concurrency: deterministic barriers prove overlap (no flaky elapsed-time assertions),
  one embed across deals, one preparation per deal, no calls for empty scope, configured work
  bounds, branch error cancellation and no detached task leakage.
- Security: collaborator/owner and inaccessible deals, membership removal, file deletion,
  version/generation/hash replacement during retrieval, missing manifests/headers, incomplete
  index, same content in two deals, cache hits under different actors, final-check races.
- Fusion: overlap sums, no duplicate contribution, empty/single branch lists as pure function
  inputs, stable ties and completion-order independence, candidate truncation, fewer results
  after revocation. Endpoint success still requires both branches.
- Cache/embedding: exact model/dimension keying, malformed/nonfinite vectors, TTL via controlled
  time, entry/byte eviction, concurrent misses, failed entries, cancellation and waiter limits.
- Compatibility: extend [shared search integration coverage](../backend/tests/integration/shared_document_search_tests.rs)
  on an explicitly selected disposable Helix instance; preserve legacy query/repository tests.
  Add new unit modules under `backend/tests/unit/` with the required `#[path]` inclusion hooks.
- Frontend: HTTP/Tauri paths and encoding, submitted-query lifecycle, loading/empty/error/retry,
  late completion, deal/user switches, badges, safe highlighting, focus/keyboard, valid/invalid
  page navigation, no-preview search, explicit keyword fallback. Extend existing document-search
  component and `frontend/tests/api/dealMembershipApi.test.ts` coverage.

Implementation gates, run from their actual build roots:

```sh
# backend/
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# frontend/
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# frontend/src-tauri/ — if native relay code changes
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Run relevant existing focused tests first. Do not invent a separate Cargo test target:
`autotests = false`. Inspect web and desktop loading/error/result behavior, network/console,
keyboard/focus, light/dark themes and relevant viewports with synthetic authorized data. Record
and stop only task-owned listeners. No real database mutation, live embedding call, destructive
utility, migration, or integration run is authorized by this planning document.

## Rollout and completion

Deliver backend changes additively before switching the UI. Keep a typed server enablement
setting for the new UI/API rollout with a clear unavailable response; do not broaden environment
access into services. Retain the legacy keyword route for explicit fallback and rollback.
Disable the hybrid UI/route to roll back concurrency/cache/fusion work; no data rollback is
needed when projection/schema is unchanged. Indexed or large-deal changes need their own
backup, compatibility, retry, readiness and rollback procedure before rollout. Never delete
canonical SQLite rows or run `clear_helix`/`reset_development` to make search pass.

Implementation is done when overlap and single-embedding behavior are demonstrated, access
invariants hold under races, bounded work survives load, relevance meets baseline-derived
targets, both runtime adapters and UI states work, and applicable native gates pass. Report the
remaining scan/candidate ceilings honestly if only the initial delivery is complete.

Update `docs/ARCHITECTURE.md` during implementation for new routes/DTOs, orchestration,
configuration/cache lifetime, error behavior, UI submission/source labels, and any index/schema
or recovery changes. Recheck sections 5, 6, 7.3, 8, 9, 10.4, 11, 12, 13 and 14.

Architecture impact of this planning change: none — only proposed work is refreshed; current
routes, runtime boundaries, data ownership, schema and behavior remain unchanged. No application
builds, tests, external calls or migrations are claimed as executed by this plan.
