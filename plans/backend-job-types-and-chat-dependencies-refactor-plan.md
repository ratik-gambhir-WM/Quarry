# Backend job types and chat dependencies refactor plan

Date: 2026-11-16. Status: proposed; implementation has not started.

## Outcome and scope

Complete the two remaining recommendations from the backend refactor review:

1. Replace notebook save-job status and stage strings with domain enums.
2. Make assistant chat service dependency requirements explicit at construction.

Implement these as separate, behavior-preserving changes in the Axum build root, `backend/`.
Preserve the earlier template import, chat execution, preview, PowerPoint reader, and repository
capability refactors already in the working tree. This plan is a design input; current code,
tests, root instructions, and the maintained architecture remain authoritative.

## 1. Typed notebook save-job status and stage

### Current state

`backend/src/domains/notebooks/model.rs` represents `SaveJob.status` and `SaveJob.stage` as
`String`; terminal detection compares string literals. `job_repository.rs` decodes these values,
composes SQL predicates and updates, and accepts a string stage in `checkpoint`. Save workflow,
retry, and event code also interpret these values.

### Implementation

- Inventory all status/stage values and their consumers under `backend/src/domains/notebooks/`,
  the schema constraints, and notebook tests before defining the variants.
- Introduce notebook-owned `SaveStatus` and `SaveStage` enums with explicit persisted-string
  mappings and serialization matching the current JSON values. Retain text columns; no PostgreSQL
  enum type, schema recreation, migration, or data reset is needed.
- Decode database values at the repository boundary and use typed values in `SaveJob`, terminal
  detection, retry decisions, worker branches, and checkpoint calls. Use exhaustive matches where
  behavior depends on the variant. Encode values explicitly when building SQL.
- Check existing handling of malformed persisted values before choosing decode errors. Preserve
  existing integrity guarantees and sanitized errors; do not silently map unknown values to a
  valid status or stage.
- Keep error codes as they are. Do not introduce a shared job framework or combine notebook,
  document, and template job state machines.

### Contracts and acceptance criteria

- Preserve exact JSON and database strings, including the stage retained on terminal jobs.
- Preserve retry eligibility, attempt bounds, sequence increments, operation IDs, accepted
  snapshots, committed revisions, event payloads, and the historical `sqliteCommitted` field.
- Keep lease tokens, database-clock deadlines, ownership/revision checks, slot release, and
  canonical notebook/index/terminal-job publication on their existing transactions.
- Cover serialization and database decoding, terminal detection, retry decisions, stale attempts,
  checkpoints, rollback, and recovery. Retain the notebook HTTP and save-job lifecycle coverage.
- Completion means status/stage decisions and checkpoint inputs use the enums; remaining string
  literals belong only to explicit mappings, SQL representation, or contract fixtures.

## 2. Explicit assistant chat dependencies

### Current state

`backend/src/domains/assistant/chat/service.rs` independently makes persistence, user lookup,
documents, deal context, and deletion lifecycle optional. Constructors and chained setters can
create partially assembled services. Production bootstrap supplies these dependencies together,
while methods repeatedly check their availability at runtime.

### Implementation

- Inventory every constructor and setter call in production and tests. Distinguish supported
  optional product capabilities from incomplete test setup before changing signatures.
- Make production construction require the repository, user directory, document reader, deal
  context reader, and deletion lifecycle. Use a small named dependency struct if it makes the
  constructor clearer; keep configuration and infrastructure assembly in `app/bootstrap.rs`.
- Keep OpenAI optional and preserve explicit unavailable responses when generation is invoked
  without it. Do not require a provider for thread operations or completed-run replay.
- Remove runtime missing-dependency branches made unreachable by required construction. Replace
  production constructor/setter usages together. Keep any intentionally supported reduced mode
  explicit only if the caller inventory demonstrates a real requirement.
- Put test builders and fixtures under `backend/tests/`. Prefer the same required constructor
  with existing test collaborators over production options or visibility added just for tests.
- Preserve the extracted preparation/replay helpers and owned `ChatRun` execution context.

### Contracts and acceptance criteria

- Preserve routes, DTOs, statuses, SSE event names/order, and client-visible optional-provider
  behavior. Production bootstrap must no longer construct chat in partially configured steps.
- Replay completed runs before provider and current deal-context resolution. Preserve request-ID
  idempotency, branch history, context limits, and failure persistence.
- Preserve authorization before canonical file loading, loaded-thread scope checks, bounded file
  resolution, upload limits, and final transaction guards. Apply the boundary-placement decision
  in `docs/backend-boundary-checks.md` to any check whose location changes.
- Retain admission ownership across the detached run and blocking file work as currently
  implemented. Preserve revocation independent of provider/SSE backpressure, disconnect handling,
  partial-text persistence, and deletion barriers.
- Verify optional-provider behavior, thread CRUD, replay, context/file failures, cancellation,
  revocation, and deletion. Update affected HTTP/bootstrap fixtures as well as service tests.

## Delivery and verification

Before each implementation, run `git status --short`, read the applicable backend skills, and
reconfirm the live call paths and tests. Start with focused notebook or chat tests. Keep test
bodies and support in the manually included `backend/tests/` tree; verify new modules are wired.

From `backend/`, run the normal completion gates for each coherent change:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings -D clippy::redundant_clone
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

Tests use isolated schemas in the separate `_test` database and local mock servers. Preserve
the existing pgvector prerequisite and opt-in integration boundaries. Do not use application
startup, development data resets, or legacy storage clearing as verification.

Review the final diff for behavior, dependency direction, error handling, ownership/copies, and
unrelated changes. Run `git diff --check` and recheck status. Update `docs/ARCHITECTURE.md` for the
implemented type/construction responsibilities, and any operational or contract change actually
required by the final design. Report checks, ignored tests, and remaining uncertainty separately.
