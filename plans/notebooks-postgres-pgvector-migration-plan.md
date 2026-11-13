# Plan 2: atomic notebook saves and PostgreSQL search

Date: 2026-11-12. Status: proposed follow-up PR; no implementation or data changes.
Depends on the shared extension/vector/bulk/search contract in
[Plan 1](documents-postgres-pgvector-migration-plan.md#2-common-storage-and-search-decisions-for-all-three-plans).
Followed by [templates and final Helix retirement](templates-postgres-pgvector-migration-plan.md).
Baseline: [notebook API](../docs/notebook-api.md) and [architecture](../docs/ARCHITECTURE.md).

## 1. Outcome and deliberate behavior change

Replace `helix.notebooks.indexes` and notebook publication/readback/delete with PostgreSQL
note artifacts/chunks, pgvector and indexed keyword search. Native editor blocks stay canonical;
Markdown remains a deterministic lossy derived representation, never the input for reconstructing
editable blocks. A notebook linked to a shared deal stays private to its notebook owner.

Today a save job first commits the note revision, then exports/chunks/embeds and publishes to Helix.
Index failure can therefore leave a new saved native revision with failed enrichment. Under this
plan, **the native note revision, title/location, complete derived chunk/embedding set and terminal
job result commit together**. Any chunk/vector failure rolls back the whole save and leaves the
previous saved revision and its search results unchanged. No SQL transaction spans an embedding call.

The atomic unit is one submitted note document, not every note in the notebook. Unrelated notes
prepare and commit independently; parent lifecycle commands still coordinate with child commits.
Keep 202 durable admission and asynchronous revision conflicts. Admission is not a successful Save.
The accepted immutable snapshot can remain durable when preparation fails, but it is not exposed
as the canonical saved note. Empty notes are valid: zero chunks is a complete explicitly counted
artifact, not an indefinitely pending vector job; no embedding call is needed for an empty batch.

## 2. Proposed data ownership

Use the same SQL pool and installed vector extension/profile policy as Plan 1. Notebook/document
embeddings use compatible declared profiles; templates may differ. No second extension setup.

| Record | Proposed responsibility |
| --- | --- |
| `notebooks`, `notebook_folders`, `notebook_notes` | Retain ownership, native blocks, title/folder, bounded note revision and same-notebook FKs. Add a single active search-set reference with a same-note composite FK. |
| Proposed `notebook_search_sets` | Note ID, immutable set ID, content/preparation/profile fingerprint, deterministic Markdown, expected chunk count and complete state. Source fingerprint covers embedding input, including title/blocks but excluding folder/deal metadata. |
| Proposed `notebook_chunks` | Search-set/note identity, ordinal/hash, chunk text, section/context, vector(D) and stored lexical vector. Unique `(search_set_id, ordinal)`, exact-profile/dimension/range/finite checks and cascading FKs. No successful row may have a missing required vector. |
| `notebook_save_jobs` | Immutable submitted snapshot, operation/payload hash, expected revision, profile/pipeline, stage/sequence, current attempt token/DB-time expiry and final committed revision/set ID. |
| Admission state | User-scoped admission coordination and independently claimable slots preserving 256 active global/32 per user bounds. Replay allocates no second slot. |

For a new note, insert native row and complete search set within the same transaction, using an
explicit deferrable same-note active-set FK if needed for insertion order. Do not create a visible
empty canonical placeholder before preparation. Previous set deletion and pointer replacement are
transactional; old read snapshots retain their old view through MVCC. Do not store an entire history
of vectors by default when the product has no note-history feature.

Simple folder-only movement can reuse a complete immutable set after validating its exact content/
profile fingerprint. Note revision still advances but artifact identity need not: expose current
revision through the note join, not a copied outdated revision on each chunk. Folder relocation
updates location/revisions under existing guards without re-embedding unchanged text. A title or
block change requires a matching new artifact. Never reuse vectors based only on equal dimensions.

Use owner/notebook/note/set B-tree indexes and GIN lexical indexes. Apply Plan 1's exact-first cosine,
profile checks, selective ANN/half-precision-index/full-precision-rerank policy and measured fallback.
The active set is the only searchable set; pending jobs, failed prepared input and old artifacts
cannot leak into retrieval. Denormalized owner/notebook keys require composite integrity constraints
or a canonical join; caller-supplied owner values never establish access.

## 3. Save protocol and failure semantics

1. At HTTP admission validate operation ID, expected revision/null semantics, schema, title/blocks,
   folder input and limits before product work. Authorize notebook/note/folder through service policy.
   Require embedding capability only for work that needs embedding; fail explicitly if unavailable,
   never silently mark a nonempty native-only save complete.
2. In a short admission transaction acquire the user admission row, notebook parent FOR SHARE,
   then operation/job identity and a free slot, following Plan 1's order. Persist immutable accepted input, expected revision,
   operation hash and pinned profile; exact replay returns its job. No canonical note is changed.
3. Claim one eligible job atomically with SKIP LOCKED; record token, expected stage and DB-time expiry.
   Export Markdown, chunk and embed outside resource transactions, with bounded concurrency/memory/
   timeout/cancellation. Preserve the current Markdown/chunk format unless a versioned change has
   fixtures. Validate all counts, chunk IDs/hashes and vectors before entering the final transaction.
4. Final transaction obtains the notebook shared lock, target folder shared lock when needed and
   current note protection, then rechecks job attempt/stage/status/DB-time expiry, notebook owner,
   folder containment and submitted revision. Never replace expected revision with a new read.
5. CAS the native note snapshot/revision (or insert the new note identity), write the complete search
   set and every chunk/vector/lexical row via bounded batch statements on this same connection.
   Verify expected count, contiguous ordinals and profile/fingerprint; update active set and remove
   superseded artifacts. A row failure, count mismatch or deferred FK failure aborts everything.
6. Mark the job completed, committed revision/set and monotonic event sequence and release its slot
   exactly once in this transaction. Retain the final token/status/expiry predicate after any waits;
   a stale attempt fails rather than committing note content. Only then commit and emit completion.
7. If preparation/final commit fails, record a sanitized job failure through a current-attempt fence
   in a separate small transaction. It cannot downgrade a completed job or a successor attempt.
   Old note/search state survives; the submitted snapshot remains recoverable for explicit retry.
   A lost commit response is reconciled by operation ID before any work is replayed.

The job-only claim/admission/stage transactions are not partial note commits. They may report progress,
but `sqliteCommitted`/`committedRevision` remain false/absent until the complete note transaction
commits. Keep stage names and SSE events compatible where possible: preparation may now be exporting,
chunking, embedding before the final persisting/indexing stage. Update clients/tests that assumed
persisting occurred first; never display Saved based only on stage ordering or a local job admission.

For writes linked to deal authority, use Plan 1's sorted deal/user-admission/parent/job/slot order.
Separate claim-only transactions do not acquire parents. Admission must not take a notebook lock
and then wait for a user row in reverse order; finish/delete must not hold a slot before its parent.
Verify terminal transitions only reduce active counts, so releasing their slot does not require
reacquiring the per-user admission row. Prove admission versus final commit/cascade cannot overfill
32/256 capacity or deadlock, including mixed parent batches and aborted transactions.
Keep one active note save, immutable replay and pipeline-change checks. Do not hold an exclusive
notebook lock across independent note saves or external work.

## 4. Revision conflicts, retries and the editor

The frontend already distinguishes submitted/saved/newer edits; preserve that split. A failed save
keeps the last committed revision and both the submitted snapshot and newer local edits. A successful
job acknowledges only its submitted version; it must not overwrite typing that happened while it ran.
An explicit reload requires discard confirmation when edits exist. Revision conflicts are not retried
against a newer revision automatically.

Retry can reuse prepared artifacts only with exact submitted-content/profile/pipeline fingerprints
and a valid current attempt. If no canonical commit occurred, retry the whole final atomic save;
if it did, return the original committed identity/revision instead of incrementing again. A client
reconnect on a different Axum instance reads durable status/events and the same canonical result.
Keep notebook/owner bounds and one active save invariant across processes, not just one local worker.

Metadata-only movement with identical Markdown may skip provider work, but still performs a full
revision/parent/attempt check and atomic job completion. Newer note edits, folder deletion/relocation,
notebook deletion and ownership denial invalidate stale saves at final commit.

## 5. Keyword and semantic query API

Today Notebook has metadata/list search, not a public semantic content search endpoint. Do not
claim that merely storing vectors makes semantic search available. Add a narrow authenticated API:

- Proposed `POST /api/v1/notebooks/{notebook_id}/search` with strict
  `{queryText, mode: "keyword" | "semantic" | "hybrid", limit}`. Proposed bounds: nonblank text up to
  2,000 Unicode scalar characters, limit 1..50. Handler validates all shape/path facts before calls.
- Authorize notebook ownership; optional deal membership never grants access to private notes.
  Query current active sets joined to owner/notebook rows before ranking/limits. Keep title/body
  weighting and technical-token fixtures consistent with Plan 1; GIN is the default inverted index.
- Query embedding happens before the read snapshot, outside SQL locks. Hybrid branches share one
  read-only snapshot and deterministic RRF. Rank exact cosine in SQL; ANN only after scoped recall
  evidence. No full vector collection pulled into Rust for ranking.
- Return bounded plaintext hits with note/notebook ID, current revision, chunk ordinal, title,
  excerpt and match sources. Markdown positions are not native editor block/page offsets; do not
  invent navigation anchors until a tested mapping exists. No raw embeddings or native blocks.
- Recheck bounded result ownership/active set and session before delivery. Deleted/changed notes
  cannot be returned from stale candidates. Semantic/hybrid without a configured query embedder
  fails explicitly; caller can deliberately choose keyword mode. No silent degraded success.

Wire the typed contract, web adapter, desktop adapter, Tauri allowed route/body validation and route/
adapter tests if this API ships in this PR. Do not widen generic native fetch permissions. A new
search UI is a separate product change; preserve the current sidebar's behavior until explicitly
integrated, rather than silently relabeling its local metadata filter as semantic search.

## 6. SQL-only deletion and recovery

Note deletion keeps displayed expectedRevision CAS. In the same transaction remove the note,
active set/chunks, fence/cancel admitted work and resolve owned slots. Parent/folder deletion uses
existing lock protocol; folder relocation preserves unchanged artifacts while advancing revisions.
A paused worker must fail its final transaction after the note/notebook disappears or job is fenced.
No remote tombstone or eventual Helix cleanup is necessary for new SQL-only notes.

Retire notebook `graph.rs` publication/readback and `notebook_cleanup` remote polling only after
all readers/writers move. If old Helix cleanup records remain during cutover, classify them in an
explicit one-time migration; do not declare remote data deleted by removing its ledger. Keep old
Helix stores intact until authorized retirement. SQL cascades replace remote cleanup for new data;
any remaining cleanup queue must identify a real external effect, not a redundant SQL mirror.

Crash tests distinguish admission only, prepared only, in-transaction rollback and committed-but-
unacknowledged. Ordinary SQL visibility replaces remote fences; durable job attempts and lifecycle
checks still protect SQL from late provider results. Removal of global writer controls remains a
separate acceptance gate in the concurrency continuation, not an automatic consequence of this PR.

## 7. Implementation owners and rollout

| Existing path | Work |
| --- | --- |
| `backend/src/domains/notebooks/{repository.rs,job_repository.rs}` | Atomic native/artifact batch transaction, parent/revision/attempt checks, slots, SQL search and cascades. Split a feature-owned search module if needed, not a generic persistence framework. |
| `backend/src/domains/notebooks/{save_jobs.rs,graph.rs,markdown.rs,model.rs,service.rs}` | Prepare before persist, typed prepared batch, durable recovery, remove Helix pipeline dependency; retain deterministic native-to-Markdown export and bounds. |
| `backend/src/domains/notebooks/{handler.rs,handler/validate.rs,route.rs}` | Existing 202/SSE/revision behavior plus strict owned query API if included. |
| `backend/src/app/{schema.sql,migrations.rs,bootstrap.rs,config.rs}` | Artifact DDL/profile/index readiness, one SQL service assembly; remove notebook Helix initializer. Documents/templates may still use Helix during ordered rollout. |
| `frontend/src/data/notebook/`, notebook API/contracts and native relay | Verify committed flags, captured revisions, retry/reconnect, newer edits and explicit reload; wire new query API without broadening transport. |
| `backend/tests/`, `frontend/tests/` | Atomicity, job/parent races, real query/index tests and saved-buffer/transport regressions. |

Implement after shared infrastructure is merged, with its schema version as the real baseline; do
not invent a conflicting fixed version number. Choose fresh empty target or preserving backfill
explicitly. For populated notes, regenerate Markdown/chunks deterministically from native blocks;
reuse existing embeddings only if exact content/profile identity is provable, otherwise explicitly
schedule re-embedding. No provider spend or old-store deletion is authorized by saving this plan.

Drain old workers before changing save ordering/schema. Backfill each note completely in one SQL
transaction, validate counts/fingerprints and publish its active reference only on success. Do not
hold a transaction across backfill provider work. Old source/Helix can remain read-only for rollback;
never permanent dual-write. Rollback compatibility and a forward-fix path must be documented before
cutover. Update API docs to make the changed save-failure semantics unambiguous.

## 8. Acceptance and checks

- Inject a bad first/middle/last chunk/vector, index/constraint failure, final job-fence loss and
  deferred commit error after earlier batches succeeded. Native title/blocks/folder/revision,
  active pointer/chunks, lexical/semantic results and committed job fields all remain unchanged.
- Prove a new note cannot be read/listed/searchable before the complete commit. A failed creation
  leaves only its accepted job/input, not a canonical placeholder. Empty notes complete correctly.
- Concurrent readers see old complete or new complete content/search, never mixed revision/sets.
  Separate notes progress while one transaction/provider is paused; stale same-note save loses.
- Folder-only changes reuse exact artifacts safely; title/block changes never reuse mismatched
  vectors. Folder relocation/delete, note-delete revision, notebook cascade, retry and prune races
  cannot resurrect or misplace notes or leak a slot. Capacity remains 256/32 across processes.
- Provider failure, unconfigured embedding, pipeline/profile mismatch, cancellation and crash before/
  after commit preserve the accepted snapshot and correct saved/unsaved feedback. Lost response
  reconciliation increments once. Typing during save, undo-to-saved, retry and reload confirmation
  work in both clients. Test cross-process durable status/SSE without local notifications.
- Query privacy across owners, shared-deal isolation, stale result recheck, Unicode/technical/phrase
  lexical fixtures, cosine reference scores, selective ANN recall and actual index query plans.
  Apply Plan 1's relevance/latency/recall/memory/rollback evaluation to skewed notebook sizes.

Run focused owning suites then backend fmt/check/Clippy and the full explicitly isolated `_test`
database suite with real pgvector and fake providers. Run frontend `npm run typecheck`,
`npm run check:boundaries`, `npm test`, both builds and each matching bundle/notebook check when
clients change; native fmt/Clippy/tests for relay changes. Confirm commands against live manifests.
Manually inspect any changed save/retry/conflict/search UI, focus, themes and reduced motion.

Update architecture, notebook/storage/API recovery documentation, verification record and affected
guidance with the actual new commit boundary. Review errors, authorization placement, concurrency,
performance and scoped refactoring. Architecture impact of this planning-only file: none.
