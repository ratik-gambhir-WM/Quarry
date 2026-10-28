# Password authentication and session design

Implemented 2026-10-28 for the web and Tauri distributions. This document describes the current
identity boundary; organization membership, SSO, recovery, and production deployment remain future work.

## Credentials and sessions

Register with a distinct lowercase ASCII username (3–32 letters/digits/dots/underscores/hyphens,
starting with a letter or digit), a `@westmonroe.com` email, names, and password. Registration
accepts only that exact domain (case normalized); the UI shows its fixed suffix. This domain
allowlist is not proof of mailbox ownership; email verification remains unimplemented. Passwords accept 15–128 Unicode characters, at most 512 UTF-8 bytes, without
trimming or normalization. Optional job title is display data and never an authorization role.

Passwords are salted Argon2id PHC hashes (19 MiB, two iterations, parallelism one). A four-permit
worker semaphore stays held inside the blocking task, even if its HTTP caller times out. Sessions
use 32 random bytes encoded as base64url; SQLite stores only the SHA-256 token digest. SHA-256 is
not used for passwords. Each session expires after eight hours, allows explicit revocation, and
survives a backend restart. Expired records are rejected regardless of bounded cleanup progress.

The global `UserProvider` subscribes to one memory-only session store. It exposes user, ID, token,
expiry, login/register/logout, and session generation. No token/password is written to browser
storage, router state, logs, or persistent native storage. Reload or desktop restart requires
sign-in. This design does not protect against code already executing through XSS.

## HTTP contract

New clients use `/api/v1`; `/api` has identical authentication policy.

| Operation | Result |
| --- | --- |
| `POST /auth/register` | 201 `{user, accessToken, tokenType: "Bearer", expiresAt}` |
| `POST /auth/login` | 200 same session response; generic 401 on invalid credentials |
| `GET /auth/me` | 200 `{user, expiresAt}` for the current verified session |
| `POST /auth/logout` | 204 after revoking the current session |

Registration accepts `{username,password,email,firstName,lastName,jobTitle?}`; login accepts
`{username,password}`. Auth bodies are capped at 8 KiB, reject unknown fields, and normalize JSON
rejections to a sanitized 400 envelope. Duplicate username/email returns 409. Responses use
`Cache-Control: no-store`. Public user DTOs never contain password hashes or provider keys.

Protected requests carry `Authorization: Bearer <token>` and `X-User-Id: <id>`. The token determines
identity; mismatching ID returns 403. Missing, malformed, expired or revoked credentials return
401 with a Bearer challenge. A 403 does not log out the client. CORS allows both headers for the
existing explicit origins and handles preflight before authentication. Cross-origin redirects
are not followed by authenticated transports. Provider/template upstream clients never inherit
end-user credentials.

Register/login are limited to 20 attempts per peer per minute and login admission to five per
normalized username per minute; a successful login clears that username bucket. Limits and
active stream revocation notifications are process-local and bounded. They are not distributed
abuse protection. Real socket peer addresses are used; arbitrary forwarding headers are ignored.

## Resource and runtime boundaries

Axum middleware protects product routers before mounting either prefix. Only health,
capabilities, login and registration are public. Legacy user creation/email lookup, login demos,
server database diagnostics and arbitrary server-path summary routes are removed. Uploaded-file
summarization remains protected. There is no profile-lookup compatibility login.

A typed authenticated actor supplies trusted identity to handlers. Existing company/deal/chat use
cases receive identity resolved from that actor rather than a caller email. Company mutations
and assistant repositories enforce owner predicates. The HTTP boundary checks the canonical
owner before every deal-scoped route, including metadata extraction and filesystem resolution;
ownership is immutable in the current product. Deal lists are scoped in SQLite. Ingestion derives
its workspace from the actor's decimal user ID, and jobs retain that owner for SSE subscriptions.
Search overrides caller workspace scope and checks canonical live file/version ownership before
returning graph content. Missing/foreign resources return 404. Old identity hints in URLs never
establish authority; current clients omit them.

Templates remain a shared Quarry app catalog: every registered user can read and mutate it.
Research endpoints require authentication but external WM AI resources do not gain organization
membership policy from this change. These shared external capabilities need additional policy
before public deployment.

Web attaches credentials to JSON, multipart, binary/PDF/export and streaming requests. Document
GET-SSE uses fetch and AbortController so bearer headers are available. Assistant POST-SSE retains
its typed event lifecycle. Native commands validate a narrow per-request credential envelope;
a scoped client clone supplies fixed auth headers without mutable global defaults. HTTP status
survives IPC, including stream admission errors. Native document and query subscriptions support
explicit cancellation. Main-window/origin, API path, size, MIME and file checks remain enforced.

SSE bodies close on same-process revocation, expiry, or failed periodic session revalidation
(one-second interval). Dropping assistant receivers invokes existing cancellation/finalization;
underlying ingestion may finish durably but cannot be observed without a valid owner session.
Client logout/user switch cancels tracked requests/streams, clears activity records, and remounts
workspace state by generation. An old 401 cannot invalidate a newer login. Offline logout clears
local state and explains that server revocation was unconfirmed.

The `/_internal/template-preview` route stays outside the product session guard. It only renders
injected snapshots for Chromium and makes no authenticated API requests. It is not a secret
bypass and supplies no backend credentials.

## Schema 11 and development reset

`users` replaces `api_key`/authorization-looking `role` with username, password_hash and optional
job_title. `auth_sessions` owns token digests, user FKs, creation/expiry/revocation times and owner/
expiry indexes. Registration commits user and initial session together. SQLite remains canonical;
Helix is a recoverable projection, not an identity database.

Deploy the backend and updated web/desktop clients together: old profile-only clients cannot
authenticate against the new contract. There is no conversion from old profiles to passwords. Normal startup only initializes an empty
database or validates schema 11; older/newer/forged layouts fail without erasing records. Use the
explicit offline `reset_development --discard <absolute-disposable-database-path>` binary after
stopping producers. Reset discards all dependent product and embedded template records together,
including sessions, chats, blobs, previews, classification/embeddings and memberships. It is never
called automatically at startup. New data survives normal restart.

The single canonical local pair is `backend/data/quarry.sqlite3` and the launcher-managed
`helix-quarry-v3-dev` on loopback port 6969. Pair a deliberate SQLite reset with a fresh isolated
Helix store so recycled user IDs cannot expose old projections. The local launcher recreates the
selected container from the digest pinned in `backend/helix.toml`; inspect its mounts before
replacement. Leave unrelated test containers and historical standalone databases untouched.
There is no recovery of discarded records without an intentionally retained snapshot.

## Verification and limits

Use the frontend typechecks, boundary check, Vitest suite, web build/bundle check and desktop UI
build; backend fmt/check/Clippy/tests; and Tauri fmt/Clippy/tests. Security coverage includes
salted hashes, token digests, policy bounds, revocation/expiry, two-user resource access, removed
bypasses, auth logging exclusions, forms, stale-session 401s and client cancellation.

Open registration, shared catalog mutation, no email verification/recovery/MFA, memory-only
client sessions, process-local throttling, server TLS provisioning, and organization tenancy
remain explicit limits. Authentication is not a claim of complete production readiness.
