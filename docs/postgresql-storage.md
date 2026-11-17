# PostgreSQL storage

Quarry uses the dedicated PostgreSQL database **`Quarry`**, with the separately provisioned pgvector
extension, as its only canonical and search store: relational records, binary assets, document
chunks, notebook search sets, template lexical postings and typed template vector pairs. Schema 5
retired the legacy graph projection; no graph service or Docker container is needed. This replacement
starts with fresh data; it does not import or erase existing SQLite databases or retired legacy stores.

## Local setup

Use the existing PostgreSQL service. PostgreSQL 18 is the validated local version. Database names
are case-sensitive when quoted: `CREATE DATABASE "Quarry";` creates the requested name.
Set the server-only connection URL before starting Quarry:

```sh
export QUARRY_DATABASE_URL=postgresql://localhost/Quarry
./quarry web
# Or: ./quarry desktop
```

The connection role must have access to `Quarry` and permission to create its initial tables and
indexes. Credentials may instead be supplied through the ignored backend configuration, never
through `VITE_*`, browser storage, Tauri arguments, screenshots, or logs. The application requires
an explicit URL; it does not use an old database path or fall back to SQLite.

On macOS, the launcher starts an existing Postgres.app 18 cluster if stopped. Axum opens
and retains its PostgreSQL pool during bootstrap, with no separate connection preflight.
A connection failure exits startup before the UI starts. `QUARRY_POSTGRES_BIN` selects the tools
(default `/Applications/Postgres.app/Contents/Versions/18/bin`) and `QUARRY_POSTGRES_DATA_DIR`
selects the existing cluster (default `$HOME/Library/Application Support/Postgres/var-18`). It
never initializes storage or creates databases. Other installations must be started separately
when these tools/storage are unavailable. Startup failures leave existing data intact; `pg_ctl`
writes startup diagnostics to `postgres-server.log` in the selected cluster. The launcher
supervises Axum and the selected UI; PostgreSQL remains running after it exits. The launcher
requires no Docker daemon and never inspects, starts or deletes containers or volumes. Retired
legacy storage containers/volumes are left exactly as they are; deleting them is a separately authorized
retirement task.

## Vector dimensions and existing indexes

`QUARRY_EMBEDDING_DIMENSION` (default 1536, 1–16000) supplies the common document/notebook
embedding dimension D. Document SQL uses checksummed `vector(D)` DDL and immutable model profiles;
notebooks use the same SQL dimension and their own preparation fingerprints. Template vectors are an
independent family: `OPENAI_SLIDE_EMBEDDING_DIMENSIONS` (default 1536, 1–16000) renders
`template_embeddings.embedding` as `vector(T)` through its own placeholder, so D never resizes
template columns and T never resizes document/notebook columns. Changing D or T selects a different
schema checksum and requires an explicitly compatible fresh/reset SQL schema or a separately planned
preserving re-embedding procedure. Only the current dimension settings are supported; obsolete
provider-specific configuration hooks have been removed. The reset command reads
both configured dimensions without exposing server credentials. It never pads, truncates or
automatically rebuilds vectors.

## Schema and adapter

### pgvector adapter prerequisite

The vector adapter pins the official `pgvector` Rust crate at 0.4.2 with
its SQLx feature, using the existing SQLx 0.8.6 pool. `CosineVector` checks dimensions
1–16,000 independently of ANN limits, finite values and finite nonzero norms before
and after provider f64-to-f32 conversion. Vector values are omitted from Debug output.
Its qualified type and operator usage does not require the extension schema in the
product search path. Domain repositories must additionally enforce model/profile identity;
matching vector dimensions alone do not prove compatibility.

Postgres.app 18 includes the server extension files. Provision the pinned extension
explicitly with a migration role, once per selected database. From the repository root,
for the disposable test database:

```sh
/Applications/Postgres.app/Contents/Versions/18/bin/psql -X -d Quarry_test \
  -f backend/tools/provision-pgvector.sql
```

The script creates `quarry_extensions` and extension `vector` version 0.8.6, revokes
public CREATE on that schema and refuses an existing mismatched version/location.
It never moves, upgrades or drops an existing extension. With a separate application
role, grant that role USAGE on the extension schema through deployment role management.
Do not grant application DDL privileges. Other server installations need the approved
server extension package first; a Cargo dependency does not install server binaries.
Tests create/drop only their own isolated product schemas, retaining the shared extension.

This prerequisite is exercised by the codec tests, including 1,536/3,072-dimensional
round trips, schema shadowing, cosine normalization, rollback and read-only enforcement.
Normal startup validates this extension before initializing or validating schema 5. Documents,
notebooks and templates all search in SQL.

### Current product schema

`backend/src/app/schema.sql` owns schema 5, extending the previous aggregate/revision/lifecycle
layout with document embedding profiles, complete chunk/search rows, durable accepted jobs and
input identity claims, notebook search sets, and template embedding profiles plus typed
subject/capability vector pairs. Template stage rows carry per-stage opaque claim tokens and
database-clock epoch-second leases/retry times instead of caller timestamp text; schema 4's template
BYTEA vectors were removed without a decoder (MVP recreation, no preservation). Deferred
complete-set constraints and integrity views protect document, notebook and template readiness.
Startup validates the rendered dimension-pinned source checksum, catalog, owned index readiness,
extension prerequisites, owners and document/notebook/template integrity. Unknown nonempty layouts fail intact.
There is no populated-data migration or automatic reset. See [document storage](document-storage.md).

### Explicit MVP reset

The user-selected rollout recreates existing product tables rather than importing schema-2 data.
After stopping the API writer and provisioning pgvector in the selected database, from `backend/`:

```sh
cargo run --locked --bin reset_postgres -- --confirm-reset
```

This explicit destructive operation loads the normal server configuration (without printing it),
requires the dedicated `Quarry` or an isolated `_test` database, and refuses unrelated relations/
functions or an active writer. It retains the schema initialization, writer-ownership and mutation
locks, drops/recreates recognized product objects and records schema 5 in one transaction.
DDL failure rolls the entire reset back. It removes users, deals, notebooks, templates and other
product rows in that selected SQL schema; it is not a document-only deletion or an upgrade path.
The extension schema, old SQLite files and retired legacy stores are untouched by the utility.
The application never invokes this reset automatically. Under the temporary MVP policy effective
2026-11-15 in [the agent guide](../AGENTS.md#persistence-and-indexing), a requested PostgreSQL schema
change authorizes table recreation unless data preservation is explicitly requested. Revisit this
policy when the product matures; resets outside schema-change work still need explicit authorization.

`adapters/postgres` owns SQLx 0.8.6 pooling, typed values/rows, parameterized queries and transactions.
Bootstrap injects one pool into all repositories. Generated numeric identities use `RETURNING` and
JavaScript-safe bounds. Binary records use `bytea`; serialized JSON and timestamps remain text so
exact fingerprints, formatting and existing API comparisons remain stable. Text uses C collation;
query builders retain ASCII-only LIKE folding and explicit SQLite-compatible NULL ordering.

| Setting | Default and bounds |
| --- | --- |
| `QUARRY_DATABASE_URL` | Required PostgreSQL URL selecting a database; redacted |
| `QUARRY_DATABASE_MAX_CONNECTIONS` | 16; 2–128; one connection retains writer ownership |
| `QUARRY_DATABASE_ACQUIRE_TIMEOUT_SECONDS` | 5; 1–60 |
| `QUARRY_DATABASE_STATEMENT_TIMEOUT_SECONDS` | 30; 1–300; also bounds lock waits and idle transactions |

Database I/O is asynchronous. After transactional schema initialization/validation, bootstrap reserves
a pooled connection for a session advisory writer lease on the selected schema. All router and
detached-worker clones retain it; another application writer is rejected. The connection closes
on drop rather than returning a session
lock to the pool. Writes check the dedicated session and fail closed on ownership loss until
restart. This retains the single active Axum writer boundary, rather than enabling a distributed
application deployment. A write transaction takes a PostgreSQL advisory lock scoped to the
selected schema, then runs at READ COMMITTED on one connection. All application mutations share
this ordering, including authorization/revisions, activity, file claims, leases and deletion
checkpoints. Constraints and conditional revision/lease guards remain in place. This deliberately
preserves the previous aggregate write serialization across processes; it does not yet provide
parallel writers for independent deals. Read aggregates use REPEATABLE READ, READ ONLY on one
connection. No database transaction wraps a provider call, graph write or native conversion.
Pool wait/query/lock failures are bounded; uncertain commits are not automatically replayed.

Each writer lease registers a fresh generation under `postgres_writer_generation` in the existing
`app_metadata` table. Before committing, writes recheck the lease session and take a shared row
lock on that generation, rejecting a missing or replaced value. Takeover updates the generation
without waiting for the aggregate advisory lock, so an older in-flight transaction cannot publish
after replacement ownership is established. If a transaction has already locked its matching
generation for commit, takeover waits for that transaction to finish before returning ownership.
This row lock closes the check-to-commit race; a session ping alone cannot do so. Single-query and
aggregate reads no longer check the writer session, its health flag or generation. Snapshot commit
also has no writer dependency. Reads retain MVCC/REPEATABLE READ, READ ONLY, pool/statement bounds
and current authorization checks; losing writer ownership does not itself make reads unavailable.
Closing that reader's pool still returns the normal pool-closed error.

The query layer now has typed resource locks (FOR SHARE/FOR UPDATE), SKIP LOCKED selection,
UPDATE/DELETE RETURNING and database-clock integer deadlines. Locking SELECT is distinct from an
ordinary read: read runners reject it, and explicit transaction locking/returning APIs reject
READ ONLY transactions. Independent-pool tests exercise those primitives without the global write
guard in a test-only harness. Production still retains the writer lease/generation and global
mutation advisory lock. Public revision APIs and resource lifecycle/parent coordination now use
these primitives. Deal guards use shared mutation locks and exclusive authority locks; notebooks,
threads and templates lock their specific parents where multi-row invariants require it. Revisions
are conditional and bounded; client drafts retain their submitted revision after conflict. Lexical
corpus statistics are exact snapshot aggregates, eliminating the singleton mutation hotspot. Durable
distributed work and remote publication fencing remain pending. This is an intermediate
concurrency rollout, not authorization to run multiple application instances.

## Search and partial failure

Documents commit canonical bytes, identities, chunks, pgvector embeddings, lexical data, activity
and terminal job result together. Preparation/provider work precedes SQL locks; accepted inputs and
image text are separate durable checkpoints. Both search branches use one read-only snapshot per
deal with authorization/profile/readiness before limits. Final bounded authorization remains.
There is no document legacy storage publication or partial canonical-then-graph failure. See
[document storage](document-storage.md) and [hybrid retrieval](hybrid-document-search.md).

Templates retain their separate lexical path, previously SQLite FTS5. PostgreSQL stores indexed
term/field frequencies and document lengths; retrieval uses the existing 15-field weights,
FTS5 BM25 formula, prefix AND/OR queries, transactionally maintained corpus-wide statistics and stable ID ties. It filters by
owner/app before limiting results. The retained public-domain Unicode 6.1 tokenizer tables preserve FTS5 case folding and diacritics
without a SQLite runtime dependency. Captured FTS5 ranking fixtures cover technical tokens,
diacritics, prefixes, ties and private/app scopes. Template semantic ranking uses exact pgvector
cosine per role over the authorized, profile-matching ready catalog; only IDs and similarities leave
SQL, and the existing fusion/tie policy runs over the complete ranking. There is no template graph
projection, publication step or startup reconciliation; SQL commit is semantic availability.

Notebook/document jobs and deletion receipts are durable; preview caches remain in memory.
Query embeddings are generated per request without caching.
The historical wire field `sqliteCommitted` and durable deletion phase `sqlite` now mean canonical
PostgreSQL commit/cleanup. Their names remain unchanged for client compatibility.

## Tests and recovery

Create a separate disposable database, then run the documented backend gates:

```sh
# Once, on the existing server: CREATE DATABASE "Quarry_test";
export QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test
cd backend
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Database-backed tests require a URL whose database name ends in `_test`. Each fixture uses a new
random schema, and restart fixtures reuse only their own schema. Normal test-process exit drops
only registered test schemas. An interrupted/killed test process may leave its schemas behind;
review those resources in the disposable test database before removing them. Tests never reset
`Quarry`. Template suites run the SQL path with no graph store, using fake providers and real
pgvector; optional real-provider checks remain separate.

Back up PostgreSQL with its supported backup tools and retain the matching configuration and
binary. Drain writers for a coordinated restore. Never delete canonical rows to hide a failure or
reset valuable data as a test. The retired graph maintenance binaries were removed without being run. The previous SQLite
files remain historical stores for the old binary; they are not an automatic
rollback/import source for this fresh PostgreSQL deployment.

See [PostgreSQL storage verification](postgresql-storage-verification.md) for recorded branch checks
and the separately documented local launcher/index validation.

## Notebook SQL cutover (schema 4)

Notebook native content, the active pgvector/GIN artifact and terminal save-job result now commit
in one retained transaction. Accepted input remains separate; failed preparation never changes
the saved revision. Notebook legacy storage calls and cleanup polling are removed; templates then still required
legacy storage (retired in schema 5, below). See [notebook API and recovery](notebook-api.md) for slot ordering, profile identity, attempt
fences and private content search.

The phrase-search correction adds bounded positional title/content projections and a GIN phrase
index to schema 4, with a `phrases-v1` notebook pipeline fingerprint. Its schema checksum changes;
an installation initialized from the earlier schema-4 definition must use a fresh selected schema
or the explicit guarded reset above before running this binary. Startup refuses the earlier
layout intact. The full SQL reset discards recognized product data, including users, deals, notebooks
and templates; it leaves unrelated databases, SQLite files and legacy stores alone. No automatic
backfill, reset or re-embedding is performed.

Drain old workers before cutover. Use a fresh target or the explicit guarded full SQL reset above,
with backups first. No preserving backfill or re-embedding runs automatically. Reset recognizes
the old notebook cleanup ledger as a product object; discarding it does not confirm any remote
effect. Retained old legacy stores remain outside reset scope and require separately authorized
retirement. Rollback requires the old binary and matching old SQL/legacy storage backup; do not open schema
4 with the previous worker. Implementation verification used isolated test schemas, without a development reset or live provider calls.

On 2026-11-16, the user subsequently requested the development reset explicitly. The guarded
`reset_postgres --confirm-reset` utility successfully recreated the local `Quarry.public` product
schema as version 4. Post-reset reads confirmed empty users/deals/notebooks/templates/document
chunks, all 256 notebook slots free, no document/notebook integrity errors and no unready indexes.
No active Quarry writer was found, and no legacy storage operation or application startup was performed.

## Template SQL cutover and legacy storage retirement (schema 5)

Templates were the last legacy storage consumer. Schema 5 replaces the template graph projection, its
startup reconciliation and the facade's process-local mutation mutex with PostgreSQL-only
protocols: stage claims with opaque tokens and database-clock leases, claim-fenced classification
publication (metadata, lexical postings and embedding input together) and one-transaction typed
vector pair publication validated by a deferred integrity trigger. The legacy storage SDK dependency, client,
configuration, binaries and launcher Docker setup were removed.

Rollout follows the 2026-11-15 MVP policy: no data is preserved. Drain the old API and workers,
take matching backups, then use a fresh selected database or run the guarded
`reset_postgres --confirm-reset` (a full product-schema recreation: users, deals, documents,
notebooks and templates). Startup refuses an existing schema-4 layout intact; it never drops,
backfills, re-embeds or retries with a reset. After recreation, missing pairs remain not ready until
templates are re-imported and classified. Rollback requires the old binary plus matching SQL and
legacy storage backups; otherwise forward-fix after drain. Retired legacy storage containers, volumes and indexes are
not deleted by this change. Implementation verification used isolated `_test` schemas only; no
development reset was run.
