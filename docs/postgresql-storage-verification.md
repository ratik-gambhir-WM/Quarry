# PostgreSQL storage verification

The dated records below retain earlier schema-1 evidence. Current schema-2 implementation and
verification are recorded in the final November 12 section.

Verified November 11, 2026 against PostgreSQL 18.6 and the repository-pinned Helix 3 image.
The canonical database is **`Quarry`**, with PostgreSQL schema version 1 and 42 tables.
Helix remains the document/notebook/template graph and search projection. This change does not
migrate old data or change client contracts. See [setup and recovery](postgresql-storage.md).

## Automated gates

All commands below completed successfully from their respective build roots.

| Root | Command | Result |
| --- | --- | --- |
| `backend/` | `cargo fmt --all -- --check` | All backend Rust formatting passes |
| `backend/` | `cargo check --locked --all-targets` | Pass |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Pass |
| `backend/` | `cargo test --locked --all-targets -- --test-threads=6` | 525 passed, 14 opt-in tests ignored; all binary targets pass |
| `backend/` | `cargo build --locked --bin quarry-backend --bin check_database` | Pass |
| `frontend/` | `npm run typecheck` | Web and desktop pass |
| `frontend/` | `npm run check:boundaries` | Pass |
| `frontend/` | `npm test` | 99 files, 651 tests pass |
| `frontend/` | `npm run build:web` | Pass |
| `frontend/` | `npm run check:web-bundle` | Pass |
| `frontend/` | `npm run check:bundle-size:web` | Pass |
| `frontend/` | `npm run check:notebook-bundle:web` | Pass |
| `frontend/` | `npm run build:desktop-ui` | Pass |
| `frontend/` | `npm run check:bundle-size:desktop` | Pass |
| `frontend/` | `npm run check:notebook-bundle:desktop` | Pass |
| Repository | `bash -n quarry` | Pass |
| Repository | `git diff --check` | Pass |

Backend database tests used `QUARRY_TEST_DATABASE_URL=postgresql://127.0.0.1/Quarry_test`.
Each fixture owns a random schema. Driver pooling, bounded acquisition/disconnection failures,
cancellation rollback, read-only repeatable snapshots, concurrent writes, retained writer lifetime,
lost writer ownership, initialization/restart, catalog drift, unknown layouts and partial-DDL
rollback all have PostgreSQL-backed coverage. Domain/HTTP suites retain authorization, membership
revision/activity, content/version uniqueness, byte fidelity, image checkpoint recovery, private
notebooks/templates, durable jobs, stale lease fencing, and deletion receipt/replay coverage.

The dependency tree contains no SQLite driver. The product SQLite adapter, its obsolete tests,
and destructive reset binary are removed. The independent legacy Node service is unchanged.

## Explicit cross-store runs

The following filters ran separately with `cargo test --locked --lib <filter> -- <options>`.
Both database and graph targets were explicitly disposable; Helix listened on port 16969.
Set `QUARRY_HELIX_COMPAT_URL=http://127.0.0.1:16969` as well as the test database URL.

| Filter | Options | Result |
| --- | --- | --- |
| `v3_` | `--include-ignored --test-threads=1` | 7 passed: retained graph protocol, bounds, deletion fences, vector candidates, scoped search and exact-byte repair |
| `postgres_notebook_commit_retry_and_cleanup_use_the_retained_helix_projection` | `--ignored` | 1 passed: nonempty canonical save, graph publication failure, same-revision retry, private ownership and cleanup |
| `deletion_worker_resumes_after_acceptance_and_after_canonical_purge` | `--ignored` | 1 passed: durable restart recovery and graph verification |
| `live_helix_templates_metadata_dual_vectors_and_deletion` | `--ignored` | 1 passed: metadata, both vector roles and graph deletion |

These runs explicitly exercise eight of the 14 opt-in tests in the broad gate. The disposable API
harness was also run for the HTTP/UI checks below. The notebook visual fixture and four dedicated
Chromium visibility/cancellation/timeout suites were not run; native preview generation itself was
verified through the live API. Real-provider paid smoke tests, the independent legacy Node server,
packaged desktop execution and interactive native desktop flows were not exercised. Tauri Rust
was unchanged, so its separate Rust gate was not required. Both shared client targets were built
and their contract/boundary tests passed.

## Lexical parity and query plans

Template keyword retrieval retains the separate path formerly implemented with FTS5; document
keyword/vector/hybrid search remains in Helix. All 27 captured FTS5 query/scope rankings and 14
Unicode 6.1 tokenizer cases pass. Fixtures include technical identifiers, punctuation, diacritics,
prefix AND/OR, stable ties, owner/app isolation and a corpus containing 300 empty documents.
The PostgreSQL implementation preserves the weighted BM25 formula and corpus-wide statistics.

`cargo test --locked --lib query_plans_use_indexes -- --nocapture` passed with 10,000-row
fixtures and `EXPLAIN (ANALYZE, BUFFERS)`. PostgreSQL selected:

| Operation | Selected index |
| --- | --- |
| Scoped authorization join | `idx_deal_memberships_user`, then `deals_pkey` |
| Exact current-content claim | `quarry_current_content_pkey` |
| Activity cursor and bounded ordering | `idx_deal_activity_cursor` backward scan |
| Durable queued-job claim | `idx_notebook_jobs_queue`, then a bounded candidate sort |

`cargo test --locked --lib lexical_order_matches_captured -- --nocapture` also passed. With
10,000 unrelated postings, the selective prefix uses `template_lexical_terms_pkey` range bounds;
corpus statistics use their singleton primary key. Scope/membership filters precede the result
limit. These are deterministic development fixtures, not production throughput benchmarks.

## Runtime and UI checks

A production-composed API and web UI ran on separate validation ports, using a temporary schema
in `Quarry_test` and the disposable Helix store. Fresh synthetic accounts verified:

- Registration, login/session restoration, logout and revoked-session rejection.
- Company creation/lookup, deal creation/update, membership-based cross-account denial.
- Private notebook creation/save, PostgreSQL persistence and Helix publication.
- Native PPTX template import, private catalog access, generated PNG preview and PPTX export.
- Graceful API restart preserving sessions, deals, notebooks and templates, including startup
  template reconciliation.
- Permanent deal deletion progressing through canonical cleanup and graph verification, with a
  private notebook retained and its deleted deal association cleared.

Browser inspection showed the PostgreSQL-backed deal and saved notebook. No browser error logs
were present. No frontend UI behavior or styling changed; a full theme/accessibility/responsive
review was outside this persistence change. Existing shared-client tests and both builds passed.

The final application binary then initialized the actual empty **`Quarry`** database through normal
bootstrap against `helix-quarry-postgres-dev` on port 6970. API health and the read-only connection
checker passed. Inspection confirmed schema version 1, 42 tables, and zero users, deals or
templates. This API process was stopped gracefully after validation. The new persistent graph and
database remain available for development. The old port-6969 graph and old SQLite files were
left untouched. All task-created validation processes, schemas and the disposable graph volume
were removed; `Quarry_test` remains available for future tests.

## Review and architecture impact

The feature review covered correctness, readability, architecture, security and performance.
The follow-up refactoring review removed the dead relational adapter/reset path, retained one
shared typed query/row/transaction layer, and corrected publication-task documentation. No
additional behavior-preserving abstraction was warranted.

Caller input/authentication checks retain their HTTP owners. Canonical ownership, current-version,
revision, lease, result authorization and deletion checks remain at their transaction or worker
point of use because they require current loaded state. Configuration validates connection bounds;
bootstrap alone constructs the pool and retains schema-scoped writer ownership. CPU/native
conversion remains off Tokio workers. SQL values are bound and database credentials/errors remain
sanitized. No unresolved required review finding remains.

Architecture/state and performance were inspected through code, real PostgreSQL constraints,
concurrency/failure tests and query plans. Accessibility/interaction and visual consistency were
unchanged; browser inspection was limited to the migrated data flows described above. Verification
coverage and unexercised environments are recorded explicitly in this document.

Architecture impact: updated `docs/ARCHITECTURE.md`, storage/recovery guidance, domain and API
notes, onboarding, `AGENTS.md` and affected project skills for PostgreSQL ownership, fresh schema,
async I/O, retained Helix, pool/writer behavior, configuration, launcher defaults and test isolation.
The final architecture sections were re-read after implementation. Historical `sqliteCommitted`
and deletion phase `sqlite` values remain for API compatibility and now mean PostgreSQL commit
and canonical cleanup. Canonical PostgreSQL commits still precede Helix publication; there is no
cross-store atomicity, data migration or general reindex command.

An earlier test run passed its assertions but failed process-exit cleanup after its running binary
was replaced by a concurrent build; Postgres.app refused that process's new connections. The final
backend gates and cross-store runs were serialized and exited successfully. No pre-existing gate
failure remains. Standard Vite large-chunk notices remain informational; repository bundle budgets
passed.


## Launcher and local vector-index follow-up — November 11, 2026

The launcher now starts an existing Postgres.app 18 cluster when stopped, verifies PostgreSQL
connectivity, starts/reuses the configured durable Helix container, and supervises only Axum and
the selected web or desktop UI. It pins embedded templates and no longer manages the legacy Node
service. PostgreSQL, Docker and Helix normally remain running when the launcher exits.

An initial normal startup exposed an existing configuration/index mismatch: the local configured
`text-embedding-3-large` model requested 3072 dimensions while the document/notebook vector indexes
were created at 1536. Both targets first passed with a temporary 1536 override; that did not change
saved configuration. The user then explicitly requested changing the Helix size to 3072.

Before that operation, with application writers stopped, inspection confirmed zero canonical
document versions, notebook notes and notebook save jobs, and zero Helix `FileChunk`,
`QuarryNotebookChunk` and `QuarryNotebookRevision` nodes. Only the two empty vector definitions
were dropped/recreated at 3072, retaining cosine distance and their `deal_id`/`owner_id` partitions.
Every drop/build operation reached `succeeded`. No canonical records, unrelated template indexes,
Helix nodes, containers or volumes were removed. No provider call or re-embedding was needed.
The one-off maintenance source was removed afterward; it is not a product maintenance command.

Both `./quarry web` and `./quarry desktop` then passed normal startup with the existing 3072
configuration and no dimension override. The native `quarry-desktop` executable launched, and
`/api/v1/health`, the internal Vite preview route, and Helix `/healthz` each returned HTTP 200.
The launcher tests also exercise an inherited legacy-mode selection without starting Node.

Follow-up checks:

- `bash -n quarry`: pass.
- `python3 -B -m unittest discover -s tests -p 'test_quarry_launcher.py'`: nine tests pass.
- Updated project skill validation: both skills pass.
- Local Markdown links/anchors in changed documents resolve; `git diff --check` passes.

This follow-up does not rerun the full Rust/frontend gates recorded above or claim new visual,
accessibility, real-provider or populated-index migration coverage. Changes to documentation were
reviewed against configuration, bootstrap, schema, pool, job and template ownership code. Historical
SQLite evidence remains labeled as such instead of being rewritten as PostgreSQL test results.

After the startup checks, owned API/UI processes were stopped. PostgreSQL and both Quarry Helix
containers were returned to the user's requested stopped state; ports 3000, 3001, 1420, 5432,
6969 and 6970 were verified free. This final service state supersedes the earlier post-migration
runtime state recorded above. Architecture, onboarding, domain/authentication/recovery guidance,
`AGENTS.md` and affected project guidance reflect the maintained launcher and storage boundaries.

## Read independence and concurrency primitives — November 12, 2026

This is an intermediate implementation. Reads are independent of writer ownership; production
writes still retain the application writer lease, generation fence and schema-wide advisory lock.
The full concurrent-writer/multi-instance transition is not complete.

Read transaction setup now selects REPEATABLE READ, READ ONLY without invoking the write guard;
successful snapshot commit also skips writer fencing. Single-query reads no longer ping or check
the writer session. Session/resource authorization remains in its existing HTTP/domain owners.
Statement/acquisition bounds, transaction rollback and cancellation behavior are preserved.

Typed queries now distinguish locking SELECT and mutation RETURNING from ordinary SELECT.
Explicit transaction APIs support FOR SHARE/FOR UPDATE, SKIP LOCKED, returned conditional edits,
and integer epoch deadlines calculated from PostgreSQL's clock with bound offsets. Read runners
and READ ONLY transactions reject the new write-capable operations. Generated INSERT IDs are
classified explicitly instead of inferred from an SQL suffix.

The four initial read regressions failed on the old implementation, then passed after decoupling.
The expanded client suite has 23 passing tests, covering all read entrypoints with the writer
mutex held, lost ownership/missing generation metadata, takeover during a stable snapshot,
independent-pool shutdown, paused writes, cancellation and bounded/closed pools. Independent-pool
SQL tests use a test-only transaction harness without the global guard. They prove shared-lock
overlap, exclusive-lock waiting, SKIP LOCKED selection, a single conditional-update winner,
unrelated-resource commits, database deadlines and rollback after a mapper failure. They do not
prove that production domain workflows are ready for concurrent writers.

The verification database is a fresh task-owned PostgreSQL 18 cluster on loopback port 15432,
with database `Quarry_test` and randomly isolated test schemas. No real configuration, development
database, existing graph, provider or application process is used. The existing ten client tests
passed before implementation. The first restricted-network invocation could not connect; rerunning
with local-network access passed. No pre-existing product test failure was found.
The final run left zero task test schemas. The task-owned PostgreSQL process was stopped and port
15432 was verified free; the temporary cluster was removed. Existing services stayed untouched.

Checks from `backend/`:

| Command | Outcome |
| --- | --- |
| `cargo test --locked --lib adapters::postgres::query::tests` | 19 passed |
| `QUARRY_TEST_DATABASE_URL=postgresql://127.0.0.1:15432/Quarry_test cargo test --locked --lib adapters::postgres::client -- --nocapture` | 23 passed |
| `cargo fmt --all -- --check` | Passed |
| `cargo check --locked --all-targets` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `QUARRY_TEST_DATABASE_URL=postgresql://127.0.0.1:15432/Quarry_test cargo test --locked --all-targets` | 526 passed, 14 opt-in tests ignored |

`git diff --check` passes. The Python skill validator could not import PyYAML; Ruby's YAML parser
validated the two edited skill frontmatters, with their names/descriptions and links inspected.
Architecture, storage guidance and project instructions were updated for the intermediate state.
The user's pre-existing plan edit was preserved.

Review coverage: transaction/state architecture, query classification, authorization preservation,
resource lock scope, rollback, cancellation, parameter binding and verification were inspected.
The refactoring review kept shared transaction completion/row mapping small and retained the
existing assignment/parameter order. No further behavior-preserving refactor was warranted.
Accessibility, interaction and visual consistency are not applicable to this backend-only slice;
frontend/Tauri builds and manual UI inspection were not run. The 14 opt-in browser/live-graph and
interactive fixtures were not run. No new unreachable production code was identified; the new
resource primitives are currently consumed by tests, ahead of domain adoption.

Remaining implementation/acceptance gates: baseline latency/throughput/queue measurements,
data-preserving schema migration, lifecycle guards and public revision contracts across all
clients, domain CAS/row-lock adoption, durable job admission/claims/projection intent, shared
replica limits, remote publication/delete fencing, two-process recovery, global write-control
removal, drained cutover and post-cutover measurements. No production throughput or distributed
safety claim follows from the primitive tests. The requested full plan remains incomplete.

## Fresh schema, revision APIs and domain coordination — November 12, 2026

This section supersedes the pending schema/revision/domain items in the preceding intermediate
record. At the user's direction, schema 2 replaces the fresh schema source without preserving old
data. Startup initializes only an empty selected schema or validates version 2; it rejects version 1
and unknown/drifted layouts without deleting records. No development database or `.env` was opened.
The original November 11 and earlier November 12 results above remain historical evidence.

Schema 2 adds safe-integer company/template/deal-content/metadata revisions and permanent per-deal
lifecycle guards. The schema initialization lock is distinct from runtime mutation locking. Lexical
corpus statistics now use exact snapshot aggregates; the shared statistics row/trigger is gone.
Canonical replacement/delete predicates check submitted revisions and affected counts. Authority
changes and protected deal writes coordinate on exclusive/shared lifecycle rows; notebooks, folders,
assistant threads and templates coordinate on their own parent rows. Providers/Helix remain outside
SQL transactions. Web and desktop clients send captured revisions, keep conflicting drafts and
require review after destructive conflicts. The native gateway validates revision transport shape.

New independent-pool domain tests shadow only the runtime advisory function in their generated test
schema (after real migration validation). There is no production setting or bypass. They exercise
same-revision deal/metadata/company/template races, atomic activity rollback, unrelated deal/notebook
progress during a blocked resource, membership no-ops/revocation, stale template deletion, revision
exhaustion, stale deletion barrier rollback and permanent identity reservation after purge. Existing
notebook job, authentication/revocation, template lexical parity and recovery suites continue to pass.
A notebook relationship-filter regression also verifies that this read does not acquire a row lock.

### Final automated gates

Commands run from their named build roots. The backend test URL selected the task-owned loopback
PostgreSQL 18 instance, database `Quarry_test`, with isolated schemas; it never selected `Quarry`.

| Root | Command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `backend/` | `QUARRY_TEST_DATABASE_URL=postgresql://127.0.0.1:15432/Quarry_test cargo test --locked --all-targets -- --test-threads=6` | 536 passed, 14 opt-in tests ignored; all binary targets passed |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test` | 99 files, 652 tests passed |
| `frontend/` | `npm run build:web` | Passed |
| `frontend/` | `npm run check:web-bundle` | Passed after web build |
| `frontend/` | `npm run check:bundle-size:web` | Passed; startup 346,704 / 350,000 bytes |
| `frontend/` | `npm run check:notebook-bundle:web` | Production notebook initialized |
| `frontend/` | `npm run build:desktop-ui` | Passed |
| `frontend/` | `npm run check:bundle-size:desktop` | Passed; startup 338,512 / 350,000 bytes |
| `frontend/` | `npm run check:notebook-bundle:desktop` | Production notebook initialized |
| `frontend/src-tauri/` | `cargo fmt --all -- --check` | Passed |
| `frontend/src-tauri/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `frontend/src-tauri/` | `cargo test --locked --all-targets` | 26 passed; binary target passed |
| Repository | `git diff --check` | Passed |

The initial restricted native test run could not bind its local mock servers; the approved local
network run passed. One frontend lazy-login test timed out under simultaneous builds/tests and
passed both alone and in the final full run. Writer takeover fixtures exposed a signal-delivery
race: PostgreSQL termination now waits for the session to exit before asserting takeover. Product
revision-contract failures found during implementation were corrected and the final full suites
passed. No unresolved pre-existing product failure is known. The Python skill validator could not
import PyYAML; Ruby validated the two modified skill frontmatters instead.

### Browser check, review and limits

The existing opt-in notebook visual fixture ran with fake embeddings/Helix and an isolated schema
on port 39001. A separate temporary Vite configuration proxied port 1427 to it, without editing
project configuration. Browser checks covered sign-in, empty notebook, note creation, save dialog,
saved feedback, a second saved edit, and opening/cancelling the delete dialog. Escape restored focus
to Delete note. The saved-note view was visually inspected. Conflict retention is covered by
source/route/component and independent-pool tests; the browser check did not establish a two-tab
conflict. Dark mode, reduced motion, mobile viewports, a screen reader and packaged desktop UI were
not manually inspected. Existing controls/motion/styles remain in use; no new layout was introduced.
The 14 opt-in live-graph/browser fixtures were excluded from the full gate; one notebook fixture was
run separately for this browser check. No live provider or graph was contacted.

Review coverage: architecture/state, handler revision bounds and transaction guard placement,
authorization/disclosure, rollback, parent lock order, captured client revisions, draft retention,
duplicate submission, keyboard/focus feedback, visual consistency, bounded queries and both startup
budgets. The review fixed read-only notebook lock acquisition and kept revision/parent helpers small.
The scoped refactoring review found no further useful behavior-preserving changes. No new dead
production code or dependency/lockfile churn was found. Architecture, storage/API recovery guidance
and project instructions were re-read and updated. The user's pre-existing plan changes remain intact.

Task-owned process identities were recorded: PostgreSQL PID 20892/port 15432, visual fixture PID
44687/port 39001 and final Vite PID 46126/port 1427. The initial temporary Vite PID 44803 was stopped
before its proxy restart. After stopping the visual fixture, its exact schema was identified by the
synthetic note ID and removed. Zero test schemas remained. All task-owned servers were stopped,
ports 15432/39001/1427 were verified free and the temporary cluster was removed. The pre-existing
listener on port 1420 was left untouched.

Remaining broader-plan work: durable multi-instance admission/claims/attempt recovery and projection
intent, remote publication/delete fencing and shared replica limits, two-process recovery evidence,
removal of global writer ownership/generation/advisory controls, drained cutover and comparative
latency/throughput/queue measurements. App/builtin catalog reconciliation and remote template worker
safety still rely on the retained controls; per-template SQL locking is not distributed attempt
fencing. This selected schema/revision/domain stage does not enable multiple application writers.
