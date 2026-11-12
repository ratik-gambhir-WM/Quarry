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

On macOS, the launcher starts an existing Postgres.app 18 cluster if stopped, then checks
PostgreSQL connectivity without changing its schema. `QUARRY_POSTGRES_BIN` selects the tools
(default `/Applications/Postgres.app/Contents/Versions/18/bin`) and `QUARRY_POSTGRES_DATA_DIR`
selects the existing cluster (default `$HOME/Library/Application Support/Postgres/var-18`). It
never initializes storage or creates databases. Other installations must be started separately
when these tools/storage are unavailable. Startup failures leave existing data intact; `pg_ctl`
writes startup diagnostics to `postgres-server.log` in the selected cluster. The launcher
supervises Axum and the selected UI; PostgreSQL remains running after it exits.
The new local graph uses `helix-quarry-postgres-dev`, a separate disk-backed Docker volume, and
loopback port **6970**, leaving the previous port-6969 graph untouched. `QUARRY_HELIX_CONTAINER_NAME`
and `QUARRY_HELIX_PORT` select another isolated local store/port. The launcher passes its selected
`HELIX_URL` to Axum. For direct backend startup, explicitly match `HELIX_URL` to the selected graph.
Existing graph credentials, vector dimensions and template embedding settings retain their meaning.

## Vector dimensions and existing indexes

`HELIX_VECTOR_DIMENSION` controls document and notebook embeddings and their Helix vector indexes;
the code default is 1536. Existing indexes retain their created dimension across restarts. Changing
configuration does not resize them: a conflicting definition stops bootstrap before the API binds.
`text-embedding-3-large` supports the selected local 3072-dimensional configuration. Template
subject/capability vectors have their own `OPENAI_SLIDE_EMBEDDING_DIMENSIONS` setting and are not
changed by the document setting.

Keep application writers stopped during an explicitly authorized dimension change. Verify canonical
document versions, notes and save jobs as well as Helix document/notebook projection counts. When
all are empty, replace only the document `FileChunk.embedding` (partition `deal_id`) and notebook
`QuarryNotebookChunk.embedding` (partition `owner_id`) vector definitions, retaining cosine distance,
and wait for each drop/build operation to succeed. Preserve unrelated nodes, indexes and volumes.
For populated projections, retain matching backups and plan re-embedding/publication with the target
model; vectors cannot be resized by padding or truncation. No automatic migration or general
reindex command is provided. Never use `clear_helix` to resolve a dimension conflict.

## Schema and adapter

`backend/src/app/schema.sql` creates the final relational equivalent of SQLite marker 18 directly.
PostgreSQL schema version **1**, its source checksum and a live catalog fingerprint are recorded
in `quarry_schema_version`. Startup serializes schema inspection/creation, initializes an empty
selected schema transactionally, or validates and reuses version 1. Unknown nonempty layouts,
changed DDL/checks/indexes and unsupported versions fail without deleting records. There is no
SQLite upgrade chain, reset binary, data backfill, standalone template database or dual write.
Exactly-one-owner and document identity/graph readiness checks still run before serving traffic.

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
lock to the pool. Operations check the dedicated session and fail closed on ownership loss until
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
This row lock closes the check-to-commit race; a session ping alone cannot do so. Aggregate reads
recheck the live lease before returning their snapshot, without taking write locks. No schema DDL,
stored document format, API contract or PostgreSQL schema-version change is required.

## Search and partial failure

Document search, embeddings, identities, graph queries and publication remain in Helix. Canonical
PostgreSQL ownership/current-version/deletion checks still filter and reauthorize results.
Files, original bytes, manifests and accepted image text commit before graph publication. A graph
failure preserves those records and returns the existing committed identity. Exact-content retry
uses the same identity and accepted image text. There is no distributed transaction or general
bulk reindex command.

Templates retain their separate lexical path, previously SQLite FTS5. PostgreSQL stores indexed
term/field frequencies and document lengths; retrieval uses the existing 15-field weights,
FTS5 BM25 formula, prefix AND/OR queries, transactionally maintained corpus-wide statistics and stable ID ties. It filters by
owner/app before limiting results. The retained public-domain Unicode 6.1 tokenizer tables preserve FTS5 case folding and diacritics
without a SQLite runtime dependency. Captured FTS5 ranking fixtures cover technical tokens,
diacritics, prefixes, ties and private/app scopes. Helix template metadata/dual-vector projection
and startup reconciliation remain. Canonical embedding artifacts still belong to PostgreSQL.

Notebook jobs and deletion receipts remain durable; document jobs and caches remain in memory.
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
files and port-6969 graph remain historical stores for the old binary; they are not an automatic
rollback/import source for this fresh PostgreSQL deployment.

See [PostgreSQL storage verification](postgresql-storage-verification.md) for recorded branch checks
and the separately documented local launcher/index validation.
