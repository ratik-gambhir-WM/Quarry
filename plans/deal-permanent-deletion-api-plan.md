# Permanent deal deletion API plan

Date: 2026-10-15. Status: proposed; no deletion, schema change, or API implementation has been performed.

This plan is grounded in the current working tree, including its uncommitted deal/company and
chat changes. [Architecture](../docs/ARCHITECTURE.md) describes the implemented product; this
document describes proposed work. Preserve existing changes. Earlier plans are design inputs,
and their references to authorized development resets do not authorize a reset for this work.

## 1. Outcome and recommended approach

Add `DELETE /api/v1/deals/{dealId}` to permanently remove a deal and all Quarry-owned data that
belongs to it: metadata, company-role assignments, diligence scopes, stored documents and every
version/blob, linked chats and messages, and Helix document/search projections.

Use a **durable asynchronous deletion operation**. SQLite and Helix cannot commit one transaction
together, and ingestion/chat work can still be running. A successful DELETE acceptance must
create durable work, prevent further use of the deal, and return an operation to track. Only mark
that operation complete after physical application records and search projections are gone.
This is permanent deletion, with a temporary cleanup phase and no restore/archive substitute.

Add `deal_id` to Helix file, version, and chunk nodes, alongside their current identities. This
supports direct deal-scoped search and cleanup. Retain a durable file-identity manifest so cleanup
can also remove legacy or partially indexed nodes whose deal property is missing or incorrect.

Do not implement this as one SQL DELETE, an archive flag, a process-local background task alone,
or a global Helix clear. Do not delete shared company records or source files outside Quarry.

## 2. Findings that shape the implementation

| Current implementation | Design consequence |
| --- | --- |
| [Schema marker 9](../backend/src/app/migrations.rs) cascades deal metadata, parties, scopes, files, versions, and blobs. | Use those cascades inside one canonical deletion transaction, including historical and soft-deleted files. |
| `assistant_threads.deal_id` uses `ON DELETE RESTRICT`; messages cascade from threads. | Explicitly delete all deal-linked threads before deleting the deal, in the same transaction. Retain RESTRICT as a guard against an incomplete workflow. |
| [Helix nodes](../backend/src/domains/documents/index/model.rs) carry workspace/file/version identities, without deal identity. | Add deal identity and backfill it from canonical SQLite ownership. Do not replace stable file/version IDs. |
| [Ingestion persistence](../backend/src/domains/documents/ingestion/persistence.rs) commits SQLite before writing a graph in several batches. | Snapshot deletion targets before deleting SQLite records; remove unpublished/partial graph nodes as well as published versions. |
| [Document jobs](../backend/src/domains/documents/ingestion/job_service.rs) retain only watch senders by job ID and lose state on restart. | Add deal ownership and cancellation/drain handles to running jobs; use a separate SQLite-backed deletion ledger for durable cleanup. |
| [Persisted chat runs](../backend/src/domains/assistant/chat/service.rs) spawn provider-stream work. | Stop linked streams and prevent their final callbacks from writing after deletion. Include archived threads, not just the current thread list. |
| [Search](../backend/src/domains/documents/search/service.rs) currently returns Helix results without canonical SQLite liveness checks. | Filter against live canonical ownership before returning results or supplying context, including while Helix cleanup is unavailable. |
| [Data Room lookup](../backend/src/domains/data_rooms/service.rs) can fall back to a configured root when its source reader returns no path. | Check deal existence/deletion state before any source resolution; deleting metadata must never reopen a configured folder through fallback. |
| Preview bytes are cached in memory; client annotations are also retained in memory. | Evict server previews and provide client invalidation hooks; an API cannot retract copies already downloaded or held by offline clients. |
| The [desktop DELETE relay](../frontend/src-tauri/src/quarry_api/client.rs) accepts only `204` and discards bodies. | Add a typed JSON DELETE path for the new `202` operation response without breaking existing thread/template deletion. |
| [CORS middleware](../backend/src/app/http/middleware.rs) already allows DELETE. | Preserve origin restrictions; no broad CORS expansion is needed for the verb. |
| Current profiles/caller-supplied emails are not authentication. | Bind destructive requests and operation polling to a trusted actor and owner policy before public exposure. |

The Axum skill contains an older schema-reset description. The live schema code, root guide,
and architecture take precedence: marker 9 is reused without deleting records. The Data Room
fallback code also needs particular attention rather than relying on older narrative assumptions.

## 3. Deletion ownership and limits

| Data | Action |
| --- | --- |
| `deals`, `deal_metadata` including extracted questions/source links, `deal_parties`, `deal_scopes` | Permanently delete. |
| Every `quarry_files` row for the deal, all `quarry_file_versions`, and `quarry_file_blobs` | Permanently delete, including non-current versions and rows already marked deleted. |
| Every `assistant_threads` row with this exact `deal_id`, including archived threads, and all their messages | Permanently delete. Do not detach them into freeform chats. |
| Helix `QuarryFile`, `FileVersion`, `FileChunk`, embeddings, text, and connecting edges for those files | Permanently delete all versions/generations and partial graph writes. |
| Running ingestion/extraction/chat/preview work and retained document-job events for this deal | Cancel or drain; purge retained results; reject late publication. |
| Server preview cache | Evict affected bytes. Its current key lacks deal identity; clearing the bounded cache is an acceptable initial implementation. |
| Connected client deal/document/chat caches, editor annotations, active streams, and route-local selections | Invalidate through explicit consumer hooks; ignore stale responses that would restore the deleted deal. |
| Reusable `companies`, users, other deals and their files/chats | Preserve, even when company roles or document content match the deleted deal. |
| Freeform chats or chats linked to a different deal | Preserve. A textual mention is not an ownership relationship. |
| Local/SharePoint source files, external SOW/fact-sheet/request-list URLs, exported/downloaded files | Preserve originals; remove Quarry's stored copies and references only. |
| Diligence Studio's app-wide templates | Preserve. There is no current durable deal-owned deliverable/template relationship to cascade. |
| `reminders.deal` | It is unused free text, not a deal foreign key. Do not delete by name matching. Add a real relationship before promising reminder cleanup. |

Keep only minimal operational deletion receipts/fences after completion: resource IDs, owner ID,
operation state, and timestamps. Delete per-file/thread manifests when no longer needed for
recovery. Never retain names, document/chat contents, links, or source paths in the ledger.
These receipts cannot reconstruct a deal and are distinct from the deleted `deal_metadata`.

Deletion means removal from active application stores and indexes. SQLite pages/WAL, backups,
provider-side retention, logs, and user-made copies require separate retention/erasure policies;
do not claim cryptographic or forensic erasure from a normal DELETE response. Do not run global
VACUUM/checkpoint/reset operations inside the request. Recovery must not replay pre-deletion
backups into a live system without reconciling the deletion ledger.

## 4. Proposed API contract

### Requests and responses

`DELETE /api/v1/deals/{dealId}` has no request body. Use the existing identifier constraints and
URL encoding. Resolve the owner from a trusted request actor; do not use a body/query `userEmail`
or `workspaceId` as proof of permission.

- `202 Accepted`: durable deletion is registered or still pending. Return JSON and a `Location`
  header pointing to `/api/v1/deal-deletions/{deletionId}`.
- `204 No Content`: a retained receipt proves this actor's same deal was already fully deleted.
- `400`: invalid identifier. `401`: no trusted actor. `404`: absent/not-owned resource or receipt,
  without revealing another owner's data. `503`: deletion could not be durably registered.
- An archived deal can be deleted directly. Archiving is not a prerequisite.
- Repeated/concurrent requests for the same owned deal return the same operation; a lost `202`
  response is recovered by repeating DELETE. Do not require a new generic idempotency framework.

Example pending response, with camelCase JSON:

```json
{
  "deletionId": "<server-generated UUID>",
  "dealId": "<validated deal ID>",
  "status": "pending",
  "statusUrl": "/api/v1/deal-deletions/<server-generated UUID>"
}
```

`GET /api/v1/deal-deletions/{deletionId}` returns an owner-scoped status:
`pending | running | retrying | blocked | completed`, a safe phase name, `retryable`, optional
sanitized `errorCode`, and timestamps. No document names, paths, message contents, or provider
errors. Polling remains available after the deal row is gone. Use JSON's `statusUrl` so browser
and desktop clients need not depend on access to the Location header. Send `Cache-Control: no-store`.

After acceptance, omit the deal from lists, deny new writes with `409 deal_deletion_in_progress`,
and make normal deal/document/chat/source reads return `404`. A normal lookup must never return
a partially purged aggregate. After completion, those resources stay absent. Existing bytes/events
already delivered cannot be recalled; stop future emissions and recheck liveness before returning
in-flight results.

Reserve deleted deal IDs using the minimal receipt; creation returns a conflict for a reserved
ID. This avoids an old DELETE retry or delayed worker acting on a newly created deal with the same
ID. Use a new ID for a new deal. Receipt retention/ID-reuse changes require an explicit future
generation scheme, not deletion of the receipt as an incidental cleanup.

### Identity prerequisite

Introduce a narrow trusted-actor extractor and deletion access policy, injected through bootstrap.
Check ownership in both the acceptance transaction and status lookup. Current profile lookup does
not supply this trust boundary. Integrate actual session/token identity before enabling these
routes outside isolated development; when no trusted actor resolver is configured, fail closed.
Tests may inject a deterministic actor. Do not put server provider keys into clients or mistake
Tauri origin validation/CORS for owner authorization. Define CSRF protection if cookie sessions
are selected. This plan does not relabel the existing login as authenticated.

## 5. Add deal identity to Helix safely

1. Add `deal_id` to `FileNode`, `FileVersionNode`, `FileChunkNode`, prepared graph identities,
   write parameters, projections, and decoded results. Populate it from the SQLite-validated
   ingestion input, never a separately trusted client property. Enforce agreement across nodes.
2. Keep `workspace_id`, `file_id`, `version_id`, and deterministic chunk/version identities.
   Deal identity is an ownership/scope property, not a replacement primary key. Never delete
   by content hash: identical bytes can belong to another deal with a different logical file ID.
3. Add suitable workspace/deal lookup indexes for all three node labels and include them in
   index-readiness checks. Preserve existing vector/keyword index behavior.
4. Add an optional validated `dealId` filter to existing search requests. Workspace-wide search
   remains supported; a scoped query must enforce both owner workspace and deal. Update Rust
   DTOs, TypeScript contracts, web/desktop mappings, and tests together. Preserve current wire
   casing for existing Helix result fields rather than silently renaming that contract.
5. Roll out reader compatibility first, then writers that always supply deal identity. During
   transition, tolerate a missing property only while resolving ownership from SQLite and never
   trust missing/mismatched ownership for authorization or search visibility.
6. With writers drained, backfill every existing node from `quarry_files` ownership, across all
   versions/chunks, including nodes without published edges. Change properties in place; do not
   regenerate embeddings or reset either database. Make the bounded backfill resumable and
   idempotent, and verify workspace/file/deal consistency. Remove transitional tolerance after
   verification. Keep the deletion feature disabled until backfill/validation completes.
7. Reconcile graph nodes with no canonical file separately: they cannot be assigned a deal by
   guessing from text/hash. Suppress them from search, report non-content counts, and investigate
   the selected environment before any broader purge. The saved deletion manifest remains
   sufficient to clean files removed by a registered operation.

Use a deal predicate for normal cleanup, plus exact manifest-scoped file predicates for legacy or
inconsistent projections. Verify every candidate belongs to the captured canonical file set before
deleting; a bad `deal_id` property must not cause another deal's file to be removed. Detect and
report unmatched/misowned graph records instead of calling the operation complete.

## 6. Durable workflow and concurrency

### Persistence and module ownership

Add `deal_deletions` with a unique deal ID, owner, operation UUID, state/phase, attempts,
retry scheduling, timestamps, and a sanitized error code. It must survive deletion of `deals`;
do not give it a cascading foreign key to that row. Add bounded per-operation target rows for
workspace/file identities and linked thread IDs, with no document or chat content.

Add a focused `domains/deals/deletion/` service/repository module, a narrow lifecycle gate exported
by the deals domain for its consumers, and collaborator interfaces for documents/chat/cache cleanup. The deletion service
owns orchestration; its repository owns the cross-table SQLite transaction. Helix deletion stays
inside the document-index repository/writer. Bootstrap supplies all collaborators and runs the
worker; route state contains service handles, not repositories/configuration. Keep SQL in the
existing bound query builder/client and blocking work in its established async offload boundary.

### Execution sequence

1. **Accept and fence.** In one SQLite transaction, verify actor/owner, return an existing receipt
   when applicable, and insert the operation. The committed operation is the persistent deletion
   barrier; all normal reads/writes consult it. Signal the worker after commit and return `202`.
   If signalling is lost, startup/periodic ledger scanning still discovers the work.
2. **Stop producers.** Close admission for deal mutations, uploads, metadata extraction, chat
   creation/runs, source previews, and graph publication. Cancel queued/provider work where safe;
   wait for already issued side effects and blocking conversions to settle. Track jobs by deal and
   their real task lifetime, not just the request/stream lifetime. Do not hold a SQL transaction
   while waiting. Never assume dropping a future stops `spawn_blocking` or a remote write.
3. **Drain and snapshot.** Acquire the exclusive per-deal lifecycle gate. Operations admitted
   earlier must hold a shared guard through persistence/publication, including spawned workers.
   Within a short SQLite transaction, recheck ownership/barrier, capture every file and linked
   thread, and persist the complete manifest before deleting any canonical data. Snapshot all
   files regardless of `deleted_at`, and include every version when checking counts/invariants.
4. **Block late Helix writes.** Establish an idempotent, minimal Helix deal-deletion fence. Setup,
   chunk insertion, and publication must atomically refuse a fenced workspace/deal in their
   server-side write transaction. Keep this control record separate from file/document nodes.
   Verify the pinned SDK/server can enforce the condition and ordering; application-only checks
   cannot prevent an already-sent remote request from arriving late. If this guarantee cannot be
   proven, leave the operation blocked rather than claim completion; resolving that mechanism
   is an implementation prerequisite. Never blindly retry a non-idempotent ingestion write.
5. **Purge SQLite.** With producers drained and the manifest/fence durable, delete linked threads
   by exact `deal_id` and then delete the owned deal in one transaction. Messages cascade from
   threads; metadata/parties/scopes/files/versions/blobs cascade from the deal. Commit the phase
   checkpoint atomically with that deletion. Keep foreign keys enabled. Verify parent-message
   constraints using branching chat histories; diagnose inconsistent cross-thread references
   rather than deleting unrelated threads to bypass a constraint.
6. **Purge Helix.** In bounded, repeatable writes, delete chunks, versions, files, and their edges
   by validated workspace/deal ownership and manifest file identities. Property predicates must
   reach orphaned/unpublished versions/chunks without traversing only CURRENT_VERSION edges.
   Treat already-absent records as success, preserve global indexes, and respect the existing
   2 MiB request cap. Checkpoint batches so a large deal does not require one request-sized purge.
7. **Evict and verify.** Purge retained job events/results and server previews; terminate linked
   streams. Verify no canonical children, graph nodes/edges, keyword/vector hits, or active
   producers remain. Verify both deal predicates and manifest identities. Filter every search
   hit against canonical live ownership throughout cleanup, even if Helix is down.
8. **Complete.** Mark completed only after verification. Remove temporary manifests once their
   recovery duty ends, retaining only the minimal receipt and durable late-write fence. Preserve
   the non-content control records needed to reject old requests and ID reuse.

Manifests must cover the complete target set, not a capped first page. Bound batch sizes and
worker concurrency, checkpoint progress, and never silently truncate a large deal's cleanup.
After filtering stale/deleting hits from search, use bounded refill where supported; return fewer
results rather than leak removed content or spin through an unbounded search loop.

The first implementation supports Quarry's current single active Axum process, with restart
recovery. Explicitly enforce/document that deployment constraint. Per-process locks do not support
multiple writers; multi-instance operation requires distributed leases/fencing before enablement.
Never silently broaden the existing process-local coordination guarantee.

### Recovery and failure policy

- Bootstrap resumes incomplete ledger entries before normal deal work is admitted; do not reuse
  the in-memory document-job retention mechanism as the deletion source of truth.
- Retrying a phase converges after a crash between a remote write and its SQLite checkpoint.
  On an ambiguous Helix acknowledgement, reconcile state; do not infer completion from a timeout.
- Helix outage after SQLite purge leaves durable manifest work in `retrying`, with bounded
  exponential backoff and a next-attempt time. The deal stays inaccessible; do not restore it or
  recreate its blobs to make the error disappear.
- Persistent identity/constraint errors become `blocked` with a safe code and an operator-visible
  diagnostic. They do not expire or silently drop work. The same DELETE can wake a recoverable
  operation; provide an internal retry tool/service hook instead of requiring database edits.
- Client disconnect/request timeout cannot cancel an accepted deletion. No user-facing undo is
  available after acceptance. Before acceptance failure, no deletion barrier/data change commits.
- If shutdown interrupts a worker, keep durable phase/targets and resume them. No real environment
  reset or global `clear_helix` is part of recovery.

## 7. Implementation sequence and file map

| Phase | Work and primary anchors | Exit criterion |
| --- | --- | --- |
| 1. Contracts and proof | Deletion DTOs/access policy; disposable Helix tests for scoped drops, edge/index cleanup, and atomic late-write fences; existing timeout/error mapping. | Accepted/completed semantics, trusted-actor integration, and the remote fence mechanism are explicit and testable. |
| 2. Additive persistence | [migrations](../backend/src/app/migrations.rs), [SQLite query/client](../backend/src/adapters/sqlite/client.rs), deletion repository, recovery tests. | Ledger/barrier/manifest survive rollback and restart without discarding existing application records. |
| 3. Helix ownership | [models](../backend/src/domains/documents/index/model.rs), [writer](../backend/src/domains/documents/index/writer.rs), [queries](../backend/src/domains/documents/index/query.rs), [repository](../backend/src/domains/documents/index/repository.rs), ingestion identity preparation, bounded backfill tooling. | All retained document nodes have validated deal identity; lookup/search filters and fences work. |
| 4. Runtime safety | Ingestion [service](../backend/src/domains/documents/ingestion/service.rs)/jobs, [chat service](../backend/src/domains/assistant/chat/service.rs)/repository, [Data Room](../backend/src/domains/data_rooms/service.rs), document viewing/search, metadata extraction. | In-flight work cannot recreate or return deleted data; missing/deleting deals cannot resolve source-root fallback. |
| 5. Orchestration/API | Deletion module, [deal handlers/routes](../backend/src/domains/deals/route.rs), [bootstrap](../backend/src/app/bootstrap.rs), service errors and operation lookup. | DELETE is idempotent, status is owner-scoped, and restart recovery converges across stores. |
| 6. Shared clients | [QuarryApi](../frontend/src/contracts/quarryApi.ts), HTTP/Tauri adapters, native JSON DELETE transport, runtime.desktop, adapter/native tests. | Both distributions can receive acceptance, poll status, and distinguish pending cleanup from completion. |
| 7. Consumer lifecycle/docs | Workspace removal/invalidation, chat/document cache hooks, architecture and a scoped deletion/recovery design record. | Stale requests cannot repopulate a deleted deal, and operational limits are documented. |

For phase 2, propose a targeted **data-preserving marker 9 to 10 upgrade** that creates only the
new ledger/target structures, plus the fresh-schema definition. Reconfirm the current marker at
implementation time. This intentionally extends today's initializer-only policy and must be
documented/tested as that narrow upgrade, not a general historical conversion ladder. Older or
unknown layouts still fail safely. Never use `reset_development`, edit local databases, or reuse
an earlier plan's reset authorization as an implementation shortcut.

For phase 6, preserve `delete(path): Promise<void>` and existing 204-only callers. Add a separate
typed JSON DELETE operation that handles `202` and a completed `204` deliberately; update its
native command, registration, runtime contract, and validation together. Keep the same restricted
API origin/path rules and no new filesystem/shell capability. Do not route this through an
archive POST simply because the current DELETE helper cannot return JSON.

The requested deliverable is an API plan. A visible Delete action/confirmation dialog is a
separate consumer task, not a reason to expand this work into a redesign. If added, it must name
permanent deletion and its scope, require deliberate confirmation, show cleanup progress, and
remove cached deal/document/chat state after acceptance without claiming completion early.
Offline clients and other tabs revalidate on access; server completion does not promise remote
erasure of bytes they already hold. Never revoke a local root needed by another deal.

## 8. Verification and acceptance

Add tests that exercise failure boundaries and ownership, not just generated SQL:

- SQL aggregate deletion: populated metadata/roles/scopes; several files and versions, old/soft-
  deleted files; regular/archived linked chats with branching messages. All children disappear
  atomically; foreign-key checks stay clean. Inject a transaction failure and verify rollback.
- Isolation: shared companies, same bytes attached to a second deal, another owner's files/chats,
  and freeform threads survive. Unmatched graph ownership blocks unsafe cleanup.
- Helix: current/historical/partial/orphaned nodes and embeddings disappear; unrelated nodes and
  global indexes remain. Delete replay is safe. Verify actual keyword/vector results and late
  write-fence behavior against an explicitly selected disposable Helix runtime.
- Backfill: repeated batches, missing properties, inconsistent identities, interrupted progress,
  and absent canonical mappings. No embedding re-generation or global graph reset.
- Races: delete versus upload SQLite commit, setup/chunk/publish, queued jobs, metadata extraction,
  streamed chat completion, Office preview, search response, update/archive, new linked thread,
  concurrent DELETE, and attempted deal-ID reuse. Use controlled barriers instead of timing sleeps.
- Recovery: crash after acceptance, manifest commit, fence creation, SQLite purge, each Helix batch,
  remote success before checkpoint, cache eviction, and completion. Recover after restart and
  outage; never lose an accepted operation or report success with remaining content.
- HTTP/access: malformed IDs, missing actor, non-owner, archived deal, repeated request, status
  lookup after deal deletion, safe errors, and root-fallback rejection. Both `/api/v1` and the
  existing compatibility mount behave intentionally; new clients use only `/api/v1`.
- Adapters/native: encoded IDs, `202` JSON, `204` without JSON parsing, polling errors, cancellation
  of local polling without cancelling deletion, and unchanged thread/template DELETE semantics.
- Client hooks: stale portfolio/chat/document requests cannot restore deleted state; cleanup is
  scoped to the correct deal and server/demo data remains distinct. A UI consumer, if added,
  also gets keyboard/focus, loading/error/retry, viewport, and theme inspection.

Include new backend test modules through the crate's existing `#[path]` inclusion pattern;
`autotests = false`. Extend architecture tests for any newly enforceable dependency boundary.

Run focused filters first, then these existing broad gates from their build roots:

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

# frontend/src-tauri/
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# repository root
git diff --check
git status --short
```

Extend the existing ignored Helix v3 compatibility suite, which already requires an explicitly
supplied disposable `QUARRY_HELIX_COMPAT_URL`. Do not point deletion tests at a normal store or
run all ignored tests against an unverified environment. Do not use routine application startup
as a test; it opens canonical storage and connects to Helix. Record and stop task-owned listeners.

The implementation is done when a completed operation leaves no deal-owned canonical records,
document projections, or active producers; pending operations survive restart and remain
inaccessible; unrelated/shared data survives; both client runtimes honor the same contract; and
the architecture and deletion/recovery record match tested behavior.

## 9. Architecture impact of this planning change

No implemented route, runtime boundary, schema, or behavior changes in this commit/task.
The architecture was rechecked against the code; no architecture edit is needed for the plan.
During implementation, update the canonical API table, state ownership, schema/migration policy,
Helix shape/backfill, cross-store recovery, jobs/caches, identity limitations, and deletion
operations sections. Record the new permanent-deletion and control-record retention decisions
in a scoped document under `docs/`; do not make this `plans/` document a current product contract.
