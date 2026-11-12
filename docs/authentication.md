# Password authentication and session design

Current storage is fresh PostgreSQL schema 1 with retained Helix. See
[PostgreSQL storage and Helix](postgresql-storage.md) for setup, tests and recovery; former
SQLite marker upgrades are historical and are not executed by this runtime.

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
use 32 random bytes encoded as base64url; PostgreSQL stores only the SHA-256 token digest. SHA-256 is
not used for passwords. Each session expires after eight hours, allows explicit revocation, and
survives a backend restart. Expired records are rejected regardless of bounded cleanup progress.

The global `UserProvider` subscribes to one session store. It exposes user, ID, token, expiry,
login/register/logout, and session generation. At the user's request, the session response is cached
in origin-scoped `localStorage` under `quarry.auth.session.v1` on both web and Tauri. Reloads,
development rebuilds, and restarts restore an unexpired session without extending its eight-hour
server expiry. The cache includes only the bearer token, token type, original expiry, and public
profile fields; passwords are never stored. Malformed or expired records are discarded. Blocked
storage falls back to memory-only sign-in.

The cache is untrusted UI state, not an authorization authority: every protected server operation
still verifies the bearer token, expiry, revocation, ownership, and the matching user header.
Logout, expiry, and protected-request 401s remove the cached session. Storage events propagate
session changes across tabs and cancel requests from the prior session. JavaScript running in this
origin can read the bearer token, so this does not provide HttpOnly-cookie or native credential-vault
protection against XSS. Provider keys and passwords remain absent from browser storage, router
state, logs, and client bundles.

## HTTP contract

New clients use `/api/v1`; `/api` has identical authentication policy.

| Operation | Result |
| --- | --- |
| `POST /auth/register` | 201 `{user, accessToken, tokenType: "Bearer", expiresAt}` |
| `POST /auth/login` | 200 same session response; generic 401 on invalid credentials |
| `GET /auth/me` | 200 `{user, expiresAt}` for the current verified session |
| `POST /auth/logout` | 204 after revoking the current session |

Registration accepts `{username,password,email,firstName,lastName,jobTitle?}`; login accepts
`{username,password}`, with `username` accepting either the registered username or email.
Login trims and ASCII-lowercases either identity and selects the corresponding unique user column;
usernames cannot contain `@`, so the two forms are unambiguous. Both columns already have PostgreSQL
unique constraints and registration normalizes them; no schema change is needed. Auth bodies are
capped at 8 KiB, reject unknown fields, and normalize JSON rejections to a sanitized 400 envelope. Duplicate username/email returns 409. Responses use
`Cache-Control: no-store`. Public user DTOs never contain password hashes or provider keys.

Protected requests carry `Authorization: Bearer <token>` and `X-User-Id: <id>`. The token determines
identity; mismatching ID returns 403. Missing, malformed, expired or revoked credentials return
401 with a Bearer challenge. A 403 does not log out the client. CORS allows both headers for the
existing explicit origins and handles preflight before authentication. Cross-origin redirects
are not followed by authenticated transports. Provider/template upstream clients never inherit
end-user credentials.

Register/login are limited to 20 attempts per peer per minute and login admission to five per
account per minute across both username and email; unknown users have a normalized-identifier
bucket. A successful login clears the account bucket. Limits and active stream revocation notifications are process-local and bounded. They are not distributed
abuse protection. Real socket peer addresses are used; arbitrary forwarding headers are ignored.

## Resource and runtime boundaries

Axum middleware protects product routers before mounting either prefix. Only health,
capabilities, login and registration are public. Legacy user creation/email lookup, login demos,
server database diagnostics and arbitrary server-path summary routes are removed. Uploaded-file
summarization remains protected. There is no profile-lookup compatibility login.

A typed authenticated actor supplies trusted identity to handlers. Existing company/deal/chat use
cases receive identity resolved from that actor rather than a caller email. Company mutations
and assistant repositories enforce private-owner predicates. Deal routes delegate operation policy
to canonical memberships: owner, manager, contributor or viewer. Creation atomically inserts the
owner; ownership transfer preserves creator provenance and demotes the prior owner. Removed
creators have no implicit access. Local roots require both current ownership and creator identity.
Shared company updates retain attached references or add actor-owned companies without opening
another user's directory. Personal notebooks/chats remain private regardless of deal links.
Ingestion uses canonical deal scope and separate authenticated uploader attribution, repeating
membership/archive checks in the commit transaction. Jobs allow any current member to subscribe;
search derives authorized deals and verifies current/live chunk identities before ranking and
before exposing text. Missing/foreign resources return 404, operation denials 403 and stale
management revisions 409. See [shared deal policy and recovery](shared-deal-projects.md).

Embedded templates are private to their authenticated owner. Catalog, asset, preview, export,
retrieval and retry operations enforce that scope; unassigned legacy records remain hidden.
See [template ownership](template-ownership.md).
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
document job streams also revalidate membership at least once per second and before delivery. Indexing accepted versions may finish, while uncommitted revoked uploads fail. Progress requires a valid session and current deal membership.
Client logout/user switch cancels tracked requests/streams, clears activity records, and remounts
workspace state by generation. An old 401 cannot invalidate a newer login. Offline logout clears
local state and explains that server revocation was unconfirmed.

The `/_internal/template-preview` route stays outside the product session guard. It only renders
injected snapshots for Chromium and makes no authenticated API requests. It is not a secret
bypass and supplies no backend credentials.

<a id="schema-15-and-development-reset"></a>
## Canonical storage and fresh deployment

PostgreSQL schema 1 includes password hashes, token digests, sessions, memberships and image
checkpoints alongside all product records. Registration commits the user and initial session
together. The existing password/session, per-resource ownership and consistency-header contracts
remain unchanged. Direct configuration requires a server-only `QUARRY_DATABASE_URL`.

This deployment starts with a fresh `Quarry` database and a separate
`helix-quarry-postgres-dev` store on port 6970. Existing SQLite files and the old port-6969 graph
are preserved; no profiles, passwords or documents are imported or reset. Startup initializes or
validates version 1 and refuses incompatible nonempty layouts without deletion. Follow
[PostgreSQL storage and Helix](postgresql-storage.md) for backups and drained recovery.

## Verification and limits

Use the frontend typechecks, boundary check, Vitest suite, web build/bundle check and desktop UI
build; backend fmt/check/Clippy/tests; and Tauri fmt/Clippy/tests. Security coverage includes
salted hashes, token digests, policy bounds, revocation/expiry, two-user resource access, removed
bypasses, auth logging exclusions, forms, stale-session 401s and client cancellation.

Open registration, no email verification/recovery/MFA, JavaScript-readable
client session persistence, process-local throttling, server TLS provisioning, and organization tenancy
remain explicit limits. Authentication is not a claim of complete production readiness.
