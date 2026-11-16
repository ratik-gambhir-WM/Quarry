# PostgreSQL storage and Helix

Quarry uses the dedicated PostgreSQL database **`Quarry`** for canonical relational records and
binary assets. Helix remains the graph/search projection. This replacement starts with fresh data;
it does not import or erase existing SQLite databases or old Helix stores.

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
supervises Axum and the selected UI; PostgreSQL remains running after it exits.
The launcher defaults to the existing `helix-quarry-v3-dev` container, its
`helix-quarry-v3-dev-data` volume, and loopback port **6969**. It reuses this graph without
migrating or resetting data. For a fresh PostgreSQL deployment, explicitly select a separate graph. `QUARRY_HELIX_CONTAINER_NAME`
and `QUARRY_HELIX_PORT` select another isolated local store/port. The launcher passes its selected
`HELIX_URL` to Axum. For direct backend startup, explicitly match `HELIX_URL` to the selected graph.
Existing graph credentials, vector dimensions and template embedding settings retain their meaning.

## Vector dimensions and existing indexes

During the staged migration, `HELIX_VECTOR_DIMENSION` still supplies the common document/notebook
embedding dimension (default 1536). Document SQL uses checksummed `vector(D)` DDL and immutable
model profiles; notebooks use the same SQL dimension and their own preparation fingerprints. A changed D requires
an explicitly compatible fresh/reset SQL schema or a separately planned preserving re-embedding procedure.
The reset command reads the configured D without exposing server credentials. It never pads,
truncates or automatically rebuilds vectors. Templates retain independent dimension settings.
Do not use `clear_helix` to resolve conflicts. Old document Helix indexes are no longer used.

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
Normal startup validates this extension before initializing or validating schema 4. Document
chunks/search now use SQL; template projections still require Helix.

### Current product schema

`backend/src/app/schema.sql` owns schema 4, extending the previous aggregate/revision/lifecycle
layout with document embedding profiles, complete chunk/search rows, durable accepted jobs and
input identity claims. A deferred complete-set constraint and integrity view protect readiness.
Startup validates the rendered dimension-pinned source checksum, catalog, owned index readiness,
extension prerequisites, owners and document integrity. Unknown nonempty layouts fail intact.
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
locks, drops/recreates recognized product objects and records schema 4 in one transaction.
DDL failure rolls the entire reset back. It removes users, deals, notebooks, templates and other
product rows in that selected SQL schema; it is not a document-only deletion or an upgrade path.
The extension schema and old SQLite/Helix stores are untouched by the utility. Normal template
startup reconciliation may subsequently prune projections missing from the new SQL catalog.
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
There is no document Helix publication or partial canonical-then-graph failure. See
[document storage](document-storage.md) and [hybrid retrieval](hybrid-document-search.md).

Templates retain their separate lexical path, previously SQLite FTS5. PostgreSQL stores indexed
term/field frequencies and document lengths; retrieval uses the existing 15-field weights,
FTS5 BM25 formula, prefix AND/OR queries, transactionally maintained corpus-wide statistics and stable ID ties. It filters by
owner/app before limiting results. The retained public-domain Unicode 6.1 tokenizer tables preserve FTS5 case folding and diacritics
without a SQLite runtime dependency. Captured FTS5 ranking fixtures cover technical tokens,
diacritics, prefixes, ties and private/app scopes. Helix template metadata/dual-vector projection
and startup reconciliation remain. Canonical embedding artifacts still belong to PostgreSQL.

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
`Quarry` or attach to the development graph. Cross-store validation uses a separately selected
fresh Helix instance and fixed embeddings; optional real-provider checks remain separate.

Back up PostgreSQL with its supported backup tools and retain the matching Helix backup,
configuration and binary. Drain writers for a coordinated restore. Never delete canonical rows to
hide a projection failure, reset valuable data, or run `clear_helix` as a test. The previous SQLite
files remain historical stores for the old binary; they are not an automatic
rollback/import source for this fresh PostgreSQL deployment.

See [PostgreSQL storage verification](postgresql-storage-verification.md) for recorded branch checks
and the separately documented local launcher/index validation.

## Notebook SQL cutover (schema 4)

Notebook native content, the active pgvector/GIN artifact and terminal save-job result now commit
in one retained transaction. Accepted input remains separate; failed preparation never changes
the saved revision. Notebook Helix calls and cleanup polling are removed; templates still require
Helix. See [notebook API and recovery](notebook-api.md) for slot ordering, profile identity, attempt
fences and private content search.

Drain old workers before cutover. Use a fresh target or the explicit guarded full SQL reset above,
with backups first. No preserving backfill or re-embedding runs automatically. Reset recognizes
the old notebook cleanup ledger as a product object; discarding it does not confirm any remote
effect. Retained old Helix stores remain outside reset scope and require separately authorized
retirement. Rollback requires the old binary and matching old SQL/Helix backup; do not open schema
4 with the previous worker. Implementation verification used isolated test schemas, without a development reset or live provider calls.

On 2026-11-16, the user subsequently requested the development reset explicitly. The guarded
`reset_postgres --confirm-reset` utility successfully recreated the local `Quarry.public` product
schema as version 4. Post-reset reads confirmed empty users/deals/notebooks/templates/document
chunks, all 256 notebook slots free, no document/notebook integrity errors and no unready indexes.
No active Quarry writer was found, and no Helix operation or application startup was performed.
