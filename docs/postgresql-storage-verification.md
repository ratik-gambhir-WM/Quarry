# PostgreSQL storage verification

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
