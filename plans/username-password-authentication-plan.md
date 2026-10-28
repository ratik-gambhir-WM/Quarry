# Username and password authentication implementation plan

Date: 2026-10-28. Status: implemented in the working tree.

Implementation update: schema 11, password/session APIs, per-user access checks, both transports,
UserProvider, login/register/account UI and verification are implemented. The authorized canonical
SQLite/Helix development pair was reset. User refinements add a wider responsive registration
layout and exact `@westmonroe.com` registration enforcement. See
[current authentication behavior](../docs/authentication.md) for the maintained contract.
The sections below retain the original implementation plan as design history.

Deliver username/password registration and login, an application-wide user/session provider,
credential headers across web and desktop transports, and Axum authentication plus resource
authorization. The user explicitly accepts losing development data instead of migrating it.
This document changes no runtime code or data and does not execute the reset.

## 1. Current implementation and constraints

The working tree was clean at planning preflight. Recheck before implementation and preserve any
subsequent changes. Code and [architecture](../docs/ARCHITECTURE.md) remain authoritative for
current behavior; this file describes proposed behavior.

| Current anchor | Finding and required change |
| --- | --- |
| [LoginPage](../frontend/src/pages/LoginPage.tsx), the former `components/login/LoginCard.tsx` | Email lookup enters the workspace without credentials. New profiles collect an unused provider API key. Replace both paths with explicit login/register forms. |
| [User repository](../backend/src/domains/users/repository.rs) | `users` stores and serializes `api_key`; there is no password hash. Separate private credential records from public user DTOs and remove the key field. |
| [Migrations](../backend/src/app/migrations.rs) | Current shared schema marker is 10; users have dependent deals, companies, chats, and document records. Rebuild the whole disposable schema through the existing offline reset rather than dropping only users. |
| [HTTP composition](../backend/src/app/http/mod.rs), [bootstrap](../backend/src/app/bootstrap.rs), [middleware](../backend/src/app/http/middleware.rs) | The same API is mounted at `/api/v1` and `/api`; no authentication exists. CORS currently allows content type and request ID headers only. |
| [Workspace session](../frontend/src/hooks/useWorkspaceSession.ts), [WorkspaceProvider](../frontend/src/app/WorkspaceProvider.tsx) | Router state/sessionStorage email is currently treated as identity. Replace that authority with a verified user/session. |
| [HTTP adapter](../frontend/src/api/httpQuarryApi.ts), [Tauri adapter](../frontend/src/api/tauriQuarryApi.ts) | There is no Axios dependency. Implement the requested interceptor behavior in the existing fetch and native relay layers without adding Axios. |
| [Native client](../frontend/src-tauri/src/quarry_api/client.rs), [stream request](../frontend/src-tauri/src/quarry_api/query_stream.rs) | JSON, multipart, bytes/export, document SSE, and assistant SSE take separate paths. All must carry the same credentials and retain structured HTTP status. |
| [RootRoutes](../frontend/src/app/RootRoutes.tsx), [App](../frontend/src/App.tsx) | Both router targets share product routes. The internal preview renderer is a separate read-only route used by backend Chromium and must continue working without an interactive user. |

Inspected manifests/locks show React `19.3.0-canary-ff7445e6-20260831`, Vite 7.3.6,
Axum 0.8, and a separate Tauri 2 crate. Preserve runtime aliases, React pins, strict TypeScript,
npm locks, and existing request size/path/MIME/PDF/stream contracts.

## 2. Concrete design decisions

### Identity and passwords

- Use a distinct username, not an implicit `@westmonroe.com` email local part. Registration
  collects username, password, email, first name, last name, and optional job title. Login accepts
  username/password only. Retain email as profile information; email is not proof of ownership.
- Canonical usernames are lowercase ASCII, 3–32 characters, matching `[a-z0-9][a-z0-9._-]{2,31}`.
  Trim and lowercase usernames before lookup/uniqueness checks. Trim/lowercase email consistently;
  keep a unique normalized email and validate its syntax and 320-character bound at HTTP input.
- Password policy: 15–128 Unicode characters, also capped at 512 UTF-8 bytes. Allow spaces, paste,
  and password managers; never trim, lowercase, normalize, or silently truncate passwords.
- Store an Argon2id PHC string with a fresh cryptographic salt per password. Initial parameters:
  19 MiB memory, two iterations, parallelism one; measure on target hardware before increasing.
  Use a maintained Rust implementation compatible with the crate's Rust 1.94 floor. Review
  dependencies/lock changes rather than copying a version from this plan. These parameters follow
  [OWASP password storage guidance](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html).
- Hash/verify on `spawn_blocking`, with a shared semaphore limiting concurrent Argon2 operations
  to four and bounded admission; reject excess work with 429. Hold the permit for the actual
  blocking task lifetime, including after an HTTP timeout. Missing users still run a dummy hash
  verification, and invalid usernames/passwords return the same 401 message.
- Remove the legacy `api_key` column, DTO field, account display, registration input, and associated
  adapter methods. Server provider keys stay server-side. A job title is display data and grants
  no administrator privileges; do not accept a registration-supplied authorization role.

### Session model and lifecycle

- Choose opaque bearer sessions backed by SQLite rather than JWTs: revocation and expiry are
  directly enforceable with the existing canonical store, without introducing signing keys.
- Generate a fresh 32-byte CSPRNG token on each successful registration/login; encode base64url
  without padding. Store only its SHA-256 digest in SQLite. SHA-256 is for high-entropy token
  lookup, never password storage. RNG failure must fail the request.
- Sessions have an eight-hour absolute lifetime, no automatic sliding extension, and immediate
  revocation on server logout. Multiple concurrent logins are allowed. No refresh token,
  remember-me, password reset, SSO, MFA, or email verification is included in this increment.
- The UI owns user/session/token state in memory only, with no password or token in localStorage,
  sessionStorage, router state, URL, activity log, or fixture data. Reload/app restart requires
  login. Native code receives per-request credentials, stores no persistent token, and never sets
  a mutable global Authorization default. A future persistence feature needs an explicit web
  cookie/native credential-store design. The memory-only choice satisfies header transport while
  avoiding browser storage persistence; it does not eliminate the consequences of XSS.
- Send `Authorization: Bearer <token>` and `X-User-Id: <id>` on protected calls. The session token
  determines the authenticated user; the separate ID is a consistency check only. Missing or
  malformed credentials return 401; a valid token with a different user ID returns 403. Never
  select identity from the header ID, email, body, or query alone.
- Return `Cache-Control: no-store` on auth/current-user responses; never echo credentials in
  errors. Unauthorized responses carry an appropriate Bearer `WWW-Authenticate` challenge.
  Transport requires HTTPS outside existing loopback development exceptions. Do not forward
  credentials through redirects to another origin or to upstream AI/template services.
- Follow [OWASP session guidance](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)
  for random tokens, server expiry, invalidation, and credential confidentiality.

## 3. Database structure and deliberate development reset

Propose schema marker **11**, provided it remains the next unused marker at implementation time.
Do not implement an old-profile-to-password migration or manufacture passwords for existing users.

| Table | Proposed structure/invariants |
| --- | --- |
| `users` | Retain integer `id`, names, normalized unique email, timestamps; add normalized unique `username` and non-null `password_hash`; replace free-form `role` with optional display-only `job_title`; remove `api_key`. |
| `auth_sessions` | `token_hash` primary/unique key, `user_id` FK to users with delete cascade, `created_at`, `expires_at`, nullable `revoked_at`; index owner and expiry; enforce expiry after creation. Store UTC timestamps consistently. |
| Existing application tables | Recreate existing company/deal/chat/file/template structures and invariants; preserve owner FK types and all non-auth fresh-schema definitions. Add explicit owner fields to process-local jobs rather than assuming a random job ID provides authorization. |

Implement registration's user/session inserts in one SQLite transaction through the existing
SQL builder/client. Unique constraints resolve concurrent registrations; map conflicts to 409
without database details. Repository methods expose separate public-user and private credential
records; credentials must not derive unrestricted `Debug` or `Serialize`. Expired/revoked session
cleanup is bounded maintenance, never a prerequisite for rejecting an invalid session.

Reset procedure during implementation:

1. Stop/drain the selected development backend, ingestion/classification jobs, and chat streams.
   Confirm the exact disposable SQLite file and associated Helix instance through deliberate
   environment selection; do not infer a path by reading `.env` during ordinary code work.
2. Update fresh-schema DDL, current-layout validation, and the offline reset drop order to include
   sessions and all dependent application/template tables. Normal startup initializes a fresh
   empty database or reuses validated marker 11; old markers fail with an actionable offline-reset
   message. Remove obsolete 9→10 automatic upgrade handling deliberately and update its tests/docs.
   Unknown/newer versions must still fail without deletion.
3. Execute the existing `reset_development --discard <absolute-disposable-database-path>` binary
   against that selected database only. Its reset transaction drops/recreates tables, verifies
   foreign keys, and commits marker 11. No data backfill, silent startup reset, or broad file deletion.
4. Pair it with a **fresh isolated Helix store** before restarting. This also discards the selected
   development graph from active use; never reuse old projections with new integer user IDs.
   Do not globally clear an unverified/shared Helix store or touch historical standalone template
   databases. Shared SQLite reset also loses template assets/previews/classification/memberships;
   reimport/reseed supported template content afterward as needed.
5. Clear obsolete workspace-email client storage through application code, reload clients, and
   register new accounts. New accounts and data must survive subsequent normal restarts.

The user's instruction authorizes discarding this development data; executing the reset is still
a separately staged implementation operation with an explicitly selected target, not this plan's
verification step. Rollback cannot restore discarded records; a fallback requires a deliberately
retained snapshot or a new empty compatible environment. Never downgrade markers in place.

## 4. API, Axum middleware, and resource authorization

All new clients use `/api/v1`. Apply identical security policy to the temporary `/api` mount;
leaving its legacy endpoints public would bypass authentication.

| Method and path | Request | Result |
| --- | --- | --- |
| `POST /auth/register` | `{username,password,email,firstName,lastName,jobTitle?}` | 201 `{user,accessToken,tokenType:"Bearer",expiresAt}`; immediately signed in |
| `POST /auth/login` | `{username,password}` | 200 same session response; generic 401 for invalid credentials |
| `GET /auth/me` | Protected headers | 200 `{user,expiresAt}`; current database profile, no hash or token |
| `POST /auth/logout` | Protected headers, no body | 204 after revoking the current session; invalid/expired token gets 401, which clients treat as already signed out |

Public `user` contains `id`, `username`, `email`, `firstName`, `lastName`, `jobTitle`, `createdAt`,
and `updatedAt`. Use camelCase DTOs and existing sanitized error envelopes; extend typed errors
for 401/403/429. Normalize auth JSON/extractor failures deliberately rather than promising a
universal envelope for unrelated existing endpoints. Reject unknown auth fields and bound body
size to 8 KiB. Validation errors are 400, identity conflicts 409, throttles 429 with Retry-After,
and storage failures sanitized 500. Do not provide username/email existence lookup endpoints.

Use `domains/identity/` for auth handlers/routes/service/session repository/password types,
`domains/users/` for public profile persistence, and an HTTP extractor/middleware for a typed
`AuthenticatedUser`. Config/bootstrap construct and inject dependencies; repositories/services
must not import AppState, construct clients, or read ambient environment. HTTP handlers validate
transport input and pass trusted user IDs into use cases.

Compose explicit public and protected routers in bootstrap before the two API mounts. Public
allowlist: shallow health/capabilities and register/login. Remove old `/users` creation and
`/users/by-email` lookup routes and login-demo bypasses; do not retain them as public compatibility.
Protect every other product route by default. Disable sensitive development diagnostics and
unscoped filesystem-summary operations in the authenticated router until access is constrained.
Keep CORS preflight outside auth rejection and allow Authorization, X-User-Id, content type, and
request ID only for the configured origins. Retain request IDs/timeouts/compression/body limits.

Add bounded login/register abuse protection: initial per-peer limit of 20 attempts/minute and
per-normalized-username limit of 5 failed logins/minute, with 429/Retry-After. Use bounded,
expiring process-local buckets and concurrency admission. Do not trust arbitrary forwarding
headers for IP identity. Document single-process limits; distributed enforcement is deferred.

Authentication is insufficient without these ownership changes, included in this implementation:

| Resource | Required policy |
| --- | --- |
| Profile/account | Read the authenticated user through `/auth/me`; no arbitrary email profile retrieval. |
| Companies/deals | Create/list/update/archive and metadata operations use the trusted owner ID; check ownership before extraction, data-room access, or other expensive side effects. Foreign-company validation stays transactional. |
| Assistant | Remove caller-supplied identity email from thread CRUD and run multipart DTOs; pass authenticated owner IDs to existing owner-filtered repository operations, including linked-deal checks. |
| Documents/data rooms/previews | Resolve the owning deal and verify owner before listing, reading bytes/text, converting, ingesting, or resolving filesystem roots. Remove upload `userId` authority; derive it from the actor. |
| Search/Helix | Derive allowed scope from canonical ownership, not caller `workspaceId`; filter returned graph hits against live owned SQLite files/versions before returning text or sending it to a model. Choose one deterministic user-to-workspace mapping for fresh ingestion and search and test it. |
| Document jobs and streams | Record owner, deal, and session association at job creation; restrict SSE subscription to that owner. A guessed job ID must not disclose progress/content. |
| Templates/export/research | Require authentication. Keep the existing Quarry app-scoped template catalog explicitly shared by all registered users in this increment; document that authorized users can mutate shared templates. Never forward session headers upstream. |
| Filesystem summaries/dev endpoints | Authentication alone does not authorize caller-selected server paths. Disable arbitrary-path endpoints for this increment, or replace paths with owned deal/file references before enabling them. |

Use 404 for another user's resource to avoid confirming existence. Avoid time-of-check/time-of-use
gaps by applying owner predicates in persistence reads/mutations and retaining owner identity in
queued work. This increment is per-user isolation, not organization/workspace membership or RBAC.

Streams authenticate before opening and close on session expiry. Add a session revocation signal
for active connections so logout terminates that session's subscriptions/runs; fail closed if
session revalidation fails. Underlying ingestion can finish durably as owner-scoped work after
disconnect, but its events remain inaccessible without a valid owner session. Define assistant
cancel/finalization on logout using existing terminal-state handling. Verify same-process logout
revocation and periodic database revalidation for cross-process changes; this is not a distributed
stream coordinator.

## 5. App-wide provider, login/register UI, and request credentials

Add typed auth contracts to `contracts/quarryApi.ts` and implement register/login/me/logout in
both adapters. Replace AddUserInput/profile lookup methods and update every consumer/test.

- Create a small transport-neutral session store with an immutable session generation and
  subscription API. `UserProvider` exposes `user`, `userId`, `token`, `expiresAt`, status,
  `login`, `register`, and `logout`; `useUser` fails clearly outside its provider. The store is
  the single source of truth, not a second provider-owned copy. The provider subscribes through
  `useSyncExternalStore`; transports read a credential snapshot without importing React.
- Mount the provider above the product route tree, including login/register and `/hub/*` for
  both BrowserRouter and HashRouter builds. The internal `/_internal/template-preview` route
  stays outside the product guard and requires no API session: it renders injected snapshots
  only, performs no protected API requests, and exposes no secret bypass token.
- Initial state is anonymous because sessions are memory-only. Login/register requests have
  explicit pending/error/success transitions; expiry, user logout, or current-session 401 clears
  the session. Treat network/500 errors as recoverable failures, not proof of invalid credentials.
  A 403 must not trigger global logout. Never automatically replay a failed mutating request.
- Protected routes render only for a current session; unauthenticated navigation redirects to
  login with a validated internal return path. Successful auth returns there or `/hub`; reject
  external/protocol-relative destinations. Anonymous deep links never start protected requests.
- Key WorkspaceProvider and its request/cache owners by session generation, abort old requests,
  dispose chat/PDF/preview subscriptions, and clear sensitive activity/cache data on logout or
  user switch. Ignore old responses and old-session 401s after a new login. The server owner ID
  and token travel together as one captured request credential snapshot.
- Replace `useWorkspaceSession`'s storage/router identity with provider-derived profile data;
  remove `quarry.workspace.email` persistence and identity email/ID arguments from APIs. Existing
  navigation state can hold navigation hints only. AccountPage shows current user profile and
  sign-out; remove its API-key masking and duplicate email-based profile lookup.
- `/login` and new `/register` provide explicit links between forms, labeled username/password
  controls, `autocomplete="username"`, `current-password`/`new-password`, optional password
  visibility, registration confirmation, inline validation, and live error/status announcements.
  Confirm-password stays client-only. Prevent duplicate submits; cancel/ignore stale requests
  after leaving a form, clear password inputs after completion, and preserve focus/keyboard
  behavior. Reuse current primitives/tokens and test responsive/theme/reduced-motion behavior.
- Logout first attempts server revocation while credentials still exist, then always clears
  local credentials/caches. If offline, show that local sign-out succeeded but server revocation
  was unconfirmed; the remote session expires at its absolute deadline.

## 6. Complete transport coverage

### Web

Centralize request credential construction under `src/api/` and apply it to **every** protected
fetch path: JSON, DELETE, multipart, PDF/text/binary downloads, PowerPoint export, and both stream
types. Public auth requests omit any stale bearer. Preserve automatic multipart boundaries.

Native browser EventSource cannot attach custom Authorization headers. Replace document-job
EventSource with fetch-based GET SSE and AbortController, reusing/extending the existing parser
where appropriate while preserving document `processing/completed/skipped/failed` semantics,
keepalive handling, UTF-8 boundaries, cancellation, and bounded reconnect behavior. Assistant
POST-SSE retains its existing event order and cancellation contract. Never place bearer tokens
in query strings. A 401 during stream admission reaches the shared session invalidation path.

### Desktop

Extend the stable runtime transport contract, `runtime.desktop.ts`, Tauri API adapter, command
arguments/models, services, HTTP client, and both stream relays together. Use a narrow optional
auth envelope on request commands, required for protected calls; do not add arbitrary header
injection or a generic native escape hatch. Validate token encoding/length and positive bounded
user IDs, retain main-window/origin/path validation, and construct only the two allowed auth
headers in Rust. Treat supplied headers as untrusted until Axum verifies them.

Preserve HTTP status/error codes across IPC (current string-only errors need a typed error
payload), including stream handshake failures, so 401 handling matches web. Do not parse human
error strings for status. Keep existing native capability/CSP restrictions and `/api/v1` path
rules. Update command registration and tests if signatures change.

Audit activity logging **before** adding credentials. Auth request bodies, auth responses, bearer
headers, password/hash/token fields, and credential-bearing IPC envelopes must never be recorded
or persisted. Add exact regression tests for nested envelopes and response fields; log auth
operation/status/duration only. Apply the same redaction discipline to Rust Debug/tracing and
error formatting. Keep server provider keys out of all IPC arguments and client configuration.

## 7. Implementation sequence and verification

1. **Contracts and security boundaries:** define DTOs, typed errors, session states, endpoint
   inventory/public allowlist, resource policies, and the shared template policy. Add failing
   contract/ownership tests and review dependency choices.
2. **Persistence and identity:** fresh schema/reset support, private credential/public user
   records, transactional registration, password worker/admission, session creation/revocation,
   expiry and limiter tests. Use temporary databases only.
3. **Backend enforcement:** mount public/protected routers, middleware/extractor, ownership across
   existing domains, job/session stream lifecycle, header/CORS policy, removal of legacy bypasses.
   No intermediate release with public compatibility routes or trusted email identity.
4. **Web/desktop transports:** auth methods, per-request snapshots, all request categories,
   fetch GET-SSE, typed IPC status, cancellation/revocation, and logging exclusions.
5. **Provider and UI:** global user store/provider, protected routes, login/register/account,
   session-keyed workspace state, stale request protection, and logout/expiry feedback.
6. **Full review and disposable smoke tests:** complete checks below; inspect web and desktop;
   update architecture/recovery docs. Then carry out the selected development reset, restart
   with fresh Helix, and verify registration/login plus persistence across restart.

Focused tests must prove:

- Different salts for identical passwords; no plaintext/password hash/token in public DTOs or
  logs; unknown user/wrong password share response; policy limits; normalized duplicates and
  concurrent registration; transaction rollback; hashing concurrency and limiter bounds.
- Sessions reject forged/malformed/missing/revoked/expired tokens, survive backend restart until
  expiry, never store raw tokens, enforce header-ID consistency, and revoke only the selected
  session. Inject clocks/randomness for deterministic failure/expiry tests.
- Fresh schema, old-version rejection without mutation, reset drop order, FK constraints and
  rollback, session cascade, and post-reset startup retention. Include new modules via existing
  `#[path]` hooks because backend `autotests = false`.
- A two-user route matrix: unauthenticated requests fail, owner succeeds, other owner gets 404,
  spoofed email/body/header/workspace IDs cannot change authority, and `/api` cannot bypass
  `/api/v1` security. Cover companies/deals, metadata, local roots, documents, jobs, search,
  thread CRUD/streaming, and the documented shared catalog policy.
- Every adapter category receives correct headers and preserves bytes/multipart/event behavior;
  preflight works; SSE never uses query credentials; native errors retain status; expiry/logout
  closes streams; failed session validation does not leave readable streams.
- UI register/login/logout, invalid credentials/conflicts/offline/retry, validation and password
  autocomplete, protected deep links, reload-to-login, expiry, failed logout revocation, and
  rapid logout/login or user switch. Stale responses and 401s cannot restore or clear a new
  session. Old workspace email storage cannot authenticate anyone.
- Existing internal preview renderer still works with injected snapshots and no session; auth
  headers are never forwarded to Diligence Studio, providers, arbitrary origins, or log storage.

Start with actual affected Vitest paths and backend test-name filters; then run these existing
gates from their stated roots (these are implementation checks, not claims of checks run now):

```sh
# backend/
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# frontend/
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# frontend/src-tauri/
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# repository root
git diff --check
git status --short
```

For runtime validation use `npm run dev:web` and `npm run dev:desktop` with an explicitly
disposable backend/Helix configuration. Inspect login/register/account, protected navigation,
actual header/IPC/status behavior without capturing credential values, uploads/downloads,
streaming, keyboard/focus, relevant viewports, supported themes, and console/network errors.
Record and stop only task-owned listeners. Do not start ordinary backend bootstrap or reset
real data as a routine test; never use global Helix clearing as verification.

## 8. Documentation, completion, and remaining limits

During implementation update `docs/ARCHITECTURE.md` for identity/session ownership, routes and
guards, DTOs/headers/CORS, every transport hop, schema 11 and offline reset, jobs/streams,
resource isolation, shared templates, removed development endpoints, logging, and remaining
limitations. Update [migration recovery](../docs/slide-templates-migration.md) and
[deal/company operations](../docs/deal-company-model.md) for the destructive reset policy.
Add a scoped authentication design record under `docs/` covering token storage, expiry/revocation,
ownership, shared catalog access, and reset consequences. Correct stale project skill claims
about old schema versions when they conflict with the final implementation.

Done means both web and desktop can register, log in, access only permitted server resources,
stream/upload/download with verified credentials, sign out, and reject expired sessions; new data
survives restart; old profile lookup cannot enter the app or bypass middleware; all applicable
checks pass and both runtimes have been inspected. Report any skipped integration inspection
explicitly rather than treating compilation as runtime evidence.

This increment does not deliver organization tenancy, authorization roles, invite-only signup,
email verification, account recovery, MFA/SSO, persistent client login, distributed rate limits,
or production deployment/TLS provisioning. Open self-registration and shared template mutation
are explicit product choices for this development increment, not a claim of production readiness.

Architecture impact of this planning change: none — the current architecture was re-read and
only a proposed implementation document was added; no implemented contracts or data changed.
