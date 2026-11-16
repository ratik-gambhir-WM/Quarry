# Permanent deal deletion

Schema 3 uses SQL-only document cleanup. `DELETE /api/v1/deals/{deal_id}` accepts an owner-authorized
revision-checked deletion and returns a durable receipt; `GET /api/v1/deal-deletions/{deletion_id}`
polls status. `/api` remains compatibility only. Receipt acceptance immediately hides the deal
and fences further protected work. Client confirmation, polling and cache invalidation are unchanged.

## Ownership and phases

Deletion removes deal metadata, parties/scopes, memberships/teams/activity, all logical files,
versions/blobs, document identities, chunks/vectors/lexical rows, input claims and document jobs.
It removes every deal-linked chat across members and detaches private notebook deal links.
Private notebook content/jobs/projections, companies, users, templates, unrelated deals, external
originals and exported copies survive. The receipt and lifecycle reservation survive completion;
deleted deal IDs cannot be reused.

Statuses remain `pending`, `running`, `retrying`, `blocked`, `completed`. Historical phases
`draining`, `fencing`, `sqlite`, `graph`, `verifying`, `completed` remain accepted wire values.
New work drains, passes through `sqlite`, verifies SQL absence and completes; transient phases
may be skipped. `sqlite` means canonical PostgreSQL cleanup. There is no document remote fence,
remote purge, index scan or Helix verification call. Schema-2 receipts are not imported in the
user-selected fresh MVP cutover. Old document Helix nodes are ignored, not certified erased.

## Worker and safety

The on-demand coordinator discovers durable receipts at startup, scans due operations in pages
of 16 and runs at most four cleanup tasks. It skips busy deals so later pages can progress, polls
once per second while work remains and retires when no pending/running/retrying receipts remain.
Acceptance commits/notifies in a short-lived task that survives cancellation of the HTTP future.
Blocked/completed receipts do not retain an idle worker.

After local producers drain, the repository takes exclusive retained deal lifecycle authority,
locks linked notebook parents, snapshots temporary file/thread targets, deletes linked threads,
detaches notebook links, cascades the deal aggregate and marks SQL file targets purged in one
transaction. SQL absence and residual-link checks precede completion and removal of temporary
targets. Failures retain a retryable/blocked receipt; canonical rows never disappear to conceal
an external failure. Late document workers must satisfy current membership, lifecycle and
attempt-token/database-clock fences inside their final mutation and cannot recreate purged data.

Preview caches are evicted. Job state needs no cache eviction: SQL cascades are authoritative,
and SSE/current-result checks stop access. Blocking preview tasks retain lifetime admissions after
request cancellation. Existing chat revocation and terminal persistence rules remain in force.
No PostgreSQL transaction waits across provider or native conversion work.

The shared global writer lease and mutation serialization remain. This change does not certify
general multi-instance workers. SQL deletion does not erase WAL/free pages, backups, provider
retention or copies already held by clients. It does not run VACUUM, a global graph clear or the
explicit MVP reset utility. Notebook/template Helix traffic remains unchanged.

## Verification and rollout

Real SQL tests cover owner/revision authorization, receipt replay, SQL purge failure/recovery,
retained other-deal/private-notebook resources, local task draining, busy-page progress,
request cancellation, lifecycle reservations and stale document attempts. See
[document storage](document-storage.md) and [PostgreSQL setup/reset](postgresql-storage.md).
The separate, explicitly invoked schema reset recreates all selected SQL product tables; it is
not this owner-scoped deletion API and is never called at startup.
