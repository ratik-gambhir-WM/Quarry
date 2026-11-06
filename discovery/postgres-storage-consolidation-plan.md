# Consolidate Quarry storage in Postgres

Date: November 6, 2026

Status: proposed implementation plan; no runtime or database changes made

## 1. Objective and hard constraints

Replace Quarry's SQLite persistence and every active Helix storage dependency with one
Quarry database in the user's existing local Postgres instance. Preserve the product's
relational data, file/version/chunk relationships, keyword search, embeddings, notebook
save behavior, and embedded template capabilities.

**Start Postgres with fresh tables and fresh development data. Do not migrate existing
data. No SQLite export/import scripts, Helix extraction scripts, historical backfills,
dual writes, or synchronization between old and new databases are required.** Recreating
the schema and manually creating new users/deals, uploading documents, importing
templates, and saving notes is the intended validation path. Old records do not need to
appear in the Postgres application.

**Leave the existing SQLite and Helix data completely untouched.** The user's follow-up
supersedes the earlier permission to delete development data. Do not reset SQLite, run
`clear_helix`, change graph indexes, remove volumes, change the Helix container, or alter
the old environment configuration. Switching back to `main` must still let the existing
application use its original SQLite and Helix stores. New Postgres records will remain
exclusive to the Postgres branch; they do not need to appear on `main`.

This task produces a plan only. Implementation, provisioning, and runtime testing are
future work. An initial Postgres schema definition is necessary implementation work;
it is not a data migration. Do not recreate the chain of SQLite schema upgrades in
Postgres. Routine future Postgres schema evolution can use normal versioned DDL.

## 2. Current code and scope

The inspected working tree is on `main` and already contains uncommitted backend,
template-ownership, test, and architecture edits. Preserve those edits. Source code and
the current architecture document take precedence over stale skill descriptions.

| Area | Current implementation | Required Postgres destination |
| --- | --- | --- |
| Product/auth data | Shared SQLite, schema marker 17 | Equivalent relational tables and constraints |
| Original documents | SQLite logical files, immutable versions, BLOBs, identity manifests, image checkpoints | Same aggregate with `bytea` for original bytes |
| Document retrieval | Helix `QuarryFile`, `FileVersion`, `FileChunk`, version/current/chunk edges | Foreign keys, publication generations, chunk text and vectors |
| Notebooks | SQLite notes and durable save jobs; Helix immutable revision/chunk projections | Canonical notes/jobs plus relational revision/chunk/vector storage |
| Embedded templates | SQLite catalog, assets, previews, classification jobs, two embedding artifacts, FTS5; Helix metadata/vector projection | One canonical catalog with full-text and vector retrieval |
| Document jobs/caches | Process-local memory | Remain process-local; consolidation does not make them durable |
| Legacy template service | Independent optional Node boundary | Keep separate; do not turn it into another Postgres data owner |

The normal bootstrap requires Helix today. Replacing only the SQLite adapter or only
document graph queries would leave the migration incomplete.

### Local Postgres discovery

Postgres.app's PostgreSQL 18.6 **client** is installed; `psql` is not on this shell's PATH.
Read-only readiness checks at the default Unix socket and `127.0.0.1:5432` returned
`no response`. A server connection and extension availability could not be verified.
This does not establish that the user's server is absent: the endpoint may differ or
be inaccessible from this execution environment. No service was started or changed,
no `.env` was read, and no database contents were inspected.

Implementation must first identify the already-running instance's actual connection
endpoint, server version, access role, and available `vector` extension. Reuse that
instance; do not install/start a second server or replace its data directory. Once
connected, inspect only server/extension metadata and the new Quarry target.

## 3. Target architecture

```text
Shared React web / Tauri desktop
              |
       existing /api/v1
              |
        Axum domain services
              |
       domain repositories
              |
       shared async PgPool
              |
  existing local Postgres instance
     dedicated Quarry database
     - relational product tables
     - original bytes / assets / previews
     - document generations and chunks
     - notebook revisions and chunks
     - template classification and vectors
     - full-text indexes and pgvector
```

Use one dedicated Quarry development database on the existing instance, with a
separately named disposable test database on that same instance. Do not use unrelated
application tables or drop/recreate an existing shared database. Keep database access
server-side. React and Tauri continue to call Axum; neither receives Postgres credentials.
Keep `/api/v1`, camelCase JSON, statuses, PDF/multipart limits, SSE names, bearer sessions,
and operation-specific authorization stable.

Recommended Rust integration: SQLx's async Postgres driver and one bounded pool created
by bootstrap. Select and pin a release compatible with Rust 1.94, Tokio, and the chosen
pgvector Rust binding during implementation; do not assume the newest releases are
compatible. SQLx supports Tokio and Postgres ([official SQLx documentation](https://docs.rs/sqlx/latest/sqlx/)).
Keep typed, parameterized queries in the adapter/repository boundary, through an owned
query module/builder rather than scattered SQL in handlers/services. Replace SQLite's
connection mutex and blocking database closures with async transactions. Continue
offloading filesystem, parsing, image, and Office work.

Do not create a generic multi-database framework. The completed branch uses Postgres
only; Git branch isolation preserves the older architecture.

## 4. Recreate the SQLite schema's final semantics

Use the **final marker-17 layout**, including the alterations after the base schema
constants. In particular, the final `quarry_files` has deal scope without the old
`workspace_id`; versions include `uploaded_by_user_id`; classification is named
`template_classification_status`; templates include `owner_user_id`. The temporary
`quarry_files_deal_scope` and historical `slide_classifications` names are not additional
target tables. Create tables in dependency order: SQLite's permissive forward references
cannot simply be pasted into Postgres.

### Table coverage checklist

| Group | Tables to account for |
| --- | --- |
| Metadata and identity | `app_metadata`, `users`, `auth_sessions` |
| Assistant | `assistant_threads`, `assistant_messages` |
| Existing inactive feature | `reminders` — retain the table without inventing a service |
| Engagements/companies | `deals`, `companies`, `deal_parties`, `deal_scopes`, `deal_metadata` |
| Authorization/activity | `deal_memberships`, `deal_access_revisions`, `deal_teams`, `deal_team_memberships`, `deal_activity_events` |
| Documents | `quarry_files`, `quarry_file_versions`, `quarry_file_blobs`, `quarry_current_content` |
| Document integrity | `document_identity_manifests`, `document_chunk_identities`, `document_image_recovery` |
| Notebooks | `notebooks`, `notebook_folders`, `notebook_notes`, `notebook_save_jobs`, `notebook_cleanup` |
| Templates | `templates`, `template_assets`, `template_metadata`, `template_previews`, `apps`, `app_templates`, `template_classification_status` |
| Template lexical retrieval | Replace the virtual `slide_classifications_fts` table with a Postgres-owned search row/vector and GIN index |

Account for every field, default, foreign key, unique/partial index, check constraint,
and delete rule. Do not copy SQLite's implementation details mechanically:

- `INTEGER PRIMARY KEY AUTOINCREMENT` becomes generated identity with `RETURNING id`;
  keep user IDs as the existing API's numeric identifiers and enforce its safe bounds.
  Keep other application IDs as text initially; they include hashes/derived strings,
  so blanket conversion to UUID would break identity contracts.
- BLOB becomes `bytea`; boolean flags become `boolean`; revision/size counters use
  appropriately bounded integer types. Keep the existing JavaScript-safe revision limit.
- Structured JSON becomes `jsonb`, with equivalent object/array checks. Keep API models
  unchanged and verify any checksum/fingerprint derived from JSON: database formatting
  or key order must not redefine application hashes.
- Use `timestamptz` for instants and `date` for date-only fields, with explicit API
  formatting. Convert epoch-based session/lease values at the repository boundary;
  never implicitly depend on server timezone or change expiry semantics.
- Port `?` parameters, SQLite conflict clauses, insert IDs, JSON functions, PRAGMAs,
  schema inspection, text collation, FTS syntax, and row decoding deliberately.
  Retain current case/normalization rules for usernames, email, teams, and catalog sort.
- Preserve partial uniqueness for one current file version, at most one owner membership,
  one active notebook save, and current content claims. Exactly one deal owner still
  requires transactional workflow enforcement; a partial index alone only guarantees
  at most one. Preserve same-deal composite foreign keys and deferred notebook-folder
  checks, and enforce the existing one-team creation rule under concurrency.
- Replace `PRAGMA user_version` with a Postgres schema version/checksum mechanism.
  Bootstrap may initialize an explicitly selected empty Quarry database and validate
  later starts; it must never silently drop tables or reset an incompatible schema.

Keep the identity/image tables initially to minimize behavioral churn. Replace their
Helix-specific readiness flag with Postgres publication validation; no old scope
checkpoint or SQLite-to-Helix migration marker should gate a fresh database. Document
any field retired as obsolete rather than carrying a false recovery contract forward.

## 5. Replace the Helix document graph with relational storage

No graph extension is needed for the implemented relationships. `file_id` foreign keys
replace `HAS_VERSION`; a current-version marker replaces `CURRENT_VERSION`; a
version/generation foreign key replaces `HAS_CHUNK`.

### Additional document storage

| Proposed relation | Required information and constraints |
| --- | --- |
| `document_index_generations` | Version, generation, expected chunk count, embedding model/dimensions, indexed timestamp and publication state; unique `(version_id, generation)` |
| `document_chunks` | Exact chunk ID, version/generation, ordinal, text/hash, token count, page range, character range, section path, created time, embedding and lexical search vector |

Use `quarry_files` and `quarry_file_versions` for display name, canonical deal, MIME,
content hash, byte size, uploader and original filename; do not duplicate a second
canonical file/version graph. Chunk ownership derives through the version/file/deal
join. If deal/file scope is denormalized for performance, enforce it with composite
foreign keys rather than trusting caller-supplied columns.

Require unique `(version_id, generation, chunk_index)` and exact manifest/chunk agreement.
Preserve every field exposed by current `FileChunkResult`, deterministic derivation,
stable content hashes and original-byte retrieval. The active generation must reference
a complete published generation belonging to the same version. Keep historical versions
and generations queryable through the existing authorized readers, while ordinary
search selects only current, live files and active published generations.

### Ingestion and publication transaction

1. Authorize and parse the input using current format/chunking rules. Resolve exact-byte
   deduplication in the same deal and prepare deterministic identities, chunk text and
   provider embeddings before acquiring a long-lived database transaction.
2. Open a bounded transaction. Recheck membership capability/archive state, serialize
   competing writes to the same logical file/content claim, and validate the prepared
   version/generation against current state. Use database uniqueness and appropriate
   row locks, not only process-local upload locks.
3. Persist the file/version/blob, identity and image checkpoints, complete generation,
   all chunk rows/vectors, and upload activity in that transaction. Batch inserts within
   the transaction for parameter/memory limits. Verify counts, ordinals, hashes, finite
   nonzero vectors and dimensions before publication.
4. Atomically advance the current-version/content claim and active generation, then
   commit. Readers see either the previous complete state or the new complete state.
   A failed chunk batch rolls back the whole aggregate. No provider/Office/network
   calls occur while holding database locks.
5. Concurrent identical uploads converge on one committed identity; a loser rereads the
   winner and returns the existing result. Retry deadlocks/serialization conflicts only
   with bounded, idempotent transaction retries. Do not blindly replay provider calls.

This removes the current SQLite-commit/Helix-failure split. Preserve public error/DTO
compatibility where fields mention `sqliteCommitted`; keep any legacy field documented
and deliberately mapped until a separate contract change, rather than leaking new
storage terminology into the API by accident. Accepted image text remains immutable
and reusable; no re-description is needed to reconstruct an already stored generation.

**Version behavior stays the same:** changed-content uploads currently create a new
logical file unless a caller explicitly supplies the earlier file identity. Supporting
version history in Postgres does not implicitly add filename-based version matching.

## 6. Keyword and embedding search

### Lexical retrieval

Use Postgres `tsvector` plus GIN, safe bound query construction, and `ts_rank_cd` for
lexical ranking ([Postgres index documentation](https://www.postgresql.org/docs/current/textsearch-indexes.html),
[query/ranking documentation](https://www.postgresql.org/docs/current/textsearch-controls.html)).
Choose an explicit text-search configuration shared by indexing and queries. Start with
`simple` for technical terms; evaluate stemming and accent handling against Quarry's
retrieval tests. Preserve template AND/OR, punctuation, identifier and diacritic behavior
deliberately instead of passing SQLite `MATCH` expressions into Postgres. `pg_trgm` or
`unaccent` is optional only if those tests establish a need; no BM25 extension is required
for the initial keyword capability.

FTS5/Helix scores and Postgres scores are not numerically equivalent. Preserve meaningful
matches, current/live scope, deterministic tie breaks, result shape and bounded behavior;
review ranking changes with test queries. Keep the existing per-deal keyword fusion
policy for the first pass rather than also redesigning global ranking. Combined
keyword/vector search can reuse existing fusion where applicable; do not add a new
public hybrid endpoint as part of storage replacement.

### Semantic retrieval

Use the `vector` extension and cosine distance, initially with exact scoped ranking.
The current default is 1,536 dimensions. Store model/dimension identity and reject
incompatible, nonfinite, or zero-length vectors. Keep document/notebook and template
embedding settings independent; do not compare vectors from different model spaces.
Postgres stores/searches embeddings; the configured AI provider still generates them.

Exact search supplies a correctness baseline. Add HNSW only after representative
measurements; it changes recall. `vector` HNSW indexes support up to 2,000 dimensions,
so larger configured embeddings require an explicit alternative. Filtered approximate
search can underfill results; evaluate iterative scans and an exact fallback before
enabling it. These constraints are documented by [pgvector](https://github.com/pgvector/pgvector).
Do not silently truncate embeddings or lower precision to fit an index.

### Authorization and search completeness

- Derive actor identity from the bearer session. Join membership, deal, live file,
  current version and complete active generation **before ranking/limiting**. Global
  requests search only that actor's accessible deals; keep the current 200-deal and
  100-result limits initially.
- Preserve the final authorization/current-state check before exposing text when access
  or publication changes during a request. Use a defined transaction/snapshot policy;
  consolidating databases does not by itself solve concurrent permission revocation.
- Replace cross-store projection verification with relational completeness checks;
  incomplete committed generations fail explicitly instead of silently hiding results.
- Preserve chunk/page/character references for preview navigation. Continue enforcing
  private template ownership/app scope and private notebook ownership independently.
- Verify query plans on authorized subsets; do not fetch all vectors/text into Rust
  for scoring or rely on an unbounded list of chunk IDs.

## 7. Cover notebooks and templates completely

### Notebooks

Replace `QuarryNotebookRevision` and `QuarryNotebookChunk` with proposed
`notebook_index_revisions` and `notebook_chunks` tables. Store note ID, immutable accepted
revision, content/pipeline hash, Markdown/chunk text, ordinal, model/dimensions and vectors.
Enforce parent/revision relationships and owner access through the notebook. A deal-linked
private notebook must not become readable to every deal member.

Keep durable save jobs, immutable accepted snapshots, operation idempotency, optimistic
revisions, retries, lease tokens and SSE sequence semantics. Use atomic job claims and
conditional completion updates to prevent an expired worker or stale revision from
publishing over a newer save. Provider work stays outside transactions. Publish a complete
revision and its job checkpoint atomically; preserve the current behavior where canonical
note saving can succeed before embedding/indexing completes. Database consolidation
does not make the external embedding call transactional.

Initially retain `notebook_cleanup` bookkeeping to avoid mixing a worker redesign into
the port, but make its target Postgres revision/chunk deletion. Same-database cascades
can make that path trivial; remove obsolete Helix-only tombstone/reconciliation code
after tests establish equivalent deletion semantics. Do not invent a notebook/global
search feature that the current product does not expose.

### Embedded templates

Keep template JSON, metadata, owner, app memberships, assets and previews canonical in
Postgres. Replace `SlideTemplate`, `SlideTemplatePart`, `SlideTemplateSubject` and
`SlideTemplateCapability` projections with direct canonical reads plus lexical fields
and subject/capability vectors. Helix's 2 MiB request partitioning does not require a new
Postgres parts table; preserve the full logical content without transport fragmentation.

Use typed vectors for the two classification embedding artifacts, with their existing
documents, fingerprints, model/dimensions, retry state and leases. Keep DTO values and
numeric precision behavior deliberate when replacing serialized SQLite BLOBs. Publish
classification/search rows transactionally; only matching ready fingerprints are
retrievable. Retain asynchronous classification/embedding, owner-scoped facade behavior,
built-in visibility/deletion choices and app scoping. Eliminate startup Helix reconciliation.

The optional external Diligence Studio service remains an independent boundary. Keep
its PowerPoint/template protocol and fail-closed ownership behavior intact. It must not
become a hidden SQLite fallback for the Postgres-backed product or require touching its
local data/configuration. Embedded mode is the consolidation acceptance target.

## 8. Preserve `main` through branch and configuration isolation

1. Implement on a dedicated `codex/postgres-storage` branch, preferably a separate
   worktree. Current uncommitted work will not automatically appear in a new worktree;
   establish the intended reviewed baseline without discarding, moving, or accidentally
   committing unrelated work. This plan does not create a branch or commit user edits.
2. Leave the existing checkout's `backend/.env`, SQLite files, Helix configuration,
   container image/name, graph data and volumes unchanged. In a separate worktree,
   resolve any old relative paths carefully; never launch the old bootstrap against a
   newly inferred SQLite path as a compatibility test.
3. Add a new server-only `QUARRY_DATABASE_URL` and bounded pool settings for Postgres.
   Supply credentials only to the Postgres process/session or its separately configured
   worktree. Do not rename/reuse `QUARRY_DATABASE_PATH`, rewrite the shared ignored
   `.env`, export Postgres values globally, or alter persistent shell settings.
4. Remove the embedding dimension's dependency on `HELIX_VECTOR_DIMENSION` on the new
   branch. Introduce storage-independent document/notebook configuration, while keeping
   the existing independent slide embedding settings. Validate schema/model dimensions
   at startup and on every provider result.
5. Change the Postgres branch's launcher to check the selected Postgres endpoint and
   start Axum/UI without Docker/Helix setup. It must not inspect/reconfigure/start/stop
   the old Helix container. Switching to `main` restores the old tracked launcher.
   Leave unrelated Docker workloads and the existing Postgres server running.
6. Treat ignored runtime files as shared local state, not branch-managed files. PostgreSQL
   schema startup/reset code must have no path that can open SQLite or call Helix.
   Missing Postgres configuration fails clearly; no fallback to the old stores.
7. Before switching branches, stop only the branch's Axum/UI/worker processes so an old
   process cannot keep serving the wrong backend on the same ports. Reload the app and
   sign in again: the browser's existing bearer cache belongs to a different database.
   Preserve existing 401/session-expiry handling; a storage migration does not justify
   changing the public session contract or clearing unrelated browser state.
8. After the port is complete, remove SQLite/Helix code, dependencies, query DSL and
   destructive maintenance binaries **from the Postgres branch only**. Do not remove
   their local data directories or infrastructure. Git restores tracked code on `main`.

No dual-backend runtime switch or data rollback tooling is needed. Returning to `main`
uses its untouched old stores; returning to the Postgres branch uses its new stores.
Compatibility means the old environment is still available, not that new Postgres data
is copied back. A real `main` runtime test may itself write sessions/index metadata, so
under the current no-touch instruction verify compatibility by isolation and code/config
review; do not start the legacy app merely to prove it still works.

### Required transaction contract: atomic and idempotent

Apply this contract to **every Postgres mutation**, including domain writes, ingestion,
classification publication, save jobs, retries and cleanup. Atomicity and idempotency
are separate requirements: using a transaction does not automatically make retries safe.

- **Atomicity:** define each use case's commit boundary. All related rows, authorization
  checks, revisions, activity events and durable job/result checkpoints within that
  boundary use one transaction on one connection. Any failure rolls back that entire
  boundary. Repository helpers must accept the caller's transaction rather than commit
  independently. Multi-stage AI jobs may have several explicit commits, but each stage
  is atomic and incomplete output cannot be published as complete.
- **Stable operation identity:** reuse existing request/operation IDs and natural unique
  keys where they identify the intended operation. For creates or other non-repeatable
  commands without one, add an actor/operation-scoped idempotency key and canonical
  request fingerprint. A proposed `mutation_receipts` table can record the scoped key,
  fingerprint and committed result in the same transaction as the mutation. Use it only
  where existing tables cannot supply that durable receipt. Never identify an operation
  by payload hash alone: two deliberately separate creates may have identical payloads.
- **Safe replay:** repeating the same key and fingerprint returns the original committed
  identifiers/result without duplicating rows, activity, jobs or revision increments.
  Reusing the key with different input returns a conflict. Reauthorize the actor before
  returning a receipt; a stored result does not bypass revoked access. Concurrent requests
  for the same key must converge through database uniqueness/locking. Keep receipt metadata
  minimal and secret-free, and define its retention/replay guarantee explicitly; never
  expire receipts while claiming unlimited duplicate protection.
- **Uncertain commit:** a lost connection during commit does not prove rollback. Resolve
  the outcome through the same durable operation identity on a fresh connection; never
  retry with a new identity. If the outcome cannot yet be determined, report a retryable
  failure that preserves that identity. Retry known serialization/deadlock rollbacks as
  whole transactions with bounded backoff and stable precomputed IDs. Do not retry
  validation, authorization or unrelated uniqueness errors as transient failures.
- **Concurrency:** use uniqueness, row locks in a consistent order, conditional revision
  updates and worker lease fencing as appropriate. Choose/document isolation per use case;
  use serializable transactions with bounded retries when row locks and constraints do
  not protect an invariant. Process-local locks are not the correctness boundary.
- **External effects:** database transactions cannot roll back provider calls, file writes
  or delivered SSE events. Keep provider work outside database transactions, checkpoint
  accepted results, and publish durable status before notifying clients. Preserve existing
  restart/replay behavior without claiming exactly-once provider execution. Add a durable
  dispatch record only if a workflow requires recoverable external delivery.

If stable operation keys require API additions, coordinate the TypeScript contract, web
and desktop adapters, Tauri relay if involved, handlers and contract tests. Preserve
existing request IDs and SSE names; do not silently change required request fields.
Include any new receipt relation in the initial Postgres schema and verification scope.

## 9. Implementation sequence and exit gates

| Step | Work | Exit evidence |
| --- | --- | --- |
| 1. Establish isolation | Agree the code baseline, create implementation branch/worktree, identify existing Postgres endpoint and dedicated Quarry/test database names | Old stores/config/services unchanged; server and extension metadata confirmed |
| 2. Add Postgres foundation | Async adapter/pool, redacted config, initial complete schema, version validation, disposable test support | Empty init, repeat startup and incompatible-layout rejection tests pass |
| 3. Port relational domains | Users/sessions, companies/deals, memberships/access/activity, assistant, notebooks/jobs, template catalog/classification persistence | Domain and route tests run against Postgres with equivalent transactions and constraints |
| 4. Port documents | Blob/store/identity access, generation/chunk storage, ingestion, current/historical readers, previews, keyword/vector search | Format matrix, atomic publication, deduplication, authorization and search tests pass |
| 5. Finish Helix replacement | Notebook revision pipeline, both template vector roles, lexical retrieval, worker deletion/retry paths | All active Helix consumers eliminated; fresh save/import/retrieval paths work |
| 6. Switch bootstrap/launcher | Inject only PgPool, remove old startup requirements and branch-only dead adapters/binaries | Web and desktop launch against Postgres without contacting old stores/Helix |
| 7. Verify and document | Full gates, manual flows, query-plan review, architecture/config/recovery documentation | Acceptance checklist below passes; diff contains no local data/config changes |

Intermediate commits may compile only with explicitly gated construction paths, but
the final branch must not retain a mixed SQLite/Postgres or Helix/Postgres deployment.
Use a fresh Postgres development dataset instead of implementing transitional backfills.

## 10. Verification and acceptance

Convert repository and route test factories from in-memory SQLite to isolated Postgres
databases/schemas in the dedicated **test** database. Assert a test-only target before
any reset, do not drop `public`, and never clean up an arbitrary configured database.
Run concurrent tests with independent namespaces/connections. Tests must exercise real
Postgres constraints, transactions and the installed vector extension, not SQLite mocks.

Required scenarios:

- Create/restart/reuse fresh schema; reject incompatible schema; no automatic destructive reset.
- Registration/login/expiry/revocation, numeric IDs, unchanged auth and sanitized errors.
- Deals/company links, ownership transfer, member removal, same-deal teams, archive rules,
  revisions and activity. Race ownership transfer against member removal/upload.
- Assistant message ordering, request idempotency, stream completion/failure/cancellation.
- PDF, DOCX, XLSX, PPTX and image upload/preview; original bytes unchanged; exact duplicates,
  concurrent duplicates, explicit later versions, deleted files and retained history.
- Inject failure after a chunk batch and immediately before publication; no partially
  searchable version or advanced current pointer survives. Previous published data remains.
- Keyword punctuation/technical terms/diacritics/AND/OR, semantic ranking with deterministic
  test vectors, no-result cases, tie ordering, limits and wrong-dimension/model rejection.
- Two users with distinct and shared deals: inaccessible chunks never appear in either
  search route, including during membership changes; templates and notebooks stay private.
- Notebook save/retry/restart, expired leases, concurrent saves, stale revision publication,
  cancellation/deletion and durable SSE/status checkpoints.
- Template import/list/assets/previews/insertion, owner/app isolation, classification retries,
  both embedding roles, stale fingerprints, restart and delete behavior.
- PgPool exhaustion/database unavailability return bounded, sanitized failures; rollback and
  retry behavior cannot duplicate activity or corrupt current-version claims.
- For every mutation family, inject a mid-transaction failure and assert that no partial
  aggregate, activity event, revision increment or success receipt remains committed.
- Replay an operation sequentially, concurrently and after Axum restart: the committed
  result and side-effect counts remain unchanged. A changed payload under the same key
  conflicts; a distinct key permits an intentionally separate operation where valid.
- Simulate a committed transaction whose response is lost, plus a transaction that truly
  rolled back. Retrying the same operation resolves each without duplicate effects.
- Force serialization/deadlock retries and stale worker completion; verify bounded retries,
  atomic rollback, stable identities, lease fencing and authorization on receipt replay.
- Fresh Postgres dataset survives Axum restart. No SQLite file, legacy Helix endpoint,
  container, volume or ignored legacy configuration is used or modified by the new branch.

From `backend/`, run the existing gates after the focused converted tests:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Keep manually included test modules wired through `src/lib.rs`/owning modules;
`autotests = false`. Remove or replace obsolete Helix SDK/SQLite tests and add architecture
checks that enforce the new adapter boundary. Database-backed suites must actually run
against the explicit disposable target; do not count skipped suites as passing.

Verify unchanged web/desktop contracts from `frontend/`:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

If native relay code changes, also run the documented Tauri Rust gates. Inspect real
web and desktop flows for sign-in, upload/preview/search, notebook saves and templates,
including loading/error/retry behavior. Use fixed test embeddings first; distinguish
optional real-provider smoke tests from deterministic storage verification. Record and
stop only test processes started for this work; leave pre-existing services alone.

Before handoff, run `git diff --check`, review status, and audit branch startup/code for
remaining SQLite/Helix consumers. Review query plans for authorization joins, current
generation lookup, lexical indexes and vector candidate selection using fresh test data.

## 11. Documentation and completion criteria

Update `docs/ARCHITECTURE.md` **when implementation lands**, including backend startup,
concurrency, schema ownership, document consistency, search, notebook/template pipelines,
configuration, launcher behavior, testing and known limitations. Update the relevant
authentication, notebook, shared-deal and template documentation; clearly mark historical
SQLite/Helix rollout procedures as historical on the Postgres branch. Update `AGENTS.md`
and project skills where they otherwise prescribe the removed adapters/checks. Keep this
file a design input rather than a second architecture source of truth.

Done means:

- All active product persistence and former Helix storage use the one Quarry Postgres database.
- New files have durable original bytes, versions, complete chunks and embeddings; authorized
  keyword/semantic search and previews work with the existing client contract.
- Notebooks and embedded templates retain their current behavior and durable worker semantics.
- Every Postgres mutation has an explicit atomic commit boundary and a tested idempotency
  strategy, including concurrent replay and uncertain-commit recovery.
- No data migration, legacy data cleanup, second Postgres server, dual-write system or permanent
  compatibility layer was built.
- The existing SQLite/Helix environment and ignored configuration remain untouched, so `main`
  can continue to use them independently of the fresh Postgres development dataset.
- Focused and broad gates pass with the actual Postgres integration suites enabled; external
  provider/runtime uncertainties and any pre-existing failures are reported separately.

## 12. Source map and planning verification

Repository sources inspected for this plan:

- [Canonical architecture](../docs/ARCHITECTURE.md), especially sections 8–11 and 14.
- [Actual schema and final upgrades](../backend/src/app/migrations.rs).
- [Bootstrap](../backend/src/app/bootstrap.rs), [configuration](../backend/src/app/config.rs),
  [entrypoint](../backend/src/main.rs), [launcher](../quarry).
- [Backend manifest](../backend/Cargo.toml), [lockfile](../backend/Cargo.lock),
  [frontend scripts](../frontend/package.json).
- [SQLite adapter](../backend/src/adapters/sqlite/client.rs),
  [document store](../backend/src/domains/documents/store/sqlite.rs),
  [identity checkpoints](../backend/src/domains/documents/store/identity.rs).
- [Document graph model](../backend/src/domains/documents/graph/model.rs),
  [queries](../backend/src/domains/documents/graph/query.rs),
  [writer](../backend/src/domains/documents/graph/writer.rs),
  [repository](../backend/src/domains/documents/graph/repository.rs),
  [ingestion](../backend/src/domains/documents/ingestion/service.rs),
  [search](../backend/src/domains/documents/search/service.rs).
- [Notebook storage](../backend/src/domains/notebooks/repository.rs),
  [jobs](../backend/src/domains/notebooks/job_repository.rs),
  [projection pipeline](../backend/src/domains/notebooks/graph.rs).
- [Template repository](../backend/src/domains/templates/slide_templates/repository.rs),
  [classification storage](../backend/src/domains/templates/slide_templates/repository/classification.rs),
  [retrieval contracts](../backend/src/domains/templates/slide_templates/retrieval.rs),
  [projection](../backend/src/domains/templates/slide_templates/graph.rs).
- [Integration suites](../backend/tests/integration/) for concrete behavior and porting scope.

External implementation references are linked at the relevant design decisions above.
Local PostgreSQL readiness/extension access remains unverified. This documentation-only
task does not run Rust/frontend builds or mutate any database. Architecture impact:
none for this planning change — it records a proposed future architecture without
changing the current runtime, contracts, data ownership or operational configuration.
