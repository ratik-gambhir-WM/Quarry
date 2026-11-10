# Deal archive and permanent deletion API plan

Date: 2026-10-15. Updated: 2026-11-10. Status: implemented API/workflow; visible Delete action remains a separate consumer task.
Implementation record (2026-11-10): the API, schema-18 ledger, worker, remote fences, web/desktop
transports and client invalidation are implemented. The sections below retain the reviewed design
baseline; [permanent deletion and recovery](../docs/deal-permanent-deletion.md) and
[Architecture](../docs/ARCHITECTURE.md) describe the current implementation. Canonical purge and
file/thread target capture commit together after the remote fence; file identities cover every
version/generation without copying content-bearing canonical manifests. Private notebook links
are detached, and their contents/jobs/projections survive. A visible action/confirmation screen
is outside this API increment. No reset or valuable local data operation was performed.

## 1. Outcome and recommended approach

Cover two distinct operations: retain and verify the existing lightweight archive API, and add
permanent deletion. Archive is a synchronous SQLite timestamp update; permanent deletion is the
durable cross-store cleanup workflow below. Archive can be verified and integrated independently
of deletion and does not require the deletion ledger or Helix backfill.

Add `DELETE /api/v1/deals/{dealId}` to permanently remove a deal and all Quarry-owned data that
belongs to it: metadata, company-role assignments, diligence scopes, stored documents and every
version/blob, linked chats and messages, and Helix document/search projections.

Use a **durable asynchronous deletion operation**. SQLite and Helix cannot commit one transaction
together, and ingestion/chat work can still be running. A successful DELETE acceptance must
create durable work, prevent further use of the deal, and return an operation to track. Only mark
that operation complete after physical application records and search projections are gone.
This is permanent deletion, with a temporary cleanup phase and no restore/archive substitute.

Reuse the existing `deal_id` on Helix file, version, and chunk nodes and the canonical document
identity checkpoints. Add a separate durable deletion-target manifest so cleanup remains possible
after canonical files disappear, including partial or inconsistent graph writes. Ownership transfer
must not change document IDs or graph scope.

Do not implement this as one SQL DELETE, an archive flag, a process-local background task alone,
or a global Helix clear. Do not delete shared company records or source files outside Quarry.

## 2. Findings that shape the implementation

| Current implementation | Design consequence |
| --- | --- |
| [Schema marker 17](../backend/src/app/migrations.rs) supports validated, data-preserving upgrades from markers 12–16. | Add the next unused marker, provisionally 18, without resetting either store; extend every supported upgrade path. |
| Deal metadata, parties, scopes, memberships, access revisions, teams, activity, and document children cascade. Linked assistant threads and notebooks use `ON DELETE RESTRICT`. | Delete all linked threads and detach personal notebook links explicitly before deleting the deal in one transaction. |
| [Membership policy](../backend/src/domains/memberships/model.rs) owns roles/capabilities. `deals.user_id` is immutable creator provenance. | Only the current owner membership may accept deletion, including after ownership transfer. Add a deletion capability; do not authorize by creator or uploader. |
| [Archive repository](../backend/src/domains/deals/repository.rs) rechecks owner access transactionally and returns existing archive state without changing timestamps. | Verify the existing authenticated API; add only deletion coordination and demonstrated coverage gaps. |
| [Graph nodes](../backend/src/domains/documents/graph/model.rs) already carry deal/file/version identities; ingestion preflights immutable graph requests before SQLite commits. | Preserve identities, request-size preflight, and canonical checkpoint verification. Add deletion fences to every executable setup/chunk/publication request. |
| [Graph repository](../backend/src/domains/documents/graph/repository.rs) owns document capabilities; templates and notebooks have separate graph owners. | Keep document purges here and preserve global indexes, private templates, and notebook projections. |
| [Helix client](../backend/src/adapters/helix/client.rs) serializes writer-only durable writes and treats SDK v3 write errors as terminal. | Local serialization cannot prove a remote late-write fence. Reconcile ambiguous deletion outcomes; do not add generic write retries. |
| [Search](../backend/src/domains/documents/search/service.rs) already scopes by accessible deals, filters canonical current identities, and reauthorizes final results. | Extend the existing access/filtering paths with deletion-barrier checks, including cached hybrid results; do not build a second search authorization system. |
| [Document jobs](../backend/src/domains/documents/ingestion/job_service.rs) remain process-local; authenticated job/stream access already exists. | Audit existing scope/cancellation tracking, then add real task drain and deletion-specific retained-result eviction. Use a separate durable ledger for cleanup. |
| [Chat service](../backend/src/domains/assistant/chat/service.rs) has persisted branching histories and provider streams. | Drain all linked runs across users, reject late callbacks, and delete archived as well as active threads. |
| [Notebook repository](../backend/src/domains/notebooks/repository.rs) stores private owner-scoped notebooks with optional deal links and durable save/cleanup jobs. | A link is context, not deal ownership of note contents. Preserve notebooks, notes, jobs, and graph data; clear matching links atomically and fence new/relinked associations after acceptance. |
| [Data Room service](../backend/src/domains/data_rooms/service.rs) can use a configured root when metadata supplies no path. Existing HTTP policy also restricts local-source access. | Retain that policy and filesystem containment; extend it with deletion liveness checks before fallback and before returning in-flight bytes. |
| Preview bytes and client annotations are cached in memory. | Evict server previews and expose scoped client invalidation; downloaded/offline copies cannot be recalled. |
| The [desktop DELETE relay](../frontend/src-tauri/src/quarry_api/client.rs) accepts only `204` and discards bodies. | Add a typed `202` JSON DELETE path while preserving existing delete callers. |
| [Bearer identity and resource access](../backend/src/domains/identity/access.rs) are implemented; organization tenancy remains incomplete. | Reuse verified identity, session revocation, and membership policy. No new identity framework or caller-email trust. |

The local Axum skill still describes schema marker 11 and older reset behavior. Live migration
code, the root guide, and architecture take precedence: marker 17 and validated additive upgrades
are current. Diligence stages, branching chat, notebooks, and shared projects are implemented;
treat their actual foreign keys and lifecycle contracts as dependencies, not future proposals.
Pencils Down, Closed, archive, and permanent deletion remain separate operations. Chat file
references are resolved through canonical authorization; do not treat them as a separate reusable
attachment store or infer ownership from a textual mention.

## 3. Deletion ownership and limits

| Data | Action |
| --- | --- |
| `deals`, `deal_metadata` including extracted questions/source links, `deal_parties`, `deal_scopes` | Permanently delete. |
| Every `quarry_files` row for the deal, all `quarry_file_versions`, and `quarry_file_blobs` | Permanently delete, including non-current versions and rows already marked deleted. |
| Every `assistant_threads` row with this exact `deal_id`, including other members’ private threads, archived threads, and all messages | Proposed policy: permanently delete through internal deal-scoped cleanup without exposing transcripts to the requesting owner. Do not detach into freeform chats. |
| `deal_memberships`, `deal_access_revisions`, teams/assignments, and `deal_activity_events` | Cascade with the deal; retain only the minimal deletion receipt outside that aggregate. |
| `quarry_current_content`, document identity/chunk manifests, and `document_image_recovery` | Cascade through the document aggregate; snapshot required non-content purge identities first. |
| Personal notebooks linked through `notebooks.deal_id` | Proposed policy: clear that exact link in the purge transaction; preserve private notebook/folder/note contents, save jobs, cleanup jobs, and notebook graph projections. |
| Helix `QuarryFile`, `FileVersion`, `FileChunk`, embeddings, text, and connecting edges for those files | Permanently delete all versions/generations and partial graph writes. |
| Running ingestion/extraction/chat/preview work and retained document-job events for this deal | Cancel or drain; purge retained results; reject late publication. |
| Server preview cache | Evict affected bytes. Its current key lacks deal identity; clearing the bounded cache is an acceptable initial implementation. |
| Connected client deal/document/chat caches, editor annotations, active streams, and route-local selections | Invalidate through explicit consumer hooks; ignore stale responses that would restore the deleted deal. |
| Reusable `companies`, users, other deals and their files/chats | Preserve, even when company roles or document content match the deleted deal. |
| Freeform chats or chats linked to a different deal | Preserve. A textual mention is not an ownership relationship. |
| Local/SharePoint source files, external SOW/fact-sheet/request-list URLs, exported/downloaded files | Preserve originals; remove Quarry's stored copies and references only. |
| Diligence Studio's app-wide templates | Preserve. There is no current durable deal-owned deliverable/template relationship to cascade. |
| `reminders.deal` | It is unused free text, not a deal foreign key. Do not delete by name matching. Add a real relationship before promising reminder cleanup. |

The notebook and cross-user chat policies above are proposed product contracts, not existing
deal-deletion behavior. Any future consumer must disclose their scope. A notebook link does not
authorize reading its contents, and the deletion worker needs no notebook content access.

Keep only minimal operational deletion receipts/fences after completion: resource IDs, accepting owner ID,
operation state, and timestamps. Delete per-file/thread manifests when no longer needed for
recovery. Never retain names, document/chat contents, links, or source paths in the ledger.
These receipts cannot reconstruct a deal and are distinct from the deleted `deal_metadata`.

Deletion means removal from active application stores and indexes. SQLite pages/WAL, backups,
provider-side retention, logs, and user-made copies require separate retention/erasure policies;
do not claim cryptographic or forensic erasure from a normal DELETE response. Do not run global
VACUUM/checkpoint/reset operations inside the request. Recovery must not replay pre-deletion
backups into a live system without reconciling the deletion ledger.

## 4. Proposed API contract

### Archive API: existing implementation and completion work

Reuse `POST /api/v1/deals/{dealId}/archive`; do not add another archive endpoint or overload DELETE.
The current [route](../backend/src/domains/deals/route.rs),
[handler](../backend/src/domains/deals/handler.rs),
[service](../backend/src/domains/deals/service.rs), and
[repository](../backend/src/domains/deals/repository.rs) already implement this operation.
`QuarryApi.archiveDeal(dealId): Promise<SavedDeal>` is exposed through both the
[HTTP adapter](../frontend/src/api/httpQuarryApi.ts) and
[Tauri adapter](../frontend/src/api/tauriQuarryApi.ts).

Preserve and verify this contract:

- The handler validates the encoded path identifier. Clients currently send an empty JSON object;
  there is no caller-controlled archive timestamp or engagement-status change in the request.
- Return `200 OK` with the updated `SavedDeal` in camelCase JSON, including `archivedAt` and
  `updatedAt`. Invalid identifiers return `400`; unauthenticated requests return `401`; missing or hidden deals
  return `404`, and visible non-owner requests follow the current permission-denied mapping. Storage failures use
  existing sanitized error mapping and must not be reported as success.
- Set `archived_at` and `updated_at` only when `archived_at IS NULL`, inside the existing SQLite
  transaction. Repeated/concurrent archive requests return the already archived deal without
  moving either timestamp. Commit before returning success; no background worker or polling.
- Preserve engagement `status`, metadata, parties, scopes, files/versions/blobs, linked chats,
  Helix projections, and external originals. Archive is portfolio visibility, not erasure.
- `GET /deals` excludes archived records; direct `GET /deals/{dealId}` remains available. Preserve
  current operation-specific archive checks, including rejection of new stored-document ingestion
  for archived deals. Do not infer a new blanket read-only, search-hiding, or job-cancellation
  policy from the archive timestamp; changing those behaviors requires an explicit contract.
- No archive schema migration, Helix update/backfill, deletion manifest, producer drain, or cache
  purge is needed. There is no deal-unarchive endpoint today; restoring/listing archives is outside
  this increment and must not be implied by an Archive action.

Completion work is to verify the existing path and close any demonstrated contract/test gaps,
not reimplement the timestamp write. If an archive consumer is wired as part of implementation,
call the existing runtime API, prevent duplicate submissions, and update/refetch the shared
workspace resource after success so lists, counts, and sidebar navigation omit the archived deal.
Preserve failure/retry feedback and ignore stale list responses that could reintroduce it.
Keep demo fixtures visibly separate. A visible Archive action is a separate consumer task unless
explicitly included; this plan does not require a new archive screen or UI redesign.

The archive handler already receives `AuthenticatedUser`; its repository uses the owner-only
transfer capability so repeated calls on an archived deal can return the existing state. Preserve
this behavior and the single `deal_archived` activity event. Recheck current membership ownership
and the deletion barrier in the archive transaction once deletion exists. If deletion was accepted first, return `409 deal_deletion_in_progress` while
pending, then `404` after completion; never resurrect a deleted deal. If archive commits first,
DELETE can still accept the archived deal. Serializing these SQLite checks is sufficient for
archive; do not give it the deletion worker's cross-store workflow.

### Permanent deletion requests and responses

`DELETE /api/v1/deals/{dealId}` has no request body. Use the existing identifier constraints and
URL encoding. Resolve the current owner membership from the existing authenticated actor; do not use a body/query `userEmail`
or `workspaceId` as proof of permission.

- `202 Accepted`: durable deletion is registered or still pending. Return JSON and a `Location`
  header pointing to `/api/v1/deal-deletions/{deletionId}`.
- `204 No Content`: a retained receipt proves this actor's same deal was already fully deleted.
- `400`: invalid identifier. `401`: no trusted actor. `404`: absent/hidden resource or receipt. A visible non-owner receives the existing permission-denied
  status (`403`); receipt lookup remains accepting-actor-only and hides others’ receipts with `404`.
  `503`: deletion could not be durably registered. Preserve sanitized error mapping for other failures.
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

### Identity and access integration

Reuse the existing HTTP bearer authentication boundary and `AuthenticatedUser`. Add an owner-only
`Delete` capability that permits deletion of archived deals. Validate identifiers and prove initial
access through the membership policy before product work. Repeat current-owner and deletion-barrier
checks in the acceptance transaction because ownership may change after HTTP preflight. Neither
`deals.user_id`, uploader attribution, nor a supplied `X-User-Id` grants deletion permission.

Freeze membership changes, ownership transfer, and new notebook links after acceptance. Bind the
receipt to the verified accepting actor so repeat DELETE and status polling remain authorized after
memberships and the deal row are gone. Polling must still require a current valid session; a receipt
is not a bearer credential. Other former members do not gain operation access from knowing its ID.
For repeat DELETE, resolve an existing accepting-actor receipt before requiring a live deal
membership; otherwise retries would fail after the deal is purged. Apply the same receipt policy
at the HTTP boundary and in the transaction.
Normal access for every member is denied while cleanup runs. Keep owner-only notebook privacy and
existing session/stream revocation checks. Organization tenancy is outside this increment.

## 5. Preserve implemented graph scope and add deletion safety

1. Reuse current `deal_id`, `file_id`, `version_id`, generation, and deterministic chunk identities.
   Document IDs are deal-scoped; migrated identities remain governed by canonical checkpoints.
   No workspace property is required in current graph nodes. Never delete by creator, uploader,
   content hash, or display name; identical bytes may belong to another deal.
2. Extend `PreparedDocumentPersistence`/`PreparedDocumentGraph` so retained setup, every chunk
   batch, and publication atomically reject the durable remote deal fence. Preserve agreement with
   `PersistedFileIdentity`, canonical manifests, and accepted image checkpoints. Include all added
   parameters and fence clauses in exact serialized-size preflight before SQLite commit.
3. Reuse existing deal-scoped lookup/search queries and index-readiness checks. Verify scoped purge
   predicates against the captured canonical file set. Add an index only if required by the purge
   implementation, with readiness and upgrade tests; DDL acceptance alone is insufficient.
4. Extend membership/access filtering with the deletion barrier in candidate preparation, current
   identity refresh, and final result authorization, including cached hybrid work. Keep existing
   scoped and global search contracts, bounds, and per-deal authorization; no new search DTO is
   required just to introduce deletion.
5. Require the existing completed document identity/graph-scope migration at startup. Do not repeat
   the removed one-time migration or invent a backfill/reset for normal current stores. Audit partial,
   unpublished, or inconsistent projections using exact file/version/generation manifests. Missing
   or mismatched canonical ownership must suppress results and block unsafe cleanup, never trigger
   a guessed assignment or deletion of another deal’s nodes.
6. Prove remote fence ordering and bounded scoped purge in an explicitly disposable Helix instance
   before enabling DELETE. The application mutex alone does not prove either guarantee.

Use deal predicates plus exact deletion-manifest file identities to reach unpublished/partial
records. Verify candidates against the canonical target set before removing them. Detect and
report unmatched/misowned records instead of reporting completion. Preserve notebook and template
namespaces and global indexes.

## 6. Durable workflow and concurrency

### Persistence and module ownership

Add `deal_deletions` with a unique deal ID, owner, operation UUID, state/phase, attempts,
retry scheduling, timestamps, and a sanitized error code. It must survive deletion of `deals`;
do not give it a cascading foreign key to that row. Add bounded per-operation target rows for
deal/file/version/generation identities, exact chunk IDs where needed, and linked thread IDs,
with no document or chat content. These are independent snapshots, not foreign keys that cascade
with canonical files or threads.

Add a focused `domains/deals/deletion/` service/repository module, a narrow lifecycle gate exported
by the deals domain for its consumers, and collaborator interfaces for documents/chat/cache cleanup. The deletion service
owns orchestration; its repository owns the cross-table SQLite transaction. Helix deletion stays
inside `documents/graph/` through `DocumentGraphWriter` and focused query builders, with verified
reads through the graph reader/search boundaries. Bootstrap supplies all collaborators and runs the
worker; route state contains service handles, not repositories/configuration. Keep SQL in the
existing bound query builder/client and blocking work in its established async offload boundary.

### Execution sequence

1. **Accept and fence.** In one SQLite transaction, verify actor/owner, return an existing receipt
   when applicable, and insert the operation. The committed operation is the persistent deletion
   barrier; all normal reads/writes consult it. Signal the worker after commit and return `202`.
   If signalling is lost, startup/periodic ledger scanning still discovers the work.
2. **Stop producers.** Close admission for deal mutations, uploads, metadata extraction, chat
   creation/runs, membership/team changes, notebook create/relink operations, source previews,
   and graph publication. Cancel queued/provider work where safe;
   wait for already issued side effects and blocking conversions to settle. Track jobs by deal and
   their real task lifetime, not just the request/stream lifetime. Do not hold a SQL transaction
   while waiting. Never assume dropping a future stops `spawn_blocking` or a remote write.
   Include ingestion that already passed graph preflight and is waiting on SQLite or the shared
   Helix write mutex. Existing prepared requests are executable data, not evidence that the deal
   is still live. Hold admission through setup, every chunk batch, and final publication.
3. **Drain and snapshot.** Acquire the exclusive per-deal lifecycle gate. Operations admitted
   earlier must hold a shared guard through persistence/publication, including spawned workers.
   Within a short SQLite transaction, recheck ownership/barrier, capture every file and linked
   thread across all actors, and persist the complete manifest before deleting canonical data.
   Snapshot all files regardless of `deleted_at`, and include every version when checking counts/invariants.
4. **Block late Helix writes.** Establish an idempotent, minimal Helix deal-deletion fence. Setup,
   chunk insertion, and publication must atomically refuse a fenced deal in their
   server-side write transaction. Keep this control record separate from file/document nodes.
   Verify the pinned SDK/server can enforce the condition and ordering; application-only checks
   cannot prevent an already-sent remote request from arriving late. If this guarantee cannot be
   proven, leave the operation blocked rather than claim completion; resolving that mechanism
   is an implementation prerequisite. Never blindly retry a non-idempotent ingestion write.
5. **Purge SQLite.** With producers drained and the manifest/fence durable, delete linked threads
   by exact `deal_id`, clear matching `notebooks.deal_id` links, and then delete the deal in one
   transaction. Link detachment is an internal referential operation, not permission to read or
   edit private note content. Serialize it with notebook create/relink operations and make those
   transactions reject a deleting deal; a save-job checkpoint must not restore a stale link.
   Messages cascade from threads; metadata/parties/scopes, membership/team/activity rows, and files/versions/blobs plus
   content claims/identity/image checkpoints cascade from the deal. Commit the phase
   checkpoint atomically with that deletion. Keep foreign keys enabled. Verify parent-message
   constraints using branching chat histories; diagnose inconsistent cross-thread references
   rather than deleting unrelated threads to bypass a constraint.
6. **Purge Helix.** In bounded, repeatable writes, delete chunks, versions, files, and their edges
   by validated deal ownership and manifest file identities. Property predicates must
   reach orphaned/unpublished versions/chunks without traversing only CURRENT_VERSION edges.
   Treat already-absent records as success, preserve global indexes, and respect the existing
   2 MiB request cap. Checkpoint batches so a large deal does not require one request-sized purge.
7. **Evict and verify.** Purge retained job events/results and server previews; terminate linked
   streams. Verify no canonical deal children, notebook deal links, document graph nodes/edges,
   keyword/vector hits, or active deal producers remain. Preserved personal notebook save workers
   may continue without reattaching the deleted deal. Verify both deal predicates and manifest identities. Filter every search
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
  Keep the existing adapter's terminal-error behavior for ordinary writes. The deletion worker
  owns bounded retry of idempotent scoped purge/fence operations after reconciliation, rather
  than guessing HTTP status/retryability from the SDK's sanitized string errors. Preserve
  writer-only/durability flags and keep remote fence proof separate from local mutex ordering.
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
| 0. Archive verification/integration | Existing archive route/service/repository, `QuarryApi.archiveDeal`, HTTP/Tauri adapters, workspace resource invalidation if a consumer is included. | Idempotent timestamps, retained data, list omission/direct lookup, safe failures, and existing owner authorization and future deletion-barrier integration are verified; no archive migration or Helix worker. |
| 1. Contracts and proof | Deletion DTOs/access policy; disposable Helix tests for scoped drops, edge/index cleanup, and atomic late-write fences; existing timeout/error mapping. | Accepted/completed semantics, existing authenticated-owner policy integration, and the remote fence mechanism are explicit and testable. |
| 2. Additive persistence | [migrations](../backend/src/app/migrations.rs), [SQLite query/client](../backend/src/adapters/sqlite/client.rs), deletion repository, recovery tests. | Ledger/barrier/manifest survive rollback and restart without discarding existing application records. |
| 3. Helix ownership | [models](../backend/src/domains/documents/graph/model.rs), [writer](../backend/src/domains/documents/graph/writer.rs), [queries](../backend/src/domains/documents/graph/query.rs), [repository](../backend/src/domains/documents/graph/repository.rs), immutable ingestion identity/preflight, existing identity checkpoints and scoped cleanup. | Prepared and committed identities agree on deal ownership; existing deal identity/checkpoints remain valid; deletion-aware filters, any required DDL readiness, and fences work. |
| 4. Runtime safety | Ingestion [service](../backend/src/domains/documents/ingestion/service.rs)/jobs, [chat service](../backend/src/domains/assistant/chat/service.rs)/repository, [Data Room](../backend/src/domains/data_rooms/service.rs), document viewing/search, metadata extraction, memberships and notebook-link transactions. | In-flight work cannot recreate or return deleted data; missing/deleting deals cannot resolve source-root fallback. |
| 5. Orchestration/API | Deletion module, [deal handlers/routes](../backend/src/domains/deals/route.rs), [bootstrap](../backend/src/app/bootstrap.rs), service errors and operation lookup. | DELETE is idempotent, status is owner-scoped, and restart recovery converges across stores. |
| 6. Shared clients | [QuarryApi](../frontend/src/contracts/quarryApi.ts), HTTP/Tauri adapters, native JSON DELETE transport, runtime.desktop, adapter/native tests. | Both distributions can receive acceptance, poll status, and distinguish pending cleanup from completion. |
| 7. Consumer lifecycle/docs | Workspace removal/invalidation, chat/document cache hooks, architecture and a scoped deletion/recovery design record. | Stale requests cannot repopulate a deleted deal, and operational limits are documented. |

For phase 2, start from **schema marker 17**; reserve the next unused marker (provisionally 18)
only after reconfirming concurrent migrations. Add the ledger/targets and any required constraints
without dropping canonical data. Preserve fresh-schema validation and all supported validated
12/13/14/15/16/17 upgrade paths, including notebooks, membership/file scope, image recovery,
classification leases, and private template ownership. Test each path transactionally and retain
unsupported/damaged-schema rejection. No archive migration or general graph backfill is needed.
Never run `reset_development`, `clear_helix`, or edit local databases for this implementation.

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

- Archive: first/repeated/concurrent requests, stable archive/audit timestamps, invalid/missing IDs,
  transaction rollback, list omission and direct lookup, unchanged engagement status, and retained
  metadata/parties/scopes/files/versions/blobs/chats. Extend the existing
  [deal HTTP tests](../backend/tests/integration/deal_company_http_tests.rs),
  [HTTP suite](../backend/tests/integration/http_tests.rs), and
  [migration retention tests](../backend/tests/integration/app/migrations_tests.rs) rather than
  claiming there is no coverage. Verify encoded IDs, `200` JSON mapping, and errors in both client
  adapters. Retain authenticated owner/non-owner coverage and add archive/delete ordering when
  deletion lands.
  If a consumer is added, verify shared list/count/sidebar invalidation, retained failure state,
  and stale-response handling. No live Helix/provider integration is needed for archive-only work.
- SQL aggregate deletion: populated metadata/roles/scopes; several files and versions, old/soft-
  deleted files; content/identity/image checkpoints; membership/team/activity rows; regular/archived
  linked chats across users with branching messages; private notebooks with accepted save jobs.
  Deal children disappear and notebook links clear while private contents/jobs/projections survive
  atomically; foreign-key checks stay clean. Inject a transaction failure and verify rollback.
- Isolation: shared companies, same bytes attached to a second deal, other deals’ files/chats, private notebook contents,
  private templates, and freeform threads survive. Linked chats from other members are deleted
  without exposing them to the accepting owner. Unmatched graph ownership blocks unsafe cleanup.
- Helix: current/historical/partial/orphaned nodes and embeddings disappear; unrelated nodes and
  global indexes remain. Delete replay is safe. Verify actual keyword/vector results and late
  write-fence behavior against an explicitly selected disposable Helix runtime.
- Graph-scope readiness: existing migration checkpoints, inconsistent identities, missing
  canonical mappings, and partial graph writes. No embedding regeneration or global graph reset.
- Races: delete versus upload SQLite commit, setup/chunk/publish, queued jobs, metadata extraction,
  streamed chat completion, Office preview, search response, update/archive, new linked thread,
  membership removal/transfer, notebook create/relink/save, concurrent DELETE, and attempted deal-ID
  reuse. Use controlled barriers instead of timing sleeps.
  Explicitly pause an immutable prepared graph before SQLite commit, after commit, and while
  queued behind the Helix write lock. Prove none can publish after the deletion fence. Extend
  [ingestion persistence tests](../backend/tests/integration/domains/documents/ingestion/persistence_tests.rs)
  and the current graph writer tests; preserve exact request-size boundary coverage after adding
  ownership/fence data. Test DDL readiness through graph repository/compatibility coverage, and
  terminal transport errors in the
  [Helix client suite](../backend/tests/integration/adapters/helix/client_tests.rs).
- Recovery: crash after acceptance, manifest commit, fence creation, SQLite purge, each Helix batch,
  remote success before checkpoint, cache eviction, and completion. Recover after restart and
  outage; never lose an accepted operation or report success with remaining content.
- HTTP/access: malformed IDs, missing/revoked actor, each non-owner role, ownership transfer,
  archived deal, repeated request, accepting-actor receipt privacy, status
  lookup after deal deletion, safe errors, and root-fallback rejection. Both `/api/v1` and the
  existing compatibility mount behave intentionally; new clients use only `/api/v1`.
- Adapters/native: encoded IDs, `202` JSON, `204` without JSON parsing, polling errors, cancellation
  of local polling without cancelling deletion, and unchanged thread/template DELETE semantics.
- Client hooks: stale portfolio/chat/document requests cannot restore deleted state; cleanup is
  scoped to the correct deal and server/demo data remains distinct. Notebook catalogs must
  revalidate detached links without discarding private edit buffers or exposing them to other users. A UI consumer, if added,
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
The current live suite covers DDL readiness, graph writes/reads/replacement, and scoped search;
it does not yet prove deal purge, orphan/edge/vector cleanup, or atomic remote fences. Add those
proofs explicitly before enabling permanent deletion. Keep graph tests under the new `graph/`
paths; the removed `index/` paths are not implementation targets.

The archive increment is done when its existing API contract and retained-data behavior are
verified across both client adapters, with any included consumer invalidating portfolio state,
and its authorization/deletion coordination requirements are explicit.

The deletion implementation is done when a completed operation leaves no deal-owned canonical records,
document projections, or active producers; pending operations survive restart and remain
inaccessible; unrelated/shared data survives; both client runtimes honor the same contract; and
the architecture and deletion/recovery record match tested behavior.

## 9. Implementation architecture impact

`docs/ARCHITECTURE.md` now describes schema 18, the deletion routes and worker, the remote control
namespace, single-writer enforcement, private notebook detachment, and client invalidation.
[Permanent deletion and recovery](../docs/deal-permanent-deletion.md) records the API, retained data,
batching, rollout, failure/retry, backup and rollback contracts. The root agent guide's current
schema references were advanced to 18. No feature-specific local skill copies were introduced.

Verification uses the gates in section 8, including explicitly disposable Helix tests for prepared
late writes, current/historical graph/search cleanup and restart recovery. A visible Delete action
remains a separate consumer increment. Accepted deletion never promises erasure of existing
provider, backup, exported or offline copies.
