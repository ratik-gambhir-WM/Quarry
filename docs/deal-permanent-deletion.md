# Permanent deal deletion and recovery

Current storage is fresh PostgreSQL schema 1 with retained Helix. See
[PostgreSQL storage and Helix](postgresql-storage.md) for setup, tests and recovery; former
SQLite marker upgrades are historical and are not executed by this runtime.

Implemented 2026-11-10. Archive remains a separate, synchronous timestamp operation.

## API and permissions

`DELETE /api/v1/deals/{dealId}` accepts no body. Only the current membership owner may
accept deletion, including for archived deals. Creator provenance and uploader attribution
provide no permission. HTTP authentication verifies the bearer session and matching user header;
the acceptance transaction repeats owner authorization. Unknown/hidden resources return 404,
visible non-owners 403, malformed identifiers 400, and missing/revoked sessions 401.

Acceptance commits one PostgreSQL receipt and returns 202 with camelCase `DealDeletion` JSON,
`Location` and `statusUrl`. The committed receipt is the deletion barrier. Repeating DELETE
returns the same operation; a completed receipt returns 204 without a body. Failed durable
registration returns 503 without committing a barrier. Repeating DELETE can wake a blocked or
retrying operation. There is no undo after acceptance.

`GET /api/v1/deal-deletions/{deletionId}` returns the operation to the authenticated accepting
actor, even after the deal and memberships are gone. Others receive 404. Receipt IDs do not
replace authentication. Both endpoints send `Cache-Control: no-store`. The compatibility `/api`
mount remains; clients use `/api/v1`.

States are `pending`, `running`, `retrying`, `blocked`, and `completed`. Phases are `draining`,
`fencing`, `sqlite`, `graph`, `verifying`, and `completed`; checkpoints can skip transient phases.
Responses contain IDs, state/phase, retryability, a safe error code, timestamps and the status URL.
They do not contain names, paths, transcripts or provider errors. Normal deal reads disappear
with 404 after acceptance; deal-path writes by current members fail with `deal_deletion_in_progress` (409).
Unknown actors remain hidden. Global lists/search omit the deal.

## Ownership and retained data

Deletion removes metadata/links, parties/scopes, memberships/access revisions, teams/assignments,
activity, all files and immutable versions/blobs, current-content claims, document identity/image
checkpoints, and document graph projections. Every thread with the exact deal association is
removed, including archived threads and private threads belonging to other members. Cleanup does
not expose those transcripts to the requesting owner. Freeform and other-deal chats survive.

Personal notebooks are private resources. The transaction clears only matching `notebooks.deal_id`
links; notebooks, folders, notes, edit buffers, save jobs, cleanup jobs and notebook graph projections
survive. New/relinked associations repeat live membership checks transactionally. Company records,
users, unrelated deals, private/app-wide templates, external source files and exported copies survive.
Free-text reminder names and textual mentions do not establish deletion ownership.

A completed operation retains only its receipt and a non-content remote fence. Temporary file and
thread manifests are removed. IDs remain reserved permanently; creating another deal must use a
new ID. Deletion removes active application records, not PostgreSQL free pages/WAL, backups, provider
retention or copies already held by other clients. No VACUUM, reset or global graph clear runs.

## Storage and worker lifecycle

PostgreSQL schema 1 includes `deal_deletions`, `deal_deletion_files` and `deal_deletion_threads`. The ledger has
no cascading relationship to `deals`. Target rows reference the receipt, not the removed files or
threads. Validated schemas 12–17 upgrade through their existing steps and the additive ledger DDL;
fresh/current layouts, foreign keys and owner invariants are validated. Damaged layouts fail safely.

`domains/deals/deletion` owns acceptance, orchestration and canonical transactions. Membership
policy applies the barrier, document search extends existing canonical/final-result filtering,
and the document graph owner supplies remote fencing/purge capabilities. Scheduling lives in
`deletion/worker.rs`, separate from acceptance and cleanup phases in `service.rs`. Acceptance
commits and notifies in a short-lived Tokio task, preserving the wake-up even if its HTTP caller
disconnects while storage is committing. A notification spawns or wakes one on-demand coordinator;
bootstrap requests a recovery scan for receipts left by the previous process. The coordinator
retires when no pending/running/retrying receipts remain. Blocked and completed receipts do not
keep an idle task alive. While unfinished work remains, it scans due operations in pages of 16,
wakes on acceptance or once per second, and spawns at most four short-lived cleanup tasks at once.
The shared task-lifetime helper makes notification and empty-queue retirement race-safe.
A busy deal is skipped without changing its
receipt and retried on the next scan; keyset pagination continues past busy pages so later deals
can progress. Every operation retains exclusive admission throughout its cleanup sweep.
The shared Helix client still serializes remote writes.
Barriers already apply to recovered operations before any route can serve them. Other deals remain
usable while cleanup resumes. Canonical PostgreSQL work uses the shared asynchronous pool; Office, filesystem and CPU-heavy
conversion keep their separate blocking-task boundaries.

Admission guards cover HTTP response bodies and actual spawned ingestion/chat tasks. Ingestion
workers recheck upload authorization after admission; job registration is also guarded. Chat
streams stop on deletion, check liveness before emitting events and reject late persistence.
Metadata/preview work drains before purge; dropped requests cannot publish a final result.
Cloneable admission tokens remain inside blocking preview/text tasks after HTTP cancellation.
Chat revocation runs independently of stream backpressure after encoding settles. If membership
loss leaves a private thread intact, cancellation persists the partial response and a terminal
`cancelled` status before releasing admission. The same transaction barrier rejects this final
write when deletion has been accepted; cancellation never recreates a removed thread. No
PostgreSQL transaction is held while waiting for tasks. A stalled producer delays that deal's completion; it does
not justify purging underneath an active task. Preserved personal notebook workers may continue.

The worker establishes the remote fence before deleting canonical data. With producers drained,
one PostgreSQL transaction snapshots every file/thread ID in bounded pages, detaches notebook links,
deletes threads and the deal, and advances to graph cleanup. File IDs identify all versions,
generations and partial graph nodes, so targets need no content or duplicated version manifests.
Capture and canonical purge commit together: a crash leaves either the original aggregate or a
durable target snapshot, never neither.

Each graph sweep examines/removes at most 128 nodes per label for each of up to 64 pending file
targets. Ownership mismatch blocks unsafe removal. A file is marked purged only after an empty
sweep for chunks, versions and files. Completion additionally checks for unmatched deal-scoped
nodes, canonical children and residual notebook links, then removes target manifests atomically
with completion. Other graph namespaces and global indexes are preserved. Server preview bytes and
retained document-job events are evicted. Search uses canonical identity and deletion checks
throughout the workflow, including cached hybrid results.

## Remote fence and deployment constraint

`QuarryDealLifecycle` is a separate Helix control namespace with a unique deal-ID index. Document
writes lazily create a live control record; deletion creates or monotonically marks it deleted.
Every setup, chunk batch and publication evaluates admission inside the same remote write batch,
including requests prepared earlier. Original conditional entries retain their conditions and
also require a live control record. The writer checks the returned fence acknowledgement and
includes the additional request data in its existing exact 2 MiB preflight. The pinned Helix v3
runtime's scalar/null behavior for `limit(1)` is avoided in vector-shaped verification queries.

This remains a single active Axum writer implementation. Bootstrap takes a PostgreSQL session
advisory lease scoped to the selected schema. Router and detached-worker client clones retain the
dedicated connection; other application writers are rejected. Operations fail closed if the lease
session is lost. Aggregate mutations additionally use a transaction advisory lock, row constraints,
revision checks and the retained worker/remote fences. Direct graph writers or different databases
are outside this coordination boundary. Offline maintenance must drain the application first.

The PostgreSQL deployment starts with fresh canonical and graph stores and leaves previous SQLite
stores untouched. No backfill, old-data conversion or destructive reset runs during startup.

## Failure, restart and rollback

A remote failure is ambiguous until reconciled. Ordinary SDK write failures remain terminal;
there is no generic write retry. Only the worker's idempotent fence/purge phases retry, with a
durable next-attempt time and capped exponential backoff. An outage before the fence leaves
canonical data retained but inaccessible. An outage after canonical purge leaves target manifests
for recovery. Restart resumes the checkpoint; remote success before a lost local checkpoint can
be repeated safely. Integrity/ownership failures become blocked and retain the barrier. Safe
error codes and operation IDs identify failures; cleanup never restores deleted blobs.

Repair an underlying availability/integrity problem through reviewed maintenance, then repeat
DELETE as the accepting actor to wake the same operation. Do not edit ledger state to manufacture
completion. There is no automatic expiry for unresolved operations. Do not delete receipts or
remote fences to release reserved IDs.

Rollback requires a drained window, a matching pre-upgrade database/binary and matching graph
backup. Never restore pre-deletion data into active service without reconciling later deletion
receipts/fences. Mixed-age restores are unsupported; there is no general restore/reindex utility.
Preserve backups securely under a separate retention policy. Never use `clear_helix` as deletion recovery.

## Clients and verification

The shared API exposes `deleteDeal` (receipt or null for completed 204) and `getDealDeletion`.
The desktop uses a separate `quarry_api_delete_json` command with exact deal-path/origin/auth
validation; existing void DELETE callers remain 204-only. The HTTP adapter checks expected statuses.
`waitForDealDeletion` polls until completed/blocked; local cancellation never cancels durable work.
Session changes discard stale acceptance/polling results.

Acceptance publishes a session-scoped invalidation event. Workspace lists/direct caches, document
previews and raw text, Data Room contents and chat state suppress late responses; active linked
chat streams stop. Notebook catalogs clear the link while retaining private drafts. Other tabs,
other members and offline clients revalidate on access; the server does not erase their copies.
The shared Deals table exposes a three-dot menu beside server-backed project names for owners.
Delete deal opens a confirmation describing removed and retained data. Submission requires the
exact, case-sensitive phrase `delete {dealName}`; whitespace is not normalized. Cancelling sends no
request, and pending acceptance prevents duplicate submission or dismissal. Authorization remains
server-enforced even if membership changes while the dialog is open.

The page owns the dialog independently of table rows. Acceptance removes the deal through the
existing invalidation event while the dialog polls the receipt until completed or blocked. A
polling failure offers Check status, which repeats only GET, never DELETE. Blocked cleanup shows
the operation reference for administrator attention. Closing after acceptance stops local polling
without cancelling durable work; session changes and unmount discard late results. Focus returns
to the row menu, or the remaining portfolio content if the row has gone. Demo data has no delete
action. The same UI and runtime contract serve web and desktop; no transport changes are needed.

Use the repository backend/frontend/desktop verification gates. The ignored deletion tests require
an explicitly disposable `QUARRY_HELIX_COMPAT_URL` and verify prepared late-write fencing, scoped
current/historical graph/search cleanup, and restart after acceptance/canonical purge. Run only
the selected deletion tests against that disposable store; never run all ignored suites against
an unverified environment. Test fixtures use isolated PostgreSQL test schemas and synthetic data.

### Verification record — 2026-11-10

| Build root | Commands and outcome |
| --- | --- |
| `backend/` | `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings` passed. `cargo test --locked --all-targets`: 546 passed, 13 opt-in tests ignored. |
| Disposable Helix | `QUARRY_HELIX_COMPAT_URL=<isolated-loopback-url> cargo test --locked --lib deletion_ -- --ignored --test-threads=1`: all three selected tests passed. The task-owned container was removed afterward. |
| `frontend/` | `npm run typecheck`, `npm run check:boundaries`, `npm test` (611 tests), `npm run build:web`, `npm run check:web-bundle`, and `npm run build:desktop-ui` passed. Builds emit large-chunk warnings. |
| `frontend/src-tauri/` | `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets` passed (25 tests). |
| Repository | Local documentation links checked; final scope review and `git diff --check` clean. No dependency/lockfile changes or generated outputs retained. |

Review covered ownership/transaction boundaries, recovery and task lifetimes, session/stale-result
handling, existing adapter compatibility, bounded requests/batches and cache invalidation. The
review fixed admission retention in detached blocking tasks, stream backpressure cancellation,
writer-lease retention in detached database workers, and deal predicates on graph removal.
No unresolved review findings or newly unused helpers remain. Accessibility/visual changes are
not applicable to new controls because this increment adds none; live keyboard/theme/viewport
inspection was not performed. Client state behavior was exercised through rendered-hook and
adapter tests. Full desktop packaging, live providers and unrelated opt-in integration suites
were not run; no production store or source folder was used for deletion testing.

Fixed review findings:

| Evidence | User impact and remedy | Disposition |
| --- | --- | --- |
| [Chat run](../backend/src/domains/assistant/chat/service.rs#L426) | A blocked stream send could delay cleanup indefinitely; revocation now runs outside the send/provider future. | Fixed; full backend gate passed. |
| [Blocking preview](../backend/src/domains/documents/viewing/service.rs#L178) | HTTP cancellation could release admission before conversion finished; the blocking task now retains a cloned token. | Fixed; cancellation/lifetime test passed. |
| [PostgreSQL client](../backend/src/adapters/postgres/client.rs#L218) | Router shutdown could release the writer lock before detached workers finished; client clones now retain the lease. | Fixed; detached-worker lease test passed. |
| [Graph purge](../backend/src/domains/documents/graph/deletion.rs#L167) | Cleanup must never remove a mismatched owner's node; removal requires both captured file identity and deal identity. | Fixed; disposable ownership-isolation and purge tests passed. |
