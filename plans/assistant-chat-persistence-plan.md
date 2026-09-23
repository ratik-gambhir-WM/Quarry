# Assistant chat persistence plan

Status: implemented

## Implementation outcome

The reviewed schema-v8 slice is now implemented across the Axum API, shared React application,
and Tauri relay. SQLite owns assistant threads and normalized messages; the run endpoint owns the
atomic prompt/pending-response insert, canonical context reconstruction, idempotent retry, and
terminal completion/failure/cancellation update. The Assistant page uses assistant-ui's remote
thread-list and history adapters, while `/api/v1/query_model` remains available for legacy callers.

All assistant repository operations use Quarry's existing `SqlBuilder`/`SqlQuery` abstraction,
including the reads and writes performed inside the immediate run transaction. The implementation
also updates the schema migration tests, backend route/repository coverage, web and desktop API
contracts, sidebar thread rendering, and the canonical architecture documentation.

The implementation intentionally retains the review decisions below: workspace email is still a
temporary development identity, process termination can leave a `streaming` row, thread URL
restoration and attachment reuse are follow-ups, and the history adapter remains load-only because
the run endpoint is the sole message writer.

## Review decisions

The implementation should ship as one coherent schema-v8 vertical slice, with the following
adjustments to the original proposal:

- Use the workspace email as an explicitly temporary development identity at the HTTP boundary,
  resolve it to the existing `users.id` in the service, and enforce that resolved owner on every
  thread operation. This preserves the current product flow without pretending the email is
  authenticated.
- Keep `/api/v1/query_model` as a compatibility endpoint. New Assistant UI traffic uses
  `/api/v1/assistant/threads/{threadId}/runs`; the legacy endpoint retains client-supplied context.
- Use the assistant message ID as the run/idempotency key. A completed duplicate is replayed and
  an in-progress duplicate is rejected, so retries cannot insert another prompt/response pair.
- Persist the prompt and pending assistant row atomically, then persist the terminal assistant
  state. Periodic text checkpoints are deferred until there is a measured durability need; an
  observed provider failure or client cancellation still stores buffered partial text. A process
  crash can therefore leave a `streaming` row and is documented as a known limitation.
- Implement the assistant-ui history adapter as load-only for run-owned messages. Its
  `append`/`update` hooks are intentional no-ops because the run endpoint is the single writer;
  this avoids duplicate writes and keeps SQLite independent of assistant-ui's vendor format.
- Derive the initial title from the first prompt and support the remote adapter's rename/archive/
  unarchive/delete contract. URL query-parameter restoration, branching, attachment reuse, and
  resumable streams remain out of scope for schema v8.

## Recommendation

Persist assistant conversations in the Axum service’s canonical SQLite database. Keep SSE for
low-latency delivery, but make the server own the conversation and message lifecycle:

1. Create or resolve a conversation before starting generation.
2. Persist the user prompt in a transaction before calling OpenAI.
3. Create an assistant message with a `streaming` status.
4. Accumulate the response on the server while forwarding deltas to the client.
5. Persist a final `completed`, `failed`, or `cancelled` assistant message, including any
   partial text that was generated.

Do not use browser `localStorage`, IndexedDB, or the assistant-ui in-memory runtime as the
authoritative store. Those can be useful as a short-lived UI cache, but they cannot support
cross-device history, desktop/web consistency, authorization, or recovery after a process
restart.

Use assistant-ui’s existing custom persistence extension as the frontend integration boundary:
`useRemoteThreadListRuntime` with a `RemoteThreadListAdapter` for the conversation list and a
`ThreadHistoryAdapter` for message hydration. Store Quarry’s normalized message model in SQLite
and translate to/from assistant-ui’s runtime format in the adapter; do not make the vendor format
the database schema.

## Findings from the current implementation

- [`Assistant.tsx`](../frontend/src/pages/Assistant.tsx) mounts a fresh provider and owns only the
  context-truncation flag. There is no conversation identity or load operation.
- [`QueryChatRuntimeProvider.tsx`](../frontend/src/components/chat/QueryChatRuntimeProvider.tsx)
  calls `useLocalRuntime`, which starts with one in-memory thread.
- [`queryModelAdapter.ts`](../frontend/src/components/chat/queryModelAdapter.ts) reconstructs
  bounded completed user/assistant pairs from runtime memory and sends them with every request.
  It yields cumulative snapshots but has no persistence callback.
- The web and desktop adapters stream the existing `/api/v1/query_model` SSE contract. The Tauri
  path is a transport relay, not a second product backend.
- [`AssistantChatService`](../backend/src/domains/assistant/chat/service.rs) forwards provider
  deltas through a bounded channel and emits a terminal event, but does not retain or write the
  prompt/response.
- [`AssistantWorkspaceSidebar.tsx`](../frontend/src/components/hub/sidebar/AssistantWorkspaceSidebar.tsx)
  renders an empty “Previous chats” list; `ThreadListPrimitive.New` is already present.
- SQLite schema version 7 is the current schema. Assistant chat is explicitly documented as
  ephemeral and the application has no inbound authentication or tenant authorization today.

## Proposed data model

Add assistant-owned tables in a new incremental schema migration (version 8):

### `assistant_threads`

- `thread_id TEXT PRIMARY KEY`
- `user_id INTEGER NOT NULL REFERENCES users(id)`
- `title TEXT NOT NULL` (initially derived from the first prompt, with explicit rename support)
- `status TEXT NOT NULL CHECK (status IN ('regular', 'archived'))`
- `created_at`, `updated_at`, and `last_message_at TEXT NOT NULL`
- optional `metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json))`

Index by `(user_id, status, last_message_at DESC, thread_id)`.

### `assistant_messages`

- `message_id TEXT PRIMARY KEY` (accept the stable assistant-ui message ID when supplied)
- `thread_id TEXT NOT NULL REFERENCES assistant_threads(thread_id) ON DELETE CASCADE`
- `parent_message_id TEXT REFERENCES assistant_messages(message_id)` for future branching
- `sequence INTEGER NOT NULL` for deterministic linear ordering
- `role TEXT NOT NULL CHECK (role IN ('user', 'assistant'))`
- `content TEXT NOT NULL`
- `status TEXT NOT NULL CHECK (status IN ('streaming', 'completed', 'failed', 'cancelled'))`
- `model TEXT`, `error_code TEXT`, `created_at TEXT NOT NULL`, `completed_at TEXT`
- optional `metadata_json` for bounded, non-secret request/response metadata

Add uniqueness for `(thread_id, sequence)` and indexes for `(thread_id, sequence)` and
`(thread_id, parent_message_id)`. Store prompt and response as separate message rows rather than
one request/response blob so failed runs, partial responses, retries, ordering, and future edits
remain representable. Do not store every token delta as an event row. Schema v8 writes the
accumulated assistant text at the terminal transition; bounded periodic checkpoints are a
follow-up once their extra write load and recovery semantics are justified.

The `user_id` foreign key is intentional, but it is not sufficient by itself today: current
routes trust caller-supplied identity. Persistent assistant endpoints must still be treated as
development-only until Axum derives the user from real authentication and enforces ownership.

## Backend implementation sequence

1. Add assistant chat repository/storage types under `backend/src/domains/assistant/chat/` for
   thread list/create/archive/rename, message load, prompt insert, assistant-message insert, and
   assistant-message checkpoint/finalization. Use the existing `SqliteClient` and
   `spawn_blocking` pattern; keep SQL and transactions inside the repository.
2. Extend `AssistantChatService` to accept the SQLite-backed repository. Resolve the authenticated
   owner (or the explicitly temporary development identity) before every read/write. Load prior
   completed messages server-side, apply the existing 64-message/400,000-character limits, then
   persist the new prompt and pending assistant row in one transaction.
3. Add a request/run identifier for idempotency. A retried request must not create duplicate user
   messages or duplicate assistant responses. Carry `threadId`, `userMessageId`,
   `assistantMessageId`, and `requestId` through the request where available; use server-generated
   IDs when absent.
4. Update the generation loop to maintain a response buffer and finalize it on provider
   completion. On provider failure, client disconnect, or abort, mark the message
   `failed`/`cancelled` and retain buffered partial text. Document that a process crash can leave
   a `streaming` row until a later recovery policy is implemented.
5. Add versioned routes under `/api/v1/assistant/threads` for:
   - paginated thread listing;
   - lazy thread creation and thread load/messages;
   - rename, archive, and delete if the UI exposes those actions;
   - an SSE run endpoint that accepts the prompt/files and returns the existing started/delta/
     completed/failed events plus stable thread/message identifiers.
6. Decide whether to preserve `/api/v1/query_model` temporarily as a compatibility wrapper. New
   frontend code should use the thread-scoped contract; avoid making a client-provided context
   snapshot authoritative once the server can load canonical thread history.
7. Wire the repository/service through `bootstrap.rs`. Add migration tests for schema creation,
   foreign keys, ordering, idempotency, archive filtering, partial/final states, and rollback.
   Add route/service tests for ownership checks, bounded context, SSE terminal behavior, disconnects,
   provider failures, and duplicate request IDs.

## Frontend implementation sequence

1. Extend `frontend/src/contracts/quarryApi.ts` with typed thread metadata, message/history
   payloads, thread mutations, and the updated stream input/events. Keep camelCase and the existing
   cleanup callback shape.
2. Implement the web adapter methods in `httpQuarryApi.ts` and the matching desktop methods in
   `tauriQuarryApi.ts`/Tauri relay. Update the desktop allowlisted paths/commands and contract tests;
   do not put persistence logic in Tauri.
3. Replace the single `useLocalRuntime(adapter)` setup with a custom remote thread-list runtime:
   - `RemoteThreadListAdapter.list` maps persisted thread metadata;
   - `initialize` creates/lazily initializes a server thread;
   - `rename`, `archive`, and `delete` call the server;
   - a per-thread `ThreadHistoryAdapter.load` fetches and maps messages;
   - `append`/`update` are idempotent reconciliation calls keyed by message ID, or are made
     intentionally no-op for server-owned run messages with that tradeoff documented.
4. Pass `runOptions.unstable_threadId`, `unstable_assistantMessageId`, and the current user
   message ID into the thread-scoped run request. The thread-scoped path does not send context;
   keep the existing bounded context calculation only for the legacy compatibility endpoint.
5. Complete the sidebar using `ThreadListPrimitive.Items`/item primitives (or a typed equivalent)
   so it shows title, active state, loading/error state, and keyboard-accessible selection. Keep
   `New chat` wired to the runtime. Preserve the assistant route and optionally reflect the active
   thread in a `thread` query parameter for refresh/back-forward behavior in a follow-up.
6. Handle visible loading, empty, failed history load, failed save, cancelled run, and successful
   completion states. If persistence fails after a streamed answer, show the answer but expose a
   retry/save error instead of silently claiming it was saved.
7. Key the provider/runtime by the resolved workspace identity so changing users cannot reuse the
   prior user’s thread cache. This is a lifecycle guard, not authorization.

## Verification plan

### Frontend

- Extend `queryModelAdapter` tests for thread/message IDs, cancellation, terminal reconciliation,
  and canonical-history loading.
- Add API adapter tests for list/load/create/rename/archive/run in both web and desktop mappings.
- Add sidebar/runtime tests for previous-chat rendering, switching, New chat, empty/loading/error
  states, and refresh restoration.
- Run the affected Vitest files, `npm run typecheck`, `npm run check:boundaries`, `npm test`,
  `npm run build:web`, `npm run check:web-bundle`, and `npm run build:desktop-ui`.

### Backend

- Add migration, repository, service, route, and integration coverage using disposable/in-memory
  SQLite only.
- Verify `cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
  `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets`.
- Do not use `cargo run` against the development database as a test.

## Architecture/documentation updates required with implementation

Update `docs/ARCHITECTURE.md` in the same implementation change to describe the new thread/message
tables, schema version, server-owned context flow, stream persistence/failure semantics, API
routes, frontend runtime adapters, and the remaining authentication/tenancy limitation. Update
the known-limitations table so it no longer says the transcript is wholly ephemeral, while still
calling out the absence of production identity, attachment reuse, resume-after-restart, rate
limits, and moderation if those remain out of scope.

## External library reference

The installed assistant-ui package is 0.15.20. Its current persistence guidance describes
single-thread `useLocalRuntime` as in-memory by default and recommends a custom
`RemoteThreadListAdapter` plus `ThreadHistoryAdapter` when the application owns its database:

- [assistant-ui Threads](https://www.assistant-ui.com/docs/runtimes/concepts/threads)
- [assistant-ui Custom thread persistence](https://www.assistant-ui.com/docs/integrations/persistence/custom-adapter)
- [assistant-ui ThreadHistoryAdapter API](https://www.assistant-ui.com/docs/api-reference/adapters/persistence)

These APIs should be confirmed against the lockfile/package version when implementation begins;
they are an integration aid, not Quarry’s persistence contract.
