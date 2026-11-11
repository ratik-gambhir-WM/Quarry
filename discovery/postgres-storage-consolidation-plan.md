# Replace SQLite with PostgreSQL; retain Helix

Updated: November 11, 2026

Status: proposed implementation plan; no runtime or database changes made

## 1. Objective and scope

Replace Quarry's shared SQLite persistence with PostgreSQL on the user's existing local
PostgreSQL instance. **Keep Helix as Quarry's graph database and graph/search projection.**
This is a relational storage replacement: recreate the current tables, integrate the
PostgreSQL adapter, and verify that existing functionality and query behavior still work.

The current user request supersedes this document's earlier proposal to eliminate Helix.
Preserve existing routes, DTOs, authorization, queries' meaning and results, file/version
identities, search behavior, notebook saves, embedded templates, and error/retry semantics.
Translate SQLite-specific SQL where PostgreSQL requires it; do not redesign the product,
move graph queries to SQL, or introduce new client contracts as part of this port.

**No existing data needs to be migrated.** Start with a fresh Quarry PostgreSQL database
and fresh users, deals, uploads, notebooks and templates. Do not implement SQLite exports,
Helix extraction, historical backfills, dual relational writes, or data rollback tooling.
Old MVP data may be left in place; deleting it is not required for this work. No databases,
files, containers, volumes, or configuration are changed by this planning task.

Use a fresh, isolated Helix store alongside the fresh PostgreSQL database for implementation
validation. Retaining the Helix technology does not require attaching orphaned old graph
records to an empty canonical database. Prefer leaving the existing SQLite and Helix stores
alone; any optional reset must target explicitly selected disposable resources, never run
implicitly during startup, and never affect another application on the shared instance.

## 2. Current implementation and intended ownership

Reconfirm the implementation baseline before coding; do not assume a branch name or a clean
working tree. The current schema is **marker 18**, owned by `backend/src/app/migrations.rs`.
Read the final schema including its additive upgrades, not just the initial CREATE statements.
The maintained architecture and current code override older planning notes and skill versions.

| Area | Current implementation | Target |
| --- | --- | --- |
| Identity, auth, deals, companies, memberships, activity, assistant | Shared SQLite | Equivalent PostgreSQL tables, constraints and transactions |
| Original documents | SQLite files, immutable versions, BLOBs, current-content claims, identity manifests and accepted image checkpoints | Same canonical records in PostgreSQL; bytes remain database-owned |
| Document graph/search | Helix file/version/chunk nodes, edges, embeddings, keyword/vector queries and publication | Retain Helix, its query contracts and graph identities |
| Notebooks | SQLite notes, durable jobs and cleanup; Helix revision/chunk projection | PostgreSQL canonical notes/jobs/cleanup plus the existing Helix projection |
| Embedded templates | SQLite catalog/assets/previews/classification, canonical embedding artifacts and FTS5; Helix metadata/dual-vector projection | PostgreSQL canonical tables and lexical replacement; retain Helix projection and retrieval |
| Permanent deal deletion | SQLite durable receipts/barriers/cleanup targets; Helix graph purge | PostgreSQL receipts/barriers/targets; retain Helix purge and verification |
| Document jobs/caches | Process-local memory | Remain process-local |
| Optional Diligence Studio service | Independent Node boundary | Remains separate; no service or storage consolidation |

PostgreSQL replaces canonical relational persistence. It does not replace Helix, make
cross-store writes atomic, make in-memory jobs durable, or add new search capabilities.
No pgvector dependency or PostgreSQL graph/chunk/vector-query schema is needed for this swap.
Keep canonical template embedding artifacts in PostgreSQL because they already belong to
SQLite today; this does not move their Helix similarity-search role into PostgreSQL.

## 3. Target architecture and local setup

```text
Shared React web / Tauri desktop
              |
       existing /api/v1
              |
       Axum domain services
              |
       domain repositories
          /          \
 async PostgreSQL    retained Helix adapters
 adapter / pool      graph writers / readers / search
        |                       |
 existing local PostgreSQL      Helix graph database
 dedicated Quarry database      isolated fresh dev/test stores
 - canonical relational rows    - file/version/chunk graph
 - original bytes and assets    - document embeddings/search
 - identity/image checkpoints   - notebook revision/chunk graph
 - notes, jobs and cleanup      - template metadata/dual vectors
 - templates and lexical index
```

Implementation first identifies the existing PostgreSQL endpoint, server version and
access role, then selects a dedicated Quarry development database and a separately named
disposable test database or isolated test schemas. The earlier plan recorded unsuccessful
readiness checks; those are historical observations, not proof that the user's server is
absent. This revision makes no new connection/readiness claim. Do not install another
PostgreSQL server or replace its data directory. Only inspect server metadata and selected
Quarry resources; no unrelated application tables need to be read or changed.

Create one bounded asynchronous PostgreSQL pool in bootstrap and inject narrow repository
handles as today. Select and pin a compatible PostgreSQL driver, such as SQLx, during
implementation against the actual Rust/Tokio versions in `Cargo.toml` and `Cargo.lock`.
Keep parameterized query construction in an owned adapter/query module. Do not scatter SQL
through handlers or services, preserve synchronous SQLite APIs by blocking on async work,
or introduce a permanent multi-database abstraction/fallback.

### Explicit adapter replacement

Replace the product SQLite adapter at `backend/src/adapters/sqlite/` with a PostgreSQL
adapter under `backend/src/adapters/postgres/`. Register the new module in
`backend/src/adapters/mod.rs`, construct its pool in bootstrap, and change every active
product repository and other consumer to use it. This is an adapter replacement, not just
a schema change or a PostgreSQL connection added alongside the existing SQLite client.

Account for every part of the current adapter:

| Existing SQLite component | PostgreSQL replacement responsibility |
| --- | --- |
| `mod.rs` | Register the PostgreSQL adapter and expose only the interfaces its consumers need. |
| `client.rs` | Replace `SqliteClient`, transactions, row/value decoding, write results and errors with async PostgreSQL pool/connection, transaction, decoding and error handling. Preserve transaction ownership and affected-row semantics. |
| `read_pool.rs` | Replace SQLite reader leases and writer coordination with bounded PostgreSQL pool acquisition and appropriate transaction/concurrency controls; retain the lifecycle guarantees consumers need. |
| `query/builder.rs` | Provide parameterized PostgreSQL query construction with validated identifiers and equivalent supported operations; translate SQLite-only conflict, substring and FTS behavior deliberately. |
| `query/statement.rs` and `query/value.rs` | Replace SQLite query and bound-value representations with PostgreSQL-compatible statements, placeholders and typed values. |
| `query/mod.rs` | Expose the PostgreSQL query interfaces and wire their tests into the existing backend test tree. |

The PostgreSQL adapter need not reproduce the same file layout or public types internally,
but it must cover these responsibilities and preserve the repository contracts described
below. Inventory imports and direct SQLite use outside this directory, including migrations,
domain repositories, bootstrap, maintenance utilities and tests; port or retire each one.
Do not retain the SQLite adapter as a runtime fallback. Remove `backend/src/adapters/sqlite/`
and its module registration only after all active product consumers have moved, equivalent
adapter/repository tests and the full backend gates pass against the intended test setup,
and the final runtime has been verified with PostgreSQL and Helix. Remove obsolete SQLite
dependencies and tests only after confirming no supported consumer still needs them.

All database credentials remain server-side and redacted. Web and Tauri still call only
Axum. Preserve `/api/v1`, camelCase JSON, status/error mappings, bearer sessions, ownership,
PDF/MIME/multipart checks, SSE names, operation IDs and preview references.

## 4. Fresh PostgreSQL schema

Create the final marker-18 equivalent directly in dependency order. Do not replay the
SQLite upgrade chain or copy temporary replacement tables into the new schema. Use a
PostgreSQL schema version/checksum mechanism for fresh initialization and later validation;
normal startup must reject an incompatible nonempty layout without deleting records.

### Required table inventory

| Group | Tables to recreate |
| --- | --- |
| Metadata/identity | `app_metadata`, `users`, `auth_sessions` |
| Assistant | `assistant_threads`, `assistant_messages` |
| Inactive feature | `reminders` — retain its table without inventing a service |
| Deals/companies | `deals`, `companies`, `deal_parties`, `deal_scopes`, `deal_metadata` |
| Authorization/activity | `deal_memberships`, `deal_access_revisions`, `deal_teams`, `deal_team_memberships`, `deal_activity_events` |
| Permanent deletion | `deal_deletions`, `deal_deletion_files`, `deal_deletion_threads` |
| Documents | `quarry_files`, `quarry_file_versions`, `quarry_file_blobs`, `quarry_current_content` |
| Document integrity/recovery | `document_identity_manifests`, `document_chunk_identities`, `document_image_recovery` |
| Notebooks | `notebooks`, `notebook_folders`, `notebook_notes`, `notebook_save_jobs`, `notebook_cleanup` |
| Templates | `templates`, `template_assets`, `template_metadata`, `template_previews`, `apps`, `app_templates`, `template_classification_status` |
| Template lexical search | Equivalent indexed search data for `slide_classifications_fts`; PostgreSQL cannot recreate an FTS5 virtual table literally |

Use actual final column definitions: deal-scoped `quarry_files` without the old
`workspace_id`, version uploader attribution, template ownership, classification/embedding
leases and fingerprints, and marker-18 deletion state. `quarry_files_deal_scope` and
`slide_classifications` are historical intermediate names, not extra target tables.

For each table, inventory columns, defaults, nullability, foreign keys, delete rules,
checks, indexes and unique/partial constraints. Map SQLite-specific features deliberately:

- Generated identity plus `RETURNING` replaces autoincrement/last-insert-row-ID. Preserve
  numeric user identifiers and JavaScript-safe bounds. Keep hash/derived application IDs
  as text; do not blanket-convert them to UUID.
- Use `bytea` for original files, assets, previews and serialized embedding artifacts;
  preserve byte-for-byte round trips and the existing embedding encoding/precision.
- Choose integer/boolean and JSON representations to preserve existing decoding and
  predicates. Use `jsonb` only where normalization cannot change fingerprints, exact
  stored representations or round trips; otherwise keep the existing serialized text.
- Preserve existing timestamp/epoch/date semantics and API formatting. Choose native
  timestamp/date types only with explicit boundary conversions; server timezone must not
  change expiry, lease comparisons, cursor order or date-only values.
- Replace `?` placeholders, SQLite conflict/JSON/FTS functions, PRAGMAs, schema inspection,
  row mappers and error-code matching with PostgreSQL equivalents. Preserve sort/case/
  Unicode normalization, NULL ordering, pagination ties and affected-row semantics.
- Preserve one current version, unique current/live content claims, at most one owner,
  same-deal composite foreign keys, notebook folder constraints, one active notebook save,
  team-creation rules and all existing transaction guards. Exactly one owner still needs
  the workflow's transactional checks, not just a partial unique index.
- Keep identity manifests, chunk identities and accepted image checkpoints as the canonical
  authority for validating Helix. Fresh initialization must establish the equivalent fresh
  readiness state through validated schema/setup code; do not fabricate historical
  migration receipts, remove live integrity checks or require old data backfills.

## 5. Repository and transaction port

Port every active SQLite consumer, including auth/session lookup, authorization policy,
blobs/checkpoints, assistant, notebook jobs, embedded templates and deletion workers.
Replace SQLite-specific clients, transaction handles, query builders, row decoding and
errors with typed PostgreSQL equivalents. Preserve service interfaces where practical;
async call-site changes are internal implementation work, not a product redesign.

Keep the current dependency direction: configuration parses environment once; bootstrap
constructs adapters and services; handlers validate/authenticate/authorize transport input;
services orchestrate; repositories own persistence. Database failures map to the existing
repository/service/HTTP error categories with sanitized client messages.

SQLite's single-writer behavior must not be assumed under PostgreSQL concurrency:

- Keep each existing aggregate mutation, access check at point of use, activity event,
  revision update and durable job/receipt checkpoint inside its intended transaction on
  one pooled connection. Helpers must share that transaction rather than commit separately.
- Use constraints, row locks in a consistent order, conditional revisions and worker lease
  fencing to preserve those invariants under concurrent requests. Select/document isolation
  per workflow and add focused concurrency tests.
- Preserve existing operation IDs, natural deduplication keys, retry snapshots and receipts.
  Reconcile an uncertain commit by the existing stable identity; do not blindly replay a
  non-idempotent create or introduce mandatory idempotency keys across unchanged APIs.
  Broader API/idempotency redesign is separate work.
- Bound pool acquisition, queries and any safe transaction retries. Retry only known rolled-
  back transient failures, with stable IDs and bounded backoff; authorization, validation
  and unrelated uniqueness failures are not transient retries.
- Use async database I/O. Continue offloading parsing, Office, image, filesystem and other
  blocking work. Remove SQLite-only writer leases/read-pool mechanics only after reviewing
  the lifecycle protections they supplied; retain equivalent protection where still needed.

## 6. Retain Helix queries and cross-store behavior

Keep Helix SDK/adapters, graph schema, node/edge names, query DSL, graph indexes, embedding
models/dimensions, size bounds, publication rules and query limits. Preserve current document
keyword/vector/hybrid behavior and configured rollout flags. No new PostgreSQL document
chunk tables, graph extension, pgvector search, HNSW index or ranking/fusion redesign is
part of this plan. Only canonical lookups used by graph writers/readers/search change
from SQLite to PostgreSQL.

Document ingestion keeps its existing sequence:

1. Validate authorization, parser output, identities, embeddings, chunk invariants and the
   retained Helix write plan, including bounded request sizes, before canonical persistence.
2. Commit file/version/blob, content claim, identity manifest, accepted image checkpoint and
   upload activity in PostgreSQL. Concurrent identical uploads must converge as today.
3. Run existing Helix setup, bounded chunk writes and final graph publication. Preserve the
   older complete published generation until final publication succeeds.
4. If Helix fails after the PostgreSQL commit, report the committed identity and retain the
   canonical records. Exact-content re-upload must recover the same identity and retry the
   missing projection, including reusing accepted image text rather than redescribing it.

This remains a two-store workflow, not a distributed transaction. Preserve readiness,
manifest/header validation, current/live checks, ownership and final authorization before
serving graph results. Historical versions, chunk/page/character references, content hashes
and filename/version behavior stay unchanged; changed bytes do not acquire new filename-
matching semantics. Do not promise a general bulk reindex tool that does not exist today.

Keep any existing public fields or durable phase values named `sqliteCommitted` or `sqlite`
when needed for contract compatibility, mapping them to canonical PostgreSQL commit/cleanup
semantics and documenting the historical name. Review all such uses; do not falsely report
that PostgreSQL and Helix committed atomically or rename wire values accidentally.

### Notebook and template flows

- Notebook canonical notes, immutable save snapshots, jobs, leases, retry/checkpoint/SSE
  state and cleanup tombstones move to PostgreSQL. Keep `QuarryNotebookRevision` and
  `QuarryNotebookChunk` in Helix, the same revision publication and stale-worker guards,
  private ownership, graph deletion/recovery and saved-but-not-indexed outcomes.
- Template JSON, owner/app memberships, metadata, assets/previews, classification state and
  both canonical embedding artifacts move to PostgreSQL. Keep `SlideTemplate`,
  `SlideTemplatePart`, `SlideTemplateSubject` and `SlideTemplateCapability` in Helix,
  including request partitioning, graph projection/reconciliation, app scoping and dual-
  vector retrieval. Do not eliminate startup Helix reconciliation.
- Port the SQLite-only template FTS5 path behind its existing repository contract. A
  PostgreSQL full-text index may replace its mechanism, but must preserve query parsing,
  field weighting intent, AND/OR, punctuation, technical identifiers, diacritics, owner/app
  filters, limits and stable ordering. Capture existing lexical fixtures/results before
  removal and use them as acceptance criteria. PostgreSQL native ranks are not numerically
  identical to FTS5 BM25: do not silently accept a relevance change; provide a compatible
  ranking/query implementation or explicitly resolve any parity gap before completing the
  port. Document/Helix search remains unchanged.
- Keep permanent deal deletion's durable receipt, admission barrier, drains, canonical
  cleanup, Helix purge, verification and restart recovery. A database swap must not leave
  orphaned graph results visible or mark deletion complete before graph verification.
- The optional external Diligence Studio service stays independent with its existing
  protocol; embedded mode is the primary acceptance target. Do not modify that separate
  server's local data/configuration or turn it into a hidden product SQLite fallback.

## 7. Configuration, bootstrap and launcher

- Use a dedicated `codex/postgres-storage` implementation branch/worktree when implementation
  starts. Establish its baseline without discarding or accidentally committing unrelated
  user work. This planning edit creates no worktree or branch.
- Add server-only `QUARRY_DATABASE_URL` and bounded pool/timeouts through `AppConfig`.
  Redact credentials, reject missing/invalid configuration, initialize/validate only the
  selected Quarry schema, and inject the pool into all former SQLite repositories.
- Replace product `QUARRY_DATABASE_PATH`/SQLite storage configuration in the implemented
  runtime. Do not reuse a path variable as a URL or expose credentials through `VITE_*`,
  Tauri payloads or logs. Local setup is implementation work; do not edit `.env` now.
- **Retain `HELIX_URL`, `HELIX_API_KEY`, `HELIX_VECTOR_DIMENSION`, graph configuration and
  independent template embedding settings.** Normal bootstrap still requires Helix and
  awaits its indexes; PostgreSQL readiness does not replace graph readiness.
- Update `./quarry web` and `./quarry desktop` to verify the selected PostgreSQL connection
  as well as their existing Helix prerequisite. Preserve Docker/Helix supervision and
  shutdown behavior. Reuse the existing PostgreSQL service; do not start a second one or
  stop shared PostgreSQL/Helix services when Axum/UI exits.
- Align fresh PostgreSQL development/test databases with isolated fresh Helix stores and
  use matching configuration. Never point tests at the existing development graph by
  accident or fall back to SQLite after a PostgreSQL connection failure.
- After all relational consumers are ported, remove unused product SQLite dependencies,
  SQLite-only adapters/tests and obsolete SQLite reset tools. Preserve Helix adapters,
  queries, SDK, tests and supported maintenance capabilities. Do not run destructive tools
  as verification or delete old database files/volumes as cleanup.

## 8. Implementation sequence and exit gates

| Step | Work | Exit evidence |
| --- | --- | --- |
| 1. Capture baseline | Inventory final marker-18 schema, SQLite consumers, Helix boundaries and current query/route tests; identify local PostgreSQL and isolated test targets | Complete table/query matrix; no unrelated data/configuration changes |
| 2. Add PostgreSQL foundation | Replace the SQLite adapter responsibilities with a PostgreSQL adapter, driver/pool, redacted config, parameterized query module, initial schema/version validation and disposable test support | Adapter tests, fresh init/restart, incompatible-schema rejection and actual constraint tests pass |
| 3. Port relational repositories | Identity/auth, deals/companies, access/memberships/activity, assistant and deletion state | Equivalent domain/route results, concurrency, transactions and errors |
| 4. Port documents | Canonical blobs, versions, content claims, identity/image checkpoints and graph-facing canonical lookups | Upload/preview/deduplication parity; unchanged Helix writes/search; commit-then-index failure recovery |
| 5. Port notebooks/templates | Notes/jobs/cleanup, catalog/assets/previews/classification/artifacts and FTS equivalent | Same saves/retries/recovery, lexical results and retained Helix projections/retrieval |
| 6. Wire final runtime | PostgreSQL repositories plus retained Helix in bootstrap/config/launchers; remove the SQLite adapter and dead product SQLite code after the adapter replacement gates pass | Both clients run against PostgreSQL + Helix; all active consumers ported; no product SQLite adapter or fallback |
| 7. Verify/document | Full gates, fresh-data product flows, query plans, updated operational guidance | Acceptance below passes; limitations reported; no migration/backfill system built |

The final runtime is PostgreSQL **and Helix**, with no product SQLite dependency. Intermediate
compile paths are temporary implementation details, not a permanent dual-relational deployment.

## 9. Verification and acceptance

Convert relational test factories from in-memory SQLite to real isolated PostgreSQL test
namespaces. Assert the disposable test target before any reset, use separate connections/
namespaces for concurrent tests, and clean up only resources created by those tests. Keep
Helix unit/contract suites and run cross-store tests against an isolated Helix target.
Mocks alone do not establish PostgreSQL constraints or PostgreSQL/Helix interoperability.

Required fresh-data scenarios:

- Empty schema creation, restart/reuse, incompatible-layout rejection and rollback on
  partial schema creation; no automatic destructive reset or old-data migration.
- Register/login/session expiry/revocation with unchanged identifiers, ownership and errors.
- Create/update deals and companies, membership/ownership/team rules, access revisions,
  activity ordering and pagination. Race protected mutations against access changes.
- Assistant thread/message ordering, existing operation replay, streaming, failures and stop.
- PDF, DOCX, XLSX, PPTX and image upload/preview; byte-for-byte original retrieval; exact
  and concurrent duplicates; explicit version history; current/deleted-file constraints.
- Mid-transaction failures leave no partial relational aggregate, activity or advanced
  current-content claim. Helix failure after commit retains PostgreSQL data and exact-
  content retry repairs the graph with the same IDs and accepted image text.
- Helix partial-batch failure, terminal publication, identity/header mismatch, keyword/
  vector/hybrid retrieval and preview navigation retain current behavior. Preserve ranking,
  limits, deterministic ties, model/dimension checks and authorization before/after ranking.
- Two users with private/shared deals: inaccessible results never leak; templates and
  notebooks stay private. Verify deletion/revocation during reads and stream/worker work.
- Notebook save/retry/restart, conflicts, expired leases, stale workers, durable SSE/status,
  deletion tombstones and Helix cleanup preserve current checkpoint semantics.
- Templates import/list/assets/previews/insertion, owner/app filtering, FTS parity fixtures,
  both embedding roles, classification leases/fingerprints/retries, restart reconciliation
  and graph deletion work together.
- Permanent deal deletion resumes across restart and completes only after canonical cleanup
  and retained Helix purge/verification. Lost responses replay existing durable receipts.
- Pool exhaustion/disconnection produces bounded sanitized failures. Safe retries cannot
  duplicate activity/jobs, lose committed IDs or bypass authorization/revision guards.
- New PostgreSQL records and Helix projections remain usable after restart. Query-plan
  review confirms indexed authorization joins, content claims, job claims, pagination and
  template lexical retrieval without unbounded scans or Rust-side full-dataset loading.

Run focused converted tests, then from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Keep all test/support code under `backend/tests/` and manually wire suites into the crate's
test-only module tree (`autotests = false`). Database-backed tests must actually run against
the disposable targets; skipped integration suites are not passing evidence. Retain Helix
architecture/SDK checks and replace SQLite-specific expectations with PostgreSQL equivalents.

Verify unchanged shared-client contracts from `frontend/`:

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

Run documented Tauri Rust gates if native code changes. Inspect fresh web and desktop flows
for auth, deals, upload/preview/search, notebook saves, templates and deletion, including
loading/error/retry and partial-commit cases. Use fixed embeddings first; distinguish optional
real-provider smoke tests from deterministic storage checks. Stop only processes created by
verification, leaving shared services alone. Finish with `git diff --check` and status review.

## 10. Documentation and definition of done

When implementation lands, update `docs/ARCHITECTURE.md`, related authentication/notebook/
shared-deal/template/recovery documents, onboarding/configuration instructions, `AGENTS.md`
and affected project skills to describe **PostgreSQL canonical storage plus Helix graph/search**.
Replace obsolete SQLite setup/reset guidance while retaining and adapting Helix procedures.
State the retained commit-before-indexing failure/retry behavior; do not claim cross-store
atomicity, a general reindex command, new durable jobs or an implemented data migration.

Done means the existing product works with freshly recreated PostgreSQL tables and Helix,
all relational consumers use PostgreSQL, and the table/query/transaction/authorization and
cross-store acceptance checks pass. No Helix replacement, pgvector conversion, general
idempotency redesign, data migration or old-data cleanup is required. Record any unresolved
behavioral/query parity gap explicitly rather than weakening tests to make the port pass.

## 11. Source map and planning verification

Use these current repository sources during implementation:

- [Canonical architecture](../docs/ARCHITECTURE.md), especially backend composition, schema,
  Helix, cross-store consistency, request flows, configuration and verification.
- [Final schema/upgrades](../backend/src/app/migrations.rs),
  [bootstrap](../backend/src/app/bootstrap.rs), [configuration](../backend/src/app/config.rs),
  [backend manifest](../backend/Cargo.toml), [lockfile](../backend/Cargo.lock), [launcher](../quarry).
- [SQLite client](../backend/src/adapters/sqlite/client.rs),
  [query builder](../backend/src/adapters/sqlite/query/builder.rs),
  [document store](../backend/src/domains/documents/store/sqlite.rs),
  [identity checkpoints](../backend/src/domains/documents/store/identity.rs).
- [Document graph queries](../backend/src/domains/documents/graph/query.rs),
  [writer](../backend/src/domains/documents/graph/writer.rs),
  [repository](../backend/src/domains/documents/graph/repository.rs),
  [ingestion](../backend/src/domains/documents/ingestion/service.rs),
  [search](../backend/src/domains/documents/search/service.rs).
- [Notebook repository](../backend/src/domains/notebooks/repository.rs),
  [jobs](../backend/src/domains/notebooks/job_repository.rs),
  [Helix pipeline](../backend/src/domains/notebooks/graph.rs).
- [Template repository](../backend/src/domains/templates/slide_templates/repository.rs),
  [classification/lexical storage](../backend/src/domains/templates/slide_templates/repository/classification.rs),
  [Helix projection](../backend/src/domains/templates/slide_templates/graph.rs).
- [Deletion repository](../backend/src/domains/deals/deletion/repository.rs),
  [integration suites](../backend/tests/integration/), [frontend scripts](../frontend/package.json).

Planning verification: review local links, table coverage against the final schema, commands
against manifests, remaining Helix-replacement language, and `git diff --check`. PostgreSQL
connection details/readiness are unverified here. No database or runtime tests run for this
plan-only edit. Architecture impact: none — this revises proposed future work without
changing the current runtime, schema, contracts or configuration.
