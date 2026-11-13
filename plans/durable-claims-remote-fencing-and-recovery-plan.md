# Concurrent writers continuation: durable claims, remote fencing and recovery

Date: 2026-11-12. Status: proposed implementation for a separate follow-up PR.
This replaces the earlier optimistic-concurrency refactoring plan. It continues the
schema/revision/resource-coordination work currently in the working tree; it does not
claim that work is committed or merged. Start the follow-up branch from that PR's merged
result, inspect the actual code and rerun the inventory before implementation.

This file is a design input. [Architecture](../docs/ARCHITECTURE.md) describes the current
product; [storage verification](../docs/postgresql-storage-verification.md) records the
completed checks and their limits. Neither multiple application writers nor distributed
worker safety is enabled today.

## 1. Completed foundation to preserve

| Area | Implemented behavior |
| --- | --- |
| Independent reads | Single-query reads use ordinary pooled connections; aggregate reads use REPEATABLE READ, READ ONLY. Neither checks writer-session health, ownership or generation, including snapshot commit. Current authorization and session checks remain. |
| Query primitives | Typed FOR SHARE/FOR UPDATE, SKIP LOCKED, mutation RETURNING and database-clock deadlines; read runners reject locking queries/mutations. Pool, query and lock waits remain bounded. |
| Fresh schema 2 | Bounded deal-content, separate metadata, company and template revisions; persistent deal lifecycle guards; dedicated startup-only schema advisory lock. Unknown, drifted and version-1 layouts fail without deletion. |
| Content conflicts | Expected revisions cross Axum, web, desktop adapters and native validation. Authorized stale replacements/deletes return conflicts; affected-row checks protect children/activity. Values stay positive and JavaScript-safe; exhaustion never wraps. |
| Deal authority | Protected mutations take shared lifecycle locks; membership/ownership/archive/deletion changes take exclusive locks. Deletion acceptance checks the displayed deal revision, retains authorized receipt replay and permanently reserves purged deal IDs. |
| Canonical parents | Notebook parent/folder coordination and final note CAS; assistant thread locking for sequence allocation; template CAS and parent coordination for refresh/classification SQL/delete. Ordinary notebook status/filter reads remain lock-free. |
| Client drafts | Submitted revisions remain tied to loaded content; conflicting drafts are kept, duplicate submission is guarded, and destructive conflicts require review. Notebook save retains 202 admission and asynchronous job conflict behavior. |
| Lexical search | Removed the corpus statistics singleton/trigger. Exact snapshot corpus aggregates retain indexed Unicode 6.1 frequencies, FTS5-compatible weighted BM25, owner/app filters and stable ties. |

The user authorized recreating the fresh SQL source without retaining old data. This
superseded the former version-1 data-preserving migration requirement. Startup still
never resets a database. No development database, real `.env`, old SQLite store or
existing Helix store was changed by verification.

Recorded foundation checks: 536 backend tests passed, 14 opt-in tests ignored; 652
frontend tests passed across 99 files; 26 desktop Rust tests passed. Formatting, checks,
Clippy, both frontend type targets, boundaries, both UI builds and their bundle gates
passed. The disposable notebook browser check covered saves and deletion cancellation/
focus restoration. Independent-pool tests exercised resource races with the runtime
advisory function disabled only inside their isolated test schemas. These are historical
results to rerun after implementation, not proof of distributed recovery or remote fencing.

### Still present; do not mistake for completed safeguards

- Bootstrap retains the application writer session, global generation commit fence and
  schema-wide runtime mutation advisory lock. Independent production writes still serialize.
- Document job status/watch state and ingestion coalescing remain process-local.
- Durable notebook jobs, template stages and deletion receipts exist, but their admission,
  claim, checkpoint, cleanup and recovery protocols are not yet safe across instances.
- Template classification parent locks protect short SQL stages, not the lifetime of a
  provider attempt. The facade/worker mutation mutex and Helix client mutex remain local.
- App registration updates supplied fields only and builtin refresh locks its template;
  new-app versus builtin attachment/reconciliation still needs a concurrency protocol.
- Remote generation/deletion enforcement, replica-wide quotas, real two-process recovery
  and comparative latency/throughput/queue measurements remain unproven.

## 2. Follow-up outcome and stable contracts

Complete durable work ownership, remote publication/delete fencing and multi-process
recovery, then remove global writer ownership and runtime serialization. Independent
resources must commit concurrently; competing changes to one resource must retain their
invariants. Do not replace the old controls with a universal row, table-wide writer,
application mutex or SQL lock held around network work.

Preserve PostgreSQL canonical ownership, immutable file versions, accepted image text,
exact-content identity reuse, Helix projection separation, membership/session revocation,
atomic activity, notebook submitted snapshots and operation IDs, bounded work, cancellation
rollback and the existing revision contracts. Keep `/api/v1`, camelCase JSON, multipart
limits, SSE event names, `sqliteCommitted` and deletion phase `sqlite`. Web and desktop
remain aligned; Tauri remains a transport/native boundary.

No provider, Helix, Office, preview or filesystem conversion call may run inside a SQL
transaction. No read replicas, multi-primary database, message broker, collaborative
merging, profile API, new template edit UI or SharePoint integration is required.

## 3. Shared claim and publication rules

### 3.1 Durable admission and attempts

1. Persist immutable accepted input, or a durable authorized blob reference, before
   acknowledging admission. Store stable operation identity and payload hash; a replay
   returns the original operation, while a mismatched payload conflicts.
2. Claim an eligible job atomically with FOR UPDATE SKIP LOCKED in a short transaction.
   Set a fresh attempt token, processing state and expiry before commit. Use database
   time for eligibility, expiry, renewal, retry and retention.
3. Separate content revision, event sequence, attempt token and monotonic projection
   generation. A random token proves attempt identity, not ordering of remote versions.
4. Every renew/checkpoint/persist/failure/completion mutation includes job ID, current
   token, expected stage/status and unexpired lease in its final predicate; check affected
   rows. A prior SELECT or token check is insufficient. Content/stage/intent commit atomically.
5. Claim transactions touch only queue rows. Mutation transactions obtain required resource
   locks first, then revalidate their attempt. Release SQL before external work and revalidate
   after it. Late results, including failures, cannot modify a successor's attempt.
6. Lease expiry means work can be reassigned; it does not prove the old provider/HTTP task
   stopped. Do not free admission capacity merely because an attempt expired.
7. Recover missed local notifications by bounded durable polling. Local watches, broadcasts,
   coalescing and wake/retire helpers are optimizations, never durable authority.
8. Define terminal, retry, cancellation and prune transitions explicitly. Resolve ambiguous
   commits by operation identity. Retry only identified database deadlock/serialization
   victims with whole-transaction rollback, bounded jitter and a deadline; never automatically
   replay providers, revision conflicts or uncertain external effects.

### 3.2 Resource locks and bounded admission

Retain shared deal mutation locks and exclusive authority/lifecycle locks. Acquire sorted
deal guards first, then sorted user admission rows and numbered slots when required, then
sorted notebook/folder/note or template/thread/file parents, then specific job/cleanup and
child rows. Choose the strongest mode at entry; avoid SHARE-to-UPDATE upgrades. Define a
canonical ordering for mixed batches and test implicit FK/unique/trigger locks too.

Notebook admission must preserve 256 active jobs globally and 32 per user without
count-then-insert races. Use independently claimable bounded slots and user-scoped
admission coordination; no singleton capacity counter. Claim a free slot and create/retry
atomically; replay returns before allocating; a terminal transition frees its slot exactly
once. Cleanup has its own attempt ownership and tombstone retention. For an absent note,
use its accepted job/cleanup identity and active-save uniqueness, not a lock on a missing row.

Keep pool/acquisition/query/lock bounds. Ordinary reads remain MVCC/snapshot reads without
claims, writer checks or explicit resource locks. Default writes remain READ COMMITTED;
use stronger isolation only for a named invariant with supporting tests.

### 3.3 Remote publication and deletion

Persist projection intent with the canonical mutation, recording resource, immutable
version/revision, monotonic generation, desired action and recoverable stage. Reuse existing
complete checkpoints; add domain-owned intent only where a gap exists. SQL commit precedes
projection and graph failure never deletes canonical records to conceal the failure.

First prove Helix's actual conditional transaction behavior against a disposable pinned
server. At every remote setup/chunk/publish/delete operation, obsolete generations and
operations must be rejected atomically at the remote point of use. A SQL check immediately
before HTTP, a content hash, a local mutex or lease expiry alone is not a fence.

If atomic remote rejection is unavailable, implement immutable generations/revision roots,
authoritative canonical visibility filtering and durable cleanup. Prove that delayed work
cannot become visible or resurrect a deleted resource. Cover every write route and batch,
not just final publication. Tombstones must survive for as long as obsolete publishers can
still arrive; pruning needs evidence of safety, not a short expiry assumption.

Cleanup and reconcile must recheck the specific canonical resource and generation. A stale
catalog snapshot must not delete a newly committed projection. Remote acknowledgments are
recorded only under the current attempt/version; retries converge after an acknowledgment
is lost. Never keep a SQL transaction open to wait for a remote acknowledgment.

## 4. Domain implementation work

Paths below are repository-relative. Existing revision/parent behavior is retained and
extended only where needed; completed foundation work is not an instruction to rebuild it.

### 4.1 Schema and startup

- In `backend/src/app/schema.sql` and `backend/src/app/migrations.rs`, add the minimum durable
  attempt, document-job/event/input, admission-slot, projection-intent and tombstone state
  required by the chosen protocols. Define ownership, unique identities, legal transitions,
  expiry indexes, safe bounds and retention before coding workers.
- Continue the authorized fresh-development-schema approach: bump the version/checksum for
  an incompatible fresh layout, initialize only an empty selected schema and reject older/
  drifted layouts without mutation. No legacy import or automatic reset is required. If the
  follow-up must preserve a populated deployment, make that an explicit migration requirement
  before changing schema; validate/backfill active jobs/slots and reject inconsistent capacity.
- Keep startup schema advisory locking separate from runtime claims. Concurrent startup and
  restart must validate the new layout, owner invariants and document readiness without deletion.
- `backend/src/app/config.rs`, `bootstrap.rs` and `main.rs` own injected limits, assembly,
  polling and shutdown. Drain only this instance's work; do not imply ownership of all workers.

### 4.2 Documents and ingestion

| Files | Required work |
| --- | --- |
| `backend/src/domains/documents/store/postgres.rs`, `store/identity.rs` | Retain shared deal authorization guard. Coordinate unique `(deal, content_hash)` and logical file/version identity, conflict-safe winner readback, version allocation/current pointer and per-version generation CAS. Preserve immutable blob/manifest/chunk/image checkpoints and activity rollback. |
| `backend/src/domains/documents/ingestion/service.rs`, `persistence.rs` | Claim before expensive duplicate-prone work. Local `processing_locks` only coalesce. Rebuild prepared plans against the winning canonical file identity; accepted image text and exact retry identities survive failures. Reauthorize canonical commit/publication. |
| `backend/src/domains/documents/ingestion/job_service.rs`, `jobs.rs`, `handler.rs`, `mod.rs`; proposed `job_repository.rs` | Replace authoritative memory-only jobs with durable admission, input, status/events, claims and recovery. Status/SSE works on another instance; preserve event payloads/limits and distinguish unsubscribe from cancellation. Wire new modules/tests. |
| `backend/src/domains/documents/graph/repository.rs`, `writer.rs`, `deletion.rs`, `query.rs` | Consume version-aware projection intent. Fence setup/chunks/current-version publication/delete, including out-of-order CURRENT_VERSION and delayed batches. Confirm only the current attempt/version; graph failure leaves canonical rows intact. |
| `backend/src/domains/documents/viewing/service.rs` | Keep bounded immutable preview cache and delivery authorization. Cache eviction is local, not deletion enforcement. Resolve local-file availability before claiming multi-host support. |

An absent hash row cannot be locked. Insert its unique claim first, or resolve the winner
in a fresh transaction. Specify failed-parse retry/retention so an upload hash is not
permanently poisoned. Simultaneous parser-generated IDs must converge to one identity
without orphan files, duplicate activity or replacing already accepted image text.

### 4.3 Notebook saves and cleanup

- `backend/src/domains/notebooks/job_repository.rs`: make admission/retry/claim/fence/persist/
  checkpoint/fail/cleanup/prune transitions conditional under database time and current attempt.
  Implement slot ownership/release and user bounds. Retain parent-before-folder-before-note
  ordering, final note CAS and atomic committed-revision/job-stage recording.
- `backend/src/domains/notebooks/save_jobs.rs`, `service.rs`, `model.rs`: carry attempt identity
  through every stage, renew/stop only owned attempts, recover queued/expired work through polling
  and preserve immutable submitted snapshots and pinned pipeline identity. Index retry must not
  increment or reapply an already committed note revision.
- `backend/src/domains/notebooks/repository.rs`, `graph.rs`: atomically fence admitted jobs and
  enqueue durable cleanup on deletion/relocation; protect new-note identities. Preserve immutable
  revision roots while adding delete/generation nonresurrection and duplicate-publication behavior.
- Existing handlers retain revision validation, 202 save admission, authorized status/SSE and
  asynchronous conflicts. Any changed status/retention contract must update both client adapters.

### 4.4 Permanent deal deletion

- `backend/src/domains/deals/deletion/repository.rs`, `model.rs`: replace select-pending followed
  by unconditional transitions with atomic claims. Check token/expiry/expected phase on checkpoint,
  purge, target completion, failure and terminal completion. Prevent phase or terminal regression.
- `service.rs`, `worker.rs`, `lifecycle.rs`: local drain/admission is only process cleanup. A durable
  barrier and current claim authorize purge; remote fencing precedes cleanup. Recover expired
  attempts with bounded fan-out; blocked deal A must not prevent B progressing.
- Preserve revisioned acceptance, authorized idempotent receipt replay, private receipt access,
  sorted notebook-parent locks for detachment, file/thread snapshots and permanent deal-ID guards.
  Test stale workers at each phase, including after canonical purge and before graph acknowledgment.

### 4.5 Templates and app reconciliation

- `backend/src/domains/templates/slide_templates/repository/classification.rs`: atomic eligible
  claims, database-time expiry and independent classification/embedding tokens. Carry template
  revision and attempt through completion/failure/retry/refresh; mutate only the active stage and
  do not overwrite another stage from a stale whole-row snapshot. Parent locks remain short.
  Classification metadata, FTS/terms and stage commit under the same current guard.
- `repository.rs`: finish atomic content/metadata/assets/preview/classification invalidation
  where replacement needs it; retain CAS and owner/app disclosure. Deletion commits durable
  projection tombstone/intent. Sort batch resource acquisition and resolve duplicate identities.
- `repository/apps.rs` and builtin paths: close concurrent new-app/builtin attachment via compatible
  per-app coordination on both paths or durable attachment reconciliation intent. Preserve omitted
  registration fields, builtin fingerprint guards and app-specific deletion choices. No global
  catalog lock. Startup currently does not seed builtins; do not reintroduce a seeder.
- `providers.rs`, `providers/worker.rs`, `models.rs`: stage-specific attempt/result DTOs, bounded
  provider timeouts/retries and token-aware terminal paths. Remove correctness dependence on the
  shared mutation mutex only after the replacement protocol is proven.
- `facade.rs`, `graph.rs`: durable projection/delete/reconcile ordering and remote generation fences;
  preserve local preparation/conversion/admission capacity and cancellation semantics. Product and
  native routes must retain matching owner/app scope, revision envelopes and error contracts.

### 4.6 Assistant, authentication and replica limits

- `backend/src/domains/assistant/chat/repository.rs`, `service.rs`, `model.rs`: retain thread sequence
  locking; reconcile durable request ID/payload, define restart behavior for stranded streaming runs
  and guard terminal writes by current run ownership, lifecycle and authorization. If recovery can
  reassign runs, use an attempt identity. Do not replay a potentially live provider request blindly.
- `backend/src/domains/identity/repository.rs`, `service.rs`, `access.rs` and
  `backend/src/app/http/auth.rs`: unique registration conflicts and bounded conditional session
  cleanup; preserve periodic database-backed stream revocation and final result checks. Logout/
  revoke on A must close delivery on B within the existing bound. Broadcasts only speed notification.
- Make identity/IP abuse limits and provider admission coherent across replicas using durable scoped
  policy or a verified shared ingress policy. Keep local Argon/CPU/conversion/search permits, but
  document/enforce total replica budgets; do not multiply a supposedly global quota per instance.
- Review local paths and accepted inputs for cross-process accessibility. Same-host tests do not
  establish multi-host filesystem availability. State any remaining deployment restriction explicitly.

### 4.7 Remaining canonical race audit

Before cutover, review every mutation, including the foundation's deals/memberships/companies,
archive, roster/team caps and no-ops, ownership transfer, notebook folder/name/location changes,
template repair/update paths and session lifecycle. Prove races with independent pools, with global
serialization disabled only by the existing test harness until removal. Cover zero-owner prevention,
reserved-ID creation, same-target edit/delete, authorization change versus commit and rollback.

Retain derived caches, local wake helpers, cancellation signals and bounded browser slots where
they protect local resources. The transaction wrapper's mutex protects one connection, not the
application; replace it with mutable borrowing only if that materially simplifies the callback API.
Do not expand the isolated SharePoint adapter or dormant reminders. Review standalone graph writers
such as `backend/src/bin/embedding.rs` for fence compatibility without invoking destructive utilities.

## 5. Verification and failure matrix

Use explicit disposable PostgreSQL `_test` schemas and a separate fresh Helix store. Test real
remote behavior as well as fakes. Retain identities/ports/schema ownership, clean only task-owned
resources and never inspect `.env`, reset development data, invoke `clear_helix` or spend on live
providers as routine verification. New backend tests/support stay under `backend/tests/` and must
be manually wired (`autotests = false`). Use barriers/observable lock waits, not sleep-based ordering.

| Scenario | Required evidence |
| --- | --- |
| Same-job competition | Exactly one eligible claim; different jobs can be claimed concurrently; stale renew/checkpoint/failure/completion rejected after takeover. |
| Resource contention | Different deals/notebooks/templates make progress while A is blocked; same-revision edits have one winner with no losing activity/children. |
| Capacity | Concurrent admissions/retries preserve 256/32 bounds; replay allocates no second slot; terminal release happens once; expiry alone releases nothing. |
| Crash boundaries | Kill after admission, claim, canonical commit, remote staging, remote publish and before acknowledgment/checkpoint. Successor recovers without duplicate content or losing accepted input. |
| Documents | Same bytes with different parser IDs converge; changed versions/current uniqueness hold; accepted image text survives restart; lost graph acknowledgment is idempotent. |
| Notebook lifecycle | Save/retry versus folder move/delete, notebook/note deletion, cleanup and prune; no obsolete attempt writes or post-delete resurrection. |
| Template stages | Classification versus embedding/refresh/delete; stale failure cannot reset successor; app/builtin registration and reconciliation do not reattach deleted choices. |
| Remote ordering | Old/new generation publication and delete versus delayed setup/chunks/publish; every path rejects obsolete work or canonical filtering hides it and cleanup converges. |
| Authorization | Removal/archive/deletion versus writes; logout on A while B streams; no stale completion or final result delivery after revocation. |
| Cross-instance status | Admission on A, status/SSE/reconnect/retry on B, A exits; durable sequences/terminal states and owner disclosure remain correct. |
| Startup/shutdown | Simultaneous startup, one process exits while another reads/writes/recovers; drain stops only local work and does not release another attempt. |
| SQL failure | Cancellation rollback, lock/pool deadlines, deadlock/serialization retries and uncertain-commit reconciliation remain bounded and sanitized. |

Extend existing owning suites: PostgreSQL client/read/concurrency tests and support, migrations,
`resource_concurrency_tests.rs`, membership/deal/company/deletion routes/repositories, document store/
ingestion, notebook repository/save-job/HTTP/graph suites, template lifecycle/retry/worker/graph/
lexical parity suites, authentication and architecture tests. Add manually wired integration suites
for admission slots, durable document jobs, remote fencing and real two-process recovery.

Two independent in-process pools are necessary but insufficient. Before global control removal,
use separate worker/service test subprocesses with test-only unguarded clients to prove recovery
protocols; the current production bootstrap intentionally rejects a second writer. After removal,
start two actual application processes through the ordinary production bootstrap against the same
disposable dependencies. Exercise simultaneous startup, overlapping independent writes, one CAS
winner, claim takeover, process kills, cross-instance SSE/revocation, duplicate uploads and deletion
versus delayed projection. Capture subprocess identity, independent connections, exact pause/kill
points and observed canonical/remote state. Only the latter establishes end-to-end application
support; neither thread tests nor a test-only bootstrap proves the final deployment behavior.

Capture a comparable baseline before changing write controls: read p50/p95, independent/same-
resource throughput, pool/lock waits and queue lag, with workload/concurrency/versions specified.
Repeat after removal. Report actual limitations and bottlenecks; tests do not imply a throughput gain.

## 6. Delivery order and removal gate

1. Rebase on the completed foundation, rerun checks/inventory and capture measurements. Resolve
   schema/deployment policy, remote conditional-write capability and filesystem assumptions.
2. Add durable schema/admission/input/intent state and atomic claim protocols. Prove per-domain
   stale-attempt and capacity tests without the global test-schema guard.
3. Implement remote generation/delete fences or the proven immutable-publication alternative.
   Connect recoverable intent consumers and token-aware cleanup/reconcile across all domains.
4. Run separate worker/service subprocess crash/recovery, status, revocation and remote-race tests
   using the test-only unguarded harness. Retain global production controls while safeguards are
   incomplete; any bypass stays test-only. Full production-bootstrap evidence follows removal.
5. Remove runtime global ownership in the follow-up PR only after the above gates: delete
   `WriterLease`, writer lease fields/registration/pings, `with_writer_lease`, `ensure_writer_lease`,
   `fence_commit`, `WRITER_GENERATION_KEY`, `postgres_writer_generation`, runtime `WRITE_LOCK`,
   `WriterBusy`/`WriterLeaseLost` and obsolete takeover tests. Update bootstrap/shutdown. Keep only
   the dedicated startup schema advisory lock; remove dead compatibility code/configuration.
6. With the ordinary production path and no bypass, rerun independent-write and full two-process
   tests plus broad gates. Add architecture enforcement against reintroducing runtime global
   ownership/serialization, with a narrow startup-only allowance. Re-measure and document results.
7. Drain old requests/workers before operational cutover. Old binaries relying on the global lock
   must not coexist with new writers ignoring it. Validate one new process, then two; keep database/
   graph/schema versions compatible. Record rollback or forward-fix steps and remaining host limits.

Final acceptance requires zero application writer ownership checks on reads **and** writes, zero
schema-wide runtime mutation serialization, independent resources actually overlapping, correct
same-resource/authority behavior, durable recovery and remote nonresurrection. Retaining the old
lease as a fallback is an intermediate result, never completion. Do not swap in a renamed global lock.

Rollback requires draining new workers and verifying schema/job/remote protocol compatibility
before reverting binaries. Otherwise forward-fix. Restoring backups is a separately scoped recovery
with an explicit data-loss window and projection repair; never silently reset existing stores.

## 7. Implementation checks and documentation

The commands below are future implementation gates, not checks run for this planning-only change.
Start with focused test-name filters. From `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

When shared/client contracts change, from `frontend/`:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run check:notebook-bundle:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
npm run check:notebook-bundle:desktop
```

When the native relay changes, from `frontend/src-tauri/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Preserve adapter/route tests for revisions, retained drafts, retries, destructive confirmation,
late responses, duplicate prevention and focus/announcements. Inspect changed UI paths, both
applicable runtimes, themes, viewports and reduced motion. Follow project review/refactoring gates.

Update current architecture and recovery guidance in the implementation PR: `docs/ARCHITECTURE.md`,
`docs/postgresql-storage.md`, its verification record, `docs/notebook-api.md`,
`docs/deal-permanent-deletion.md`, `docs/template-ownership.md`, `docs/authentication.md`,
`docs/backend-boundary-checks.md`, `AGENTS.md` and affected project skills. Describe actual claim/
retention/schema/remote semantics, replica budgets, revocation timing, deployment limits and rollout.
Remove single-writer guidance only with the code removal; do not describe planned support as current.

Before removing controls, run and inspect these inventories from the repository root:

```sh
rg -n 'writer_lease|WriterLease|WriterBusy|WriterLeaseLost|WRITER_GENERATION_KEY|ensure_writer_lease|fence_commit|postgres_writer_generation|WRITE_LOCK|pg_.*advisory' backend/src backend/tests
rg -n 'SqlBuilder::(insert_into|update|delete_from)|Q::(insert_into|update|delete_from)|sqlx::query|sqlx::raw_sql|\.pool\(' backend/src
rg -n 'PostgresTransaction|transaction_async|transaction_result_async|read_transaction' backend/src
rg -n 'Mutex|RwLock|Semaphore|AtomicBool|broadcast::|watch::|lease|revision|generation' backend/src
```

Reconcile every mutation/external writer, SQL trigger and graph query with the protocol. Text
searches alone do not prove correctness. Final ownership hits may include startup-only locking
and negative regression assertions, but no runtime global lease/generation/universal write lock.

Planning review coverage: completed-versus-proposed state, canonical ownership/authorization,
claim/parent ordering, remote capability uncertainty, recovery, bounded work and acceptance evidence.
No runtime, schema, API or operational behavior changes in this document-only replacement.
Architecture impact: none — canonical architecture already records the retained global controls
and pending distributed safeguards. Validate local references, Markdown structure, removal of the
superseded file and `git diff --check`; implementation builds/tests need not rerun to save this plan.
