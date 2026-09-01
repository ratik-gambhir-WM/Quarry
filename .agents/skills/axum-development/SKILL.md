---
name: axum-development
description: Develop, refactor, debug, review, and verify Quarry's Axum backend under backend/, including routes, handlers, services, repositories, configuration, SQLite, Helix, document jobs, parsers, and server integrations. Use for backend Rust or API behavior; do not use as the primary guide for frontend-only React changes or the Tauri desktop crate.
---

# Quarry Axum development

Make backend changes through Quarry's explicit dependency graph and verify them without mutating
valuable local data or external infrastructure.

## Read before editing

1. Read the root `AGENTS.md` and the relevant backend, data, API, configuration, and security
   sections of `docs/ARCHITECTURE.md`.
2. Run `git status --short` and preserve unrelated work.
3. Confirm exact crate versions in `backend/Cargo.toml` and `Cargo.lock`. Quarry currently uses
   Axum 0.8, Tower 0.5, tower-http 0.7, Tokio 1, reqwest 0.13, rusqlite 0.39, and Helix 2.
4. Inspect the route, extractor/handler, service, repository/client, DTO, and existing tests along
   the complete request path before editing.

## Preserve the dependency graph

```text
main -> AppConfig -> bootstrap -> clients/repositories/services -> AppState
request -> router -> handler -> service -> repository/client
```

- `main.rs`: tracing, config, bootstrap, bind, serve, shutdown.
- `config.rs`: all ambient environment parsing, defaults, validation, and secret wrappers.
- `bootstrap.rs`: SQLite open/migration, shared client construction, repositories, services, state.
- `state.rs`: cheap-to-clone `Arc<Service>` handles for Axum handlers.
- `routes/`: route/middleware composition.
- `handlers/`: extract/validate transport input, invoke service, adapt response.
- `services/`: use-case validation and orchestration.
- `repository/`: SQLite/Helix persistence and projection operations.
- `core/clients`, parsers, SQL/query/node modules: concrete infrastructure and pure mechanisms.

Services and repositories must not depend on `AppState`, read ambient configuration, or construct
infrastructure clients. Handlers must not import repositories or construct clients. Current
architecture tests enforce the `AppState` and ambient-configuration rules and check handler
repository imports/client construction; extend them when a new forbidden dependency pattern is
introduced.

## Axum delivery rules

- `/api/v1` is the intended client contract; `/api` is compatibility only.
- Compose feature routers and keep global request ID, tracing, compression, timeout, CORS, and
  state policy centralized.
- Use typed Axum extractors. Body-consuming extractors such as `Json`/`Multipart` belong last.
- Validate path/query/multipart transport facts at the handler boundary; put use-case invariants in
  services and persistence invariants in repositories/model helpers.
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
- The current API has no authentication or tenancy. Do not add routes that assume caller-supplied
  email, `userId`, or `workspaceId` is trustworthy; flag the missing authorization boundary.
- Never move OpenAI, Helix, WM AI, Azure, or other secrets into frontend-visible configuration.

## Async and blocking work

- Never block a Tokio worker with SQLite, filesystem traversal, Office subprocess work, or heavy
  parsing. Use the existing blocking-pool and concurrency-control patterns.
- Keep `AppState` lean and clone service `Arc`s, not large mutable objects.
- Bound concurrency, response/upload sizes, retries, caches, and task retention deliberately.
- Do not use `std::sync::Mutex` across an `.await`. The SQLite client intentionally contains its
  synchronous connection and exposes async offloading methods; do not leak the connection outward.

## Persistence and graph rules

- SQLite is canonical for users, deals, logical files, versions, and blobs.
- Helix is the versioned search projection. SQLite commits before Helix indexing; re-uploading the
  same bytes can find the committed version by hash and retry indexing. There is no general
  SQLite-to-Helix reindex command today.
- Preserve content hashes, exact-content idempotency, foreign keys, one-current-version uniqueness,
  and transaction rollback behavior. Document IDs are content-derived; version and final chunk IDs
  are deterministic after a logical `file_id` is selected.
- Parsers create a new random `file_id`, and the normal ingestion path reuses one only for an exact
  content-hash match. Changed content currently becomes a new logical file even though repository
  tests can create later versions by supplying the same `file_id` explicitly.
- Schema version 6 migration drops and recreates older application tables. Use only disposable
  databases in tests and make any new migration/recovery policy explicit.
- `cargo run --bin clear_helix` is destructive. It is never a development or verification command;
  use it only with explicit authorization and ADR 0002's backup/drain/reindex procedure.
- Jobs and caches are currently process-local. Do not describe them as durable or distributed.

## Configuration and optional capabilities

- Parse configuration from injected key/value sources in tests; do not mutate process-global env
  in parallel tests.
- OpenAI is optional: an API key enables it, and model names have defaults. WM AI is an all-or-none
  optional group; partial WM AI configuration must fail precisely.
- Helix is mandatory during normal bootstrap and index initialization.
- Keep secrets in `SecretString` or an equivalent redacted wrapper.
- Update `.env.example` when the schema changes, but remember it currently has known partial-OpenAI
  drift; test the parser rather than assuming the example starts the app.
- The Rust SharePoint client is isolated and not assembled into routes/services. A stored
  SharePoint URL is metadata, not an active import.

## Tests in this crate

`autotests = false`. Tests under `backend/tests/` run only when a source module includes them with
`#[cfg(test)] #[path = "..."] mod tests;`. When adding a file, add and verify the inclusion hook.

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
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Run a focused `cargo test <filter>` before the full suite when practical. Add
`cargo build --locked --release` for release/build-system changes.

Do not use `cargo run` as a routine check: startup opens and may destructively migrate SQLite,
requires Helix, and initializes indexes. A runtime smoke test needs explicit disposable
configuration and live dependencies.

## Architecture completion gate

After implementation, re-read `docs/ARCHITECTURE.md`. Update it in the same change if endpoints,
DTOs, layer responsibilities, dependencies, schemas, graph shape, data ownership, migrations,
jobs, configuration, integrations, security/trust boundaries, error contracts, operational
behavior, or verification commands changed. If no edit is needed, hand off with
`Architecture impact: none — <reason>`.

Report exact checks and outcomes, skipped integration tests, remaining external-dependency
uncertainty, and pre-existing failures separately.
