---
name: axum-development
description: Develop, refactor, debug, review, and verify Quarry's Axum backend under backend/, including routes, handlers, services, repositories, configuration, PostgreSQL, pgvector, document jobs, parsers, and server integrations. Use for backend Rust or API behavior; do not use as the primary guide for frontend-only React changes or the Tauri desktop crate.
---

# Quarry Axum development

Make backend changes through Quarry's explicit dependency graph and verify them without mutating
valuable local data or external infrastructure.

## Read before editing

1. Read the root `AGENTS.md` and the relevant backend, data, API, configuration, and security
   sections of `ARCHITECTURE.md`.
2. Run `git status --short` and preserve unrelated work.
3. Confirm exact crate versions in `backend/Cargo.toml` and `Cargo.lock`. Quarry currently uses
   Axum 0.8, Tower 0.5, tower-http 0.7, Tokio 1, reqwest 0.13, SQLx 0.8.6 (PostgreSQL/Tokio/Rustls), and pgvector 0.4.2.
4. Inspect the route, extractor/handler, service, repository/client, DTO, and existing tests along
   the complete request path before editing.

## Preserve the dependency graph

```text
main -> AppConfig -> bootstrap -> clients/repositories/services -> feature routers with private state
request -> router -> handler -> service -> repository/client
```

- `main.rs`: tracing, config, bootstrap, bind, serve, shutdown.
- `config.rs`: all ambient environment parsing, defaults, validation, and secret wrappers.
- `app/bootstrap.rs`: bounded PostgreSQL pool and retained writer lease, schema validation, clients, repositories and services.
- `app/http/`: global HTTP middleware and API mounts.
- `domains/<feature>/route.rs`: feature route composition and private HTTP state binding before merge.
- Feature `handler.rs`/`*_handler.rs`: extract/validate transport input, invoke service, adapt response.
- Feature `service.rs`/`*_service.rs`: orchestration and preconditions derived from loaded or upstream state.
- Feature `repository.rs`/`*_repository.rs`: owned PostgreSQL persistence and search operations.
- `adapters/`, domain parsers, and query/model modules: concrete infrastructure and pure mechanisms.

Services and repositories must not depend on `AppState`, read ambient configuration, or construct
infrastructure clients. Handlers must not import repositories or construct clients. Current
architecture tests reject global `AppState`, enforce feature router state binding, and reject
lower-layer ambient configuration reads, handler repository imports and client construction.
Extend them when a new forbidden dependency pattern is introduced.

## Axum delivery rules

Before adding or moving a check, use the
[backend boundary rules](../../../AGENTS.md#backend-development-rules).
Name the inputs it needs and its earliest safe execution point. Explain a later check's concrete
state/lifecycle dependency; do not classify caller-only checks as service logic merely because
they describe a domain field. Route-level tests must demonstrate early rejection before product
work and preserve necessary guards at the actual operation.


- `/api/v1` is the intended client contract; `/api` is compatibility only.
- Compose feature routers and keep global request ID, tracing, compression, timeout, CORS, and
  state policy centralized.
- Use typed Axum extractors. Body-consuming extractors such as `Json`/`Multipart` belong last.
- Validate client-controlled path/query/header/JSON/multipart facts before product work. Reusable
  request checks belong in handler-owned `validate.rs`, normally `handler/validate.rs`. A flat
  sibling is supported when `handler.rs` includes it as a private child through `#[path = "validate.rs"]`,
  as in document viewing. Multipart streaming bounds may remain in handler-owned `upload.rs`. Keep HTTP errors and request policy out of models/services/
  adapters. Dedicated validation modules are named `validate.rs`.
- Authenticate/authorize at the HTTP boundary using shared middleware for common session/path
  policy and handlers for body-selected resources. Resolve policy through service interfaces.
  Keep transaction permission checks, persisted/upstream integrity, filesystem containment,
  worker/stream revocation, and final result authorization at their actual point of use when an
  earlier check would be stale or lacks the necessary state. Shared pure parsers stay HTTP-free.
- Keep handlers small. They may coordinate transport-specific extraction but must delegate product
  workflow and job policy.
- Return intentional statuses and response headers; retain JSON casing and SSE event contracts.
- Built-in extractor failures do not all use Quarry's `AppError` envelope. Do not claim a universal
  error schema without normalizing and testing it.

## Errors and security

- Use `RepositoryError` for storage/index failures, `ServiceError` for use-case failures, and
  `AppError` only at the HTTP boundary.
- Log internal context and send sanitized 500 messages. Never expose keys, raw provider bodies,
  database internals, or stack details.
- Use parameterized SQL through the current SQL builder/client.
- Preserve file size, name, MIME, path-containment, PDF, identifier, timeout, and URL validation.
- Treat CORS and client/Tauri validation as transport controls, not authorization.
- The API verifies bearer sessions and per-user resource ownership. Never trust caller-supplied
  email, `userId`, or `workspaceId`; use the authenticated actor. Organization tenancy remains absent.
- Never move OpenAI, WM AI, Azure, or other secrets into frontend-visible configuration.

## Async and blocking work

- Never block a Tokio worker with filesystem traversal, Office subprocess work or heavy parsing.
  PostgreSQL I/O uses the asynchronous pool; do not wrap it in blocking tasks or block on futures. Use the existing blocking-pool and concurrency-control patterns.
- Keep feature HTTP state narrow and clone service `Arc`s, not large mutable objects. Do not
  introduce a global `AppState` or repository/configuration service locator.
- Bound concurrency, response/upload sizes, pool acquisition, queries, retries and caches.
- Never hold a blocking mutex across `.await`. The PostgreSQL transaction wrapper uses a Tokio
  mutex on one owned transaction connection. Helpers must share it rather than commit independently.

## Persistence and graph rules

- PostgreSQL is canonical for users, deals, logical files, versions, and blobs.
- Document chunks, full pgvector embeddings, GIN lexical metadata and durable jobs are PostgreSQL-owned.
  Accepted input/image checkpoints precede canonical publication; one retained transaction commits
  file/version/blob, identities, every chunk/vector/lexical batch, current selection, activity and
  terminal job result. Document reads, writes, indexing and deletion all use PostgreSQL.
- Keep feature data access in `repository.rs` or specific `*_repository.rs` modules, including
  table names, joins, authorization scope and query composition. Reuse the generic typed SQL builder
  for grouped predicates, ranking and bounded multi-row INSERTs. Never add feature SQL to adapters.
- Preserve exact-content input claims, deterministic version/chunk IDs, one-current uniqueness,
  immutable accepted image text, profile compatibility and rollback. Changed bytes create a new
  logical file; admission derives file/version/document IDs from the deal ID and content hash, so exact-byte
  retries converge and identical content in different deals remains independent.
- PostgreSQL schema 7 includes resource revisions, lifecycle guards, document profiles/chunks/jobs/
  input claims, content-address constraints, document classifications, notebook search sets and
  owner-scoped template artifacts. Initialize only an empty
  selected schema; validate its source checksum/catalog fingerprint and ownership/readiness on
  restart. Unknown layouts fail without deletion. There is no import, fallback or automatic reset. The explicitly invoked guarded MVP reset requires user authorization; it is not verification.
- Bootstrap retains a dedicated-session application writer lease through all client clones; database
  writes fail closed on ownership loss. Writes additionally serialize with a schema-scoped
  transaction advisory lock at READ COMMITTED, preserving guards/activity/revisions/leases. Aggregate
  reads use REPEATABLE READ, READ ONLY and ordinary reads use MVCC. Neither read path checks
  writer ownership, session health or generation, including snapshot commit. Current access and
  final-result/session checks remain in their owning domain/HTTP layers. Typed resource locks,
  mutation RETURNING and database-clock deadlines are available; read APIs and READ ONLY
  transactions reject locking queries/mutations. Resource CAS and lifecycle/parent locks protect canonical aggregates. Schema initialization has
  its own startup-only advisory lock. Global write controls still protect worker/remote protocols
  during the concurrency transition. Do not claim distributed application support from
  independent reads or test-only transactions that bypass the global guard.
- Template lexical indexing retains exact Unicode 6.1 folding and FTS5-compatible weighted BM25,
  owner scoping and stable ties. Its captured fixtures must continue to pass. Templates use typed subject/capability
  vector pairs in PostgreSQL. Documents use exact scoped cosine and indexed English/technical lexical search.
- Document jobs retain immutable SQL input, database-clock attempts, image checkpoints and terminal replay. Preview caches remain process-local; query embeddings are not cached. Notebook atomic saves, template classification
  jobs, and deal deletion receipts persist in PostgreSQL and resume through their owned workers.
  Durable receipts do not make these workers a distributed queue.

## Configuration and optional capabilities

- Parse configuration from injected key/value sources in tests; do not mutate process-global env
  in parallel tests.
- OpenAI is optional: an API key enables it, and model names have defaults. WM AI is an all-or-none
  optional group; partial WM AI configuration must fail precisely.
- PostgreSQL and its separately provisioned pgvector extension are required at bootstrap.
- Keep secrets in `SecretString` or an equivalent redacted wrapper.
- Keep the configuration tables in `ARCHITECTURE.md` aligned with parser changes. Quarry uses
  only the ignored `backend/.env` at runtime and does not maintain a second env-shaped template.
- The Rust SharePoint client is isolated and not assembled into routes/services. A stored
  SharePoint URL is metadata, not an active import.

## Tests in this crate

`autotests = false`. Unit tests live under `backend/tests/unit/` in a tree that mirrors
`backend/src/`; they run only when the owning source module includes them with
`#[cfg(test)] #[path = "..."] mod tests;`. Cross-module suites live under
`backend/tests/integration/` and are included from the crate root. When adding a file, add and
verify the inclusion hook.

Use `cargo test <name-filter>` for focused checks. Do not use `cargo test --test <filename>` for
these manually included modules.

Cover the narrowest meaningful layer:

- handler/router: extraction, validation, status, response shape, error sanitization;
- service: use-case rules, collaborator calls, missing capability, failure propagation;
- repository: transactions, constraints, idempotency, concurrency, decode failures;
- config/bootstrap: defaults, partial groups, redaction, construction/migration failures;
- parser/query: deterministic outputs and invalid input;
- architecture: forbidden dependencies and ambient configuration reads.

## Verification ladder

From `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings -D clippy::redundant_clone
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

Run a focused `cargo test <filter>` before the full suite when practical. Add
`cargo build --locked --release` for release/build-system changes.

Do not use `cargo run` as a routine check: startup initializes/validates the selected PostgreSQL schema, validates pgvector readiness and initializes indexes. A runtime smoke test needs explicit disposable
configuration and live dependencies.

If an authorized smoke test starts Axum or another port-listening dependency, record its process
identity and port before testing. Stop that exact process gracefully before handoff and verify the
port is no longer listening, including after a failed test. Do not terminate a pre-existing or
ambiguously owned listener.

## Architecture completion gate

After implementation, re-read `ARCHITECTURE.md`. Update it in the same change if endpoints,
DTOs, layer responsibilities, dependencies, schemas, graph shape, data ownership, migrations,
jobs, configuration, integrations, security/trust boundaries, error contracts, operational
behavior, or verification commands changed. If no edit is needed, hand off with
`Architecture impact: none — <reason>`.

Report exact checks and outcomes, skipped integration tests, remaining external-dependency
uncertainty, and pre-existing failures separately.

## PostgreSQL validation environment

Server configuration requires redacted `QUARRY_DATABASE_URL`; the dedicated development database is
`Quarry`. Pool/query/acquisition bounds are parsed only in `AppConfig`. Tests require an explicit URL
ending in `_test`, use isolated random schemas and clean only namespaces they created. Keep
development and test databases separate. No remote graph service is required. Inspect
`backend/src/adapters/postgres/client.rs` and its test support for connection/recovery behavior.

## Document vector prerequisite

Provision vector 0.8.6 once per database in `quarry_extensions` using
`backend/tools/provision-pgvector.sql`. Startup validates it; tests never install/drop the shared extension.
Document repositories use the same SQLx pool, qualified official vector codec and retained
transactions. Preserve per-deal read snapshots and bounded final authorization. Numeric checks
and immutable provider/model/dimension/preparation profiles are separate requirements. Exact
cosine is the default; no ANN index or measured relevance/scale claim is implied. Keep the global
writer lease and mutation serialization. Inspect document ingestion/store repositories and
their tests for current limits and recovery behavior.
