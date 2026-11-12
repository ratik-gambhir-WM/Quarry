# Concurrent readers, writers, and optimistic concurrency

Date: 2026-11-12. Status: proposed; implementation has not started.

This plan follows the concurrency assessment and replaces the proposed single-application-writer
model with concurrent PostgreSQL access. Saving this document changes no runtime behavior,
API, schema, or stored data. [Architecture](../docs/ARCHITECTURE.md) remains the description of
the implemented system. Preserve all existing uncommitted work and re-check these anchors before
implementation; the PostgreSQL consolidation is itself present in the working tree.

## 1. Intended outcome and stable contracts

Allow independent requests to read and write concurrently, first within one Axum process and
ultimately across multiple Axum processes using one PostgreSQL primary. Use optimistic revisions
for edits that require stale-write protection, database constraints for structural invariants, and short resource-scoped
transactions/locks for operations that must coordinate. Do not introduce a writer per table.

Remove the application writer lease, its global generation fence, and the schema-wide mutation
lock only after every dependent invariant has a tested replacement. Retain separate startup/schema
coordination and job-specific ownership leases. Optimistic concurrency still uses PostgreSQL row
locks during updates; it avoids holding an application lock while a person edits a resource.

Preserve:

- PostgreSQL canonical ownership; Helix as a recoverable search projection; canonical commit
  precedes external publication, and projection failure never deletes accepted content.
- Authenticated actor identity, ownership, membership, current-version and deletion barriers,
  including revocation checks during workers, streams, and final result delivery.
- Atomic activity/revision updates, immutable versions, deduplication identities, notebook save
  snapshots and operation IDs, bounded work, rollback, and explicit partial-failure reporting.
- Shared web/desktop behavior, `/api/v1`, camelCase JSON, existing SSE event names, and historical
  `sqliteCommitted` / `sqlite` wire values. Coordinate any additive revision fields across clients.
- No provider, Office, filesystem conversion, or Helix network call inside a PostgreSQL transaction.
- Existing data, old SQLite/Helix stores, secrets, and unrelated working-tree changes.

Read replicas, multi-primary PostgreSQL, collaborative text merging, offline synchronization,
and a general message broker are outside this refactor. No throughput target is claimed before
measurement. Read replicas are unnecessary to permit concurrent readers on the primary.

## 2. Current evidence and implementation owners

| Area | Current anchor and dependency to replace |
| --- | --- |
| PostgreSQL execution | `backend/src/adapters/postgres/client.rs`: dedicated session lease, serialized health pings, generation commit fence, schema-wide write lock; aggregate reads already use repeatable-read snapshots. |
| Startup/schema | `backend/src/app/bootstrap.rs`, `backend/src/app/migrations.rs`, `backend/src/app/schema.sql`: schema initialization/validation precedes lease registration; version 1 uses source checksum and live catalog fingerprint. |
| Memberships | `backend/src/domains/memberships/repository.rs`: `bump` and `set_role` write previously computed revisions without old-revision predicates; current global ordering protects them. |
| Notebook saves | `backend/src/domains/notebooks/job_repository.rs`: candidate selection and claim update are separate; claim update uses job ID without an eligibility predicate. Audit admission, persist, renew, retry, failure, and cleanup as well. |
| Deal deletion | `backend/src/domains/deals/deletion/`: local lifecycle locks/draining plus durable receipts and remote lifecycle fencing; local drain knowledge is insufficient across processes. |
| Documents | `backend/src/domains/documents/store/`, `backend/src/domains/documents/ingestion/`, `backend/src/domains/documents/graph/`: exact-content retry, identity/current-version claims, local ingestion locks, in-memory jobs, and serialized graph publication. |
| Templates | `backend/src/domains/templates/slide_templates/repository/`, `providers/worker.rs`, `graph.rs`: classification/embedding claims, retries, lease expiry, owner/app scope, and projection ordering. |
| Other mutations | Companies, users, identity, deal metadata, assistant chats and template catalog mutations: inventory all mutation entrypoints, including internal worker and cleanup paths. |
| Existing evidence | `backend/tests/integration/adapters/postgres/client_tests.rs`: snapshot overlap, cancellation rollback, lease lifetime/loss and takeover. These tests describe existing behavior, not proof of concurrent-writer safety. |

Relevant canonical context: [storage/recovery](../docs/postgresql-storage.md),
[boundary placement](../docs/backend-boundary-checks.md),
[notebook API](../docs/notebook-api.md), and
[deal deletion](../docs/deal-permanent-deletion.md).

## 3. Design rules

### User-only data: select concurrency policy by overwrite risk

User-only ownership is an authorization scope, not evidence that only one write can occur.
The same person can edit from multiple tabs/devices, and asynchronous saves, retries or background
jobs can overlap. Do not blanket-disable conflict protection on every user-owned resource.

For simple user-only values whose earlier changes may safely be overwritten, use direct writes
without optimistic revisions. Apply this policy beyond profile fields to other private settings
and preferences when their operation has the same independent-field, last-write-wins semantics.
Record that classification in the mutation inventory; being user-scoped alone is not sufficient.

| User-only operation | Policy |
| --- | --- |
| Ordinary profile fields and private settings/preferences | Direct field-scoped writes; last-write-wins is accepted. No expected revision or custom concurrency lock. |
| Private notes/documents and other valuable editable content | Retain optimistic revisions when stale replacement could lose work; notebooks remain protected. |
| Atomic counters, toggles and read-modify-write commands | Use atomic database expressions or targeted transactions. No client revision is needed when command semantics resolve concurrency safely. Define whether a toggle means invert or set an explicit value. |
| Creates, retries, imports and background jobs | Retain operation identity, uniqueness, atomic claims and worker fencing where duplicate or stale work would be harmful. |
| Credentials, sessions, account deletion and security-sensitive changes | Retain operation-specific verification, integrity and lifecycle safeguards. |

For example, two tabs changing a private note from the same saved version can lose one edit even
though both requests belong to the same user. Two tabs setting a display preference may safely
resolve to the last applied value. Choose by that consequence rather than by owner count.

### Personal profile and preferences: direct writes

Ordinary user-scoped profile fields and personal preferences use direct, last-write-wins updates.
Do not add `expectedRevision`, a revision preflight, an application lease, or a custom concurrency
lock to these operations. Concurrent changes to the same field from the user's tabs/devices may
overwrite one another; this is an accepted product policy for these settings.

Update only explicitly submitted fields so changing a preference does not overwrite unrelated
profile fields from a stale form snapshot. Normal PostgreSQL statement atomicity and constraints
still apply. Scope the write to the authenticated user's record, validate allowed fields/values,
and retain normal missing-record/error handling. No concurrency check does not mean no security
or integrity checks.

This policy covers ordinary profile/preferences data, not every private resource. Preserve existing
notebook revision protection. Password/email changes, sessions, account deletion, permissions and
other security-sensitive lifecycle operations retain their specific verification and transactional
safeguards; user ownership alone does not make those operations safe to overwrite blindly.

### Optimistic edits and conflict contract

Use a monotonic integer revision on each resource/aggregate selected for stale-write protection.
Exclude ordinary personal profile/preferences updates as specified above. Reuse existing
revisions where their meaning matches; keep content, access-control and job-event revisions distinct.
Do not use PostgreSQL `xmin` or wall-clock timestamps as public edit revisions. Preserve the existing
JavaScript-safe integer bounds and define exhaustion as a conflict rather than wrapping.

A read returns `revision`; a replacement/patch that depends on the previously read state submits
`expectedRevision`. The repository performs the authorized conditional write and revision increment
atomically, conceptually `UPDATE ... SET revision = revision + 1 WHERE id = ? AND revision = ?`.
Check the affected row count. Never check a revision with a SELECT and then issue an unconditional
write. A transaction containing child writes and activity must roll back fully on a failed guard.

Return an authorized stale-edit conflict as HTTP 409, following existing error conventions. Resolve
missing/inaccessible resources according to current endpoint policy before disclosing conflict state.
Missing/malformed required revisions are request-validation errors. Document no-op behavior per
operation; preserve membership no-ops that neither advance revisions nor emit activity. Define
delete-versus-edit behavior explicitly; destructive actions must not silently discard newer edits.

Notebook asynchronous saves retain HTTP 202 admission and existing job conflict reporting where
the conflict is determined by the worker. Admission does not promise a committed content revision.
Do not flatten all asynchronous conflicts into synchronous HTTP 409 responses.

Clients preserve unsaved/submitted content, distinguish newer edits from an in-flight snapshot,
and offer explicit reload/review after conflict. Do not silently overwrite or retry against the new
revision. Explicit reload requires confirmation when it would discard unsaved edits. This is
optimistic concurrency control, not a requirement to optimistically display an uncommitted save.

### Transactions, authorization, and targeted coordination

Default short writes to READ COMMITTED. Keep repeatable-read, read-only aggregates where a stable
multi-query snapshot is required. Snapshot consistency does not replace current authorization or
final-result revocation checks. Use SERIALIZABLE only for identified invariants where it is simpler
than explicit coordination, with tested whole-transaction retries.

For each mutation, list every fact whose concurrent change would invalidate its commit: permissions,
deletion state, parent membership, resource revision, uniqueness and lifecycle status. Coordinate
those facts through stable rows or consistently keyed transaction advisory locks before checking
them. All competing paths, including revoke/delete, must take compatible conflicting locks. A plain
authorization SELECT followed by a write at READ COMMITTED leaves a race. Preserve early HTTP
authorization as well as the transaction guard; apply the boundary placement decision to each check.

Prefer existing parent/control rows as stable lock targets. Lock absent-resource invariants through
a stable parent or a unique constraint; SELECT FOR UPDATE cannot lock a row that does not exist.
Start with a deal-scoped control lock for complex membership/deletion workflows if necessary,
while keeping unrelated deals independent. Measure before further partitioning within a deal.
Document one acquisition order for deal, resource and job locks, including multi-deal operations;
test deadlock handling. Do not replace one global hotspot with a universally updated control row.

### Workers, idempotency, and external publication

Claim work atomically, using a short `FOR UPDATE SKIP LOCKED` transaction or an eligibility-checked
conditional update with RETURNING. Use database time for lease eligibility and expiry. Increment
an attempt generation or issue an attempt token; renewals and every checkpoint/terminal update
must match active ownership. Completion after ownership loss must fail. Cover expired-lease
takeover, retry, cancellation, invalid checkpoints and cleanup, not just the successful claim.

Use stable operation IDs and unique constraints for creates/admission. Exact replay returns the
original result; reuse with different payload is rejected. Reconcile uncertain commits by operation
identity before replay. Limit automatic retries to known-safe database-only attempts, such as selected
deadlock/serialization failures, with bounded jitter/backoff and a deadline. Never replay provider
calls or ambiguous commits blindly. A worker lease alone cannot prevent duplicated external effects.

Persist projection intent in the canonical transaction, either through proven existing durable
checkpoints or a minimal outbox where coverage is absent. Choose per domain during the inventory;
do not introduce duplicate queues. Consumers must be idempotent and version-aware. Remote writes
must reject obsolete generations/deleted resources at the external point of use; checking a token
only in PostgreSQL before a Helix call is insufficient. A delayed worker must not overwrite a newer
projection or recreate deleted content. Durable intent requires retry, recovery and retention policy.

## 4. Implementation stages and acceptance gates

Each stage should be a separately reviewable change. Earlier stages retain global guards until
replacement protections pass tests with those guards disabled in an isolated test harness. Passing
tests under the old serialization does not demonstrate concurrency safety.

### Stage 1 — Inventory and baseline

- Enumerate every repository mutation, worker transition, direct pool access, local lock, cache,
  rate limit and cross-store side effect. Record owner, invariant, coordination scope, retry policy,
  replacement and test for each; include create/delete and operations without public edit fields.
- Classify each API mutation as direct last-write-wins personal settings, stale-state edit,
  atomic command, idempotent create, or lifecycle transition. Add expected revisions only where
  required by that semantic contract; ordinary personal profile/preferences require none.
- Capture representative read latency, independent-write throughput, global lock wait, pool wait,
  conflict rates and worker recovery behavior using disposable data. Include same-resource contention.

Acceptance: no mutation remains unclassified; baseline is reproducible; schema and deployment
decisions below are resolved before changes that depend on them.

### Stage 2 — Data and public edit contracts

- Add missing revisions, constraints, claim fields or durable intent only where the inventory requires
  them. Define revision ownership for metadata, child mutations and deletes. Backfill deterministically.
- Decide the data-preserving schema transition explicitly. The current version-1 migrator rejects
  checksum/catalog drift; editing schema.sql alone would strand populated databases. Introduce a
  reviewed forward migration mechanism if DDL is needed, validating known prior layouts and supporting
  fresh initialization to the same final layout. Keep old checksums immutable in migration history.
- Define backup, verification and recovery before rollout. No reset, automatic recreation, silent
  checksum refresh, or real-data experiment is permitted. Test upgrade rollback on failure.
- Coordinate DTOs/handlers, `frontend/src/contracts/quarryApi.ts`, adapters and Tauri relay where
  their actual payload handling requires it. Integrate conflict UX through feature-owned workflows.
- Use a coordinated client/server deployment for newly mandatory revision fields; an old client
  must never silently bypass a required revision. Preserve `/api` compatibility intentionally.

Acceptance: two authorized edits from the same revision yield one commit and one conflict, with
correct web/desktop feedback and intact unsaved edits. Migration tests cover populated supported
layouts, unknown-layout rejection, restart validation and fresh-schema parity.

### Stage 3 — Replace relational ordering dependencies

- Implement atomic revision predicates and row-count checks. Keep related content, activity and
  revisions in one transaction; enforce deduplication and current-version identity with constraints.
- Introduce scoped permission/deletion coordination across all competing writers. Protect ownership
  transfer and exactly-one-owner invariants, including the zero-owner case not prevented by uniqueness.
- Convert notebook and template claims/checkpoints; give deletion workers distributed claim ownership.
- Audit users/authentication, companies, deals, chats and internal maintenance paths alongside the
  highlighted notebook/membership paths. Local mutexes may remain optimizations, never correctness guards.

Acceptance: deterministic races prove safety without the global mutation lock; unrelated resources
overlap; stale edits, revocation, deletion, duplicate admission and expired worker attempts cannot
produce unauthorized writes, duplicate identities or partial aggregate commits.

### Stage 4 — Close cross-process and cross-store gaps

- Make document job admission/status/events durable and address replay/reconnection so a request
  accepted on instance A can be polled/streamed on B. Durable status plus bounded polling can be
  sufficient initially; a new broker is not mandatory. Preserve event names and authorization.
- Replace local-only deletion draining with distributed active-work accounting/claims and remote
  lifecycle fencing. Model worker crash, long provider calls, lease expiry and old remote writes.
- Implement/reuse durable projection intent and remote generation fencing for documents, notebooks
  and templates. Preserve exact-content retry and accepted image text. Verify actual Helix conditional
  transaction capabilities before selecting the remote fencing protocol.
- Classify caches as disposable derived state or correctness-sensitive state. Specify cross-instance
  invalidation/revalidation, stream revocation, local file accessibility and native conversion artifacts.
- Bound aggregate worker/provider concurrency and abuse admission across replicas; process-local
  authentication limits must not be multiplied into an undocumented security bypass.

Acceptance: kill/restart one instance during admitted work and run two competing workers; another
instance can recover/poll/stream safely, and delayed work cannot resurrect deleted or newer content.
If any domain lacks a proven external fence, multi-instance deployment remains blocked for that domain.

### Stage 5 — Remove global controls and cut over

- Decouple schema initialization locking from runtime write locking in migrations/client code.
  Concurrent startup must serialize initialization and reject incompatible schemas without deletion.
- Remove `with_writer_lease`, lease pings, ownership-loss gating and the global generation commit
  fence after replacements are proven. Remove the schema-wide mutation advisory lock. Audit all
  references and tests rather than leaving a dormant bypass or an unsupported runtime toggle.
- Keep bounded pool/query/lock timeouts, rollback on cancellation, read-only transaction checks,
  snapshots, scoped resource locks and job leases. Size total connections across APIs/workers/replicas.
- Replace tests for deliberately removed single-owner behavior with concurrency/recovery tests;
  retain useful cancellation, transaction and snapshot coverage. Simplify obsolete metadata/errors
  only after confirming ownership and compatibility; do not delete uncertain user work.
- Drain old HTTP requests and workers before the cutover. Do not run old globally coordinated
  writers beside new writers that ignore those locks. Deploy one new instance for smoke verification,
  then two against the same isolated validation database/projection, then enable replicas operationally.

Acceptance: two instances start and accept overlapping independent writes; reads remain available
when one instance exits; all race/security/recovery gates pass. No application-wide writer owner or
universal runtime write lock remains. Only schema operations retain startup-wide coordination.

### Stage 6 — Review, document and measure

- Compare the same baseline workloads at one and two instances. Report latency, throughput, pool
  saturation, scoped lock waits, conflicts/retries and queue lag; do not equate more replicas with
  guaranteed throughput gains. Observe PostgreSQL/Helix bottlenecks and provider quotas separately.
- Review changed backend and shared client code for correctness, security, maintainability and
  performance; apply small behavior-preserving simplifications. For changed UI, review architecture/
  state, accessibility/interaction, visual consistency, subscription isolation and loading costs.
- Update canonical architecture, storage/recovery, notebook/deletion/authentication documentation
  and affected agent guidance in the implementation changes that alter those contracts. Clearly
  distinguish completed concurrent SQL access from remaining multi-instance limitations.

Acceptance: no unresolved correctness/security finding, no unsupported distributed claims, no
unrelated diff or accidental dependency churn; remaining limitations and measurements are explicit.

## 5. Required race and failure coverage

Use independent PostgreSQL connections/client instances and barriers, not sleeps as the ordering
mechanism. Use isolated random schemas in an explicit `_test` database. Add process-level tests for
the multi-instance gate; several client clones alone do not exercise distributed runtime assumptions.

| Race or failure | Required result |
| --- | --- |
| Different resources updated concurrently | Both commit; a paused write does not globally block the other. |
| Two edits/delete-versus-edit from one revision | Defined single winner/conflict outcome; no silent overwrite or partial activity. |
| Two unrelated reads plus a pending write | Reads overlap; aggregate snapshot remains internally consistent. |
| Personal profile/preferences from two tabs/devices | No revision is required; same-field writes follow last-write-wins, distinct-field patches preserve both changes, and another user cannot modify the record. |
| Revoke/ownership transfer versus protected mutation | Defined transaction order; stale authority cannot authorize a later protected commit. |
| Delete versus upload, chat persistence or projection | Accepted deletion barrier prevents resurrection across both stores/processes. |
| Duplicate create/save/upload and uncertain response | One canonical operation/identity; exact replay recovers it without duplicate side effects. |
| Simultaneous job claim/expired takeover | One current owner; obsolete attempts cannot renew, checkpoint, publish or complete. |
| Provider/Helix failure or crash after canonical commit | Accepted content retained; durable intent recovers without undoing newer work. |
| Disconnect, cancellation, deadlock, pool exhaustion | Rollback/release; bounded failures/retries; no unsafe automatic replay. |
| Restart/concurrent startup/schema mismatch | Safe validation/migration; no deletion, split ownership assumptions or checksum masking. |
| SSE reconnect on another instance/session revoked | Correct durable status/event ordering and parent scope; revoked delivery stops. |

## 6. Verification and operational recovery

Run focused test-name filters first; backend tests/support live under `backend/tests/` and must be
wired into the manually included test tree. Then, from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

For shared frontend/API changes, from `frontend/`:

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

If native relay code changes, from `frontend/src-tauri/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Inspect conflict/reload/retry flows, keyboard/focus, themes and reduced motion in the changed UI.
Use disposable integration dependencies for real cross-store/process tests; do not start production
services as routine verification, read real .env files, reset Quarry, or run clear_helix. Record and
clean only validation processes, schemas, containers and stores created by the task.

Operational rollback starts by draining every new worker/instance. Do not simply start an old binary
against a changed schema or concurrent workers. Prefer a compatible forward fix; permit binary
rollback only to a version verified against the resulting schema and durable job format. Restore
backups only through a separately scoped drained recovery procedure with an explicit data-loss
window and projection reconciliation. A retained obsolete writer lease is not a rollback strategy.

Before each implementation handoff, re-read the relevant architecture sections, run
`git diff --check`, inspect final status, report exact commands/outcomes and separate pre-existing
failures from new ones. This planning-only change requires path/content checks, not runtime builds.

## 7. Completion criteria

The refactor is complete only when concurrent SQL reads/writes and multiple backend instances are
proven, every inventoried invariant has a replacement test, required stale-edit conflicts work in
web and desktop, background work recovers across instances, and remote publication rejects stale
attempts. Global application ownership and mutation serialization are removed, schema startup
coordination remains, existing data survives, and canonical documentation matches the implementation.

Architecture impact of saving this plan: none — it records proposed work without changing the
implemented architecture. The implementation stages explicitly require canonical documentation and
guidance updates when their runtime, API, schema or operational contracts change.
