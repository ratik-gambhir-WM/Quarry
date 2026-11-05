# Sign in with ChatGPT and delegated inference implementation plan

Date: 2026-11-04. Status: proposed; this document implements no authentication or inference changes.

## 1. Outcome and feasibility

Add **Continue with ChatGPT** as another Quarry sign-in method. Separately let an eligible
user authorize interactive Quarry chat requests against their ChatGPT plan. Preserve password
sign-in and Quarry's own sessions, resource authorization, and existing server-funded AI features.

The requested token reuse is supported for eligible inference **only with the separately granted
OAuth access token**. Identity scopes and an ID token alone do not authorize inference.
The documented route is `POST https://api.openai.com/v1/responses`; do not use private ChatGPT
endpoints or read credentials from ChatGPT/Codex installations. See [quickstart][S1] and
[models and inference][S4].

There are two deployment paths, with different registration rules:

| Deployment | Integration prerequisite | Plan decision |
| --- | --- | --- |
| Hosted web or commercial/private deployment | Selected-partner access and a registered client; delegated plan usage must be approved as well as identity sign-in. | Treat access, callback registration, and the partner's exact delegated authorization contract as a release gate. |
| Eligible open-source or personal application running locally | The documented dynamic-client flow supports loopback authorization and user-approved plan usage. | A possible local proof of concept, after confirming Quarry's distribution qualifies; not a workaround for hosted access. |

The public docs distinguish these paths. Do not assume this repository's visibility, an internal
deployment, or Tauri packaging establishes eligibility. Confirm intended distribution with the
product owner and OpenAI before implementation. If partner access is needed, prepare a request
through the [interest form][S10]; submitting it is a separate action. [S1], [S2], [S3]

Published user eligibility differs across the commercial help article and developer preview
guides. Do not hard-code a subscription-tier test or promise Enterprise/Business plan funding.
Use the actual grant and account-specific provider responses. Sign-in may be available while
plan usage is unavailable. [S1], [S11]

## 2. Current implementation and stable boundaries

| Source | Current behavior and implementation consequence |
| --- | --- |
| `backend/src/domains/identity/{route,handler,service,repository,model}.rs` | Password registration/login issue eight-hour Quarry bearer sessions. Add external identity resolution and reuse session issuance; do not accept OpenAI tokens as Quarry bearer tokens. |
| `backend/src/app/http/auth.rs` | Validates Quarry sessions and the matching `X-User-Id`; downstream authorization uses the authenticated actor. Preserve this trust boundary. |
| `backend/src/app/migrations.rs` | Shared SQLite schema marker is 16. Users require a password hash and unique username/email. Federated-only accounts need an explicit additive credential migration. |
| `backend/src/adapters/openai/client.rs` | One injected `OpenAiClient` holds a server API key. Chat already sends array input, instructions, `store: false`, and `stream: true`. Add a distinct per-user credential path without replacing this global key. |
| `backend/src/domains/assistant/chat/{service,handler,model}.rs` | Persistent and ephemeral chat use that client; ephemeral `ask` currently has no explicit authenticated actor argument. Thread ownership and actor context must flow into credential selection for both entry points. |
| `backend/src/app/{config,bootstrap,state}.rs` | Configuration and dependency construction live here. Inject OAuth clients, credential storage, and inference routing into services; do not introduce ambient configuration reads below this layer. |
| `frontend/src/auth/UserProvider.tsx`, `sessionStore.ts`, and `persistedSession.ts` in that folder | Retains the Quarry session in origin-scoped local storage. Extend only public connection state; never add OpenAI access/refresh/ID tokens to this cache. |
| `frontend/src/api/http/{auth,assistant,transport}.ts`, `httpQuarryApi.ts`, `tauriQuarryApi.ts` | The current uncommitted HTTP refactor composes domain APIs. Add new methods to the domain modules and both adapters, not back into the composition file. |
| `frontend/src-tauri/src/quarry_api/` and `security.rs` | Validated IPC and transport remain native responsibilities. Business rules, token exchange, provider calls, and durable state stay in Axum. |

Build roots affected by implementation: `backend/`, `frontend/`, and potentially
`frontend/src-tauri/`. No Diligence Studio server changes are planned. Preserve existing
uncommitted API refactor work. This plan is a design input; current behavior remains described by
[architecture](../docs/ARCHITECTURE.md) and [authentication](../docs/authentication.md).

## 3. Identity, sessions, and credentials

Keep these credentials separate:

| Artifact | Purpose | Owner |
| --- | --- | --- |
| Verified OpenAI ID token claims | Resolve an external identity at sign-in; retain only necessary claims, plus a protected ID token if the chosen flow needs reauthorization hints. | Axum identity integration |
| OpenAI access/refresh token set | Authorize eligible provider requests and renew the grant. | Protected Axum credential store |
| Quarry bearer session | Authenticate Quarry API calls and enforce local resource policy. | Existing identity service and session store |

Map external identities by verified `(issuer, client_id, subject)` to a local user. Email is
display/onboarding information, not an account key or authorization proof. Linking an existing
account requires a current Quarry session plus recent reauthentication and an explicit linking
intent bound to the OAuth attempt. An email collision during unauthenticated sign-in requires
the person to authenticate the existing account; never merge automatically. Distinct dynamic
registrations remain distinct even when their emails match. [S2], [S6]

For new federated accounts, collect any required profile fields the provider omits. Preserve
username/email uniqueness and existing local user IDs. Do not invent a password or hash to
satisfy the current schema. Proposed migration: permit an absent password hash for accounts
with a verified external identity; password login must reject absent hashes through the normal
generic failure path and retain dummy-hash timing protection. Unlinking must not remove the
last usable sign-in method. Password reset/enrollment is not implicitly introduced by this work.

Issue the existing `SessionResponse` only after verification, account resolution, and required
onboarding. Preserve the eight-hour policy and existing cancellation/session-generation logic.
OpenAI disconnection and Quarry logout are distinct operations; explain each in the UI. [S2], [S6]

## 4. OAuth flow in web and desktop

### Hosted partner flow

1. Axum creates a short-lived, one-use attempt containing intent (`login`, `link`, or
   `enablePlanUsage`), fresh state, nonce, PKCE verifier/challenge, exact callback, environment,
   and the initiating account/session when present. Bound attempts, expiries, and initiation rate.
2. Bind browser authorization to a temporary HttpOnly/Secure/SameSite transaction cookie;
   use an allowlisted return route. Open the authorization URL in the system browser for Tauri.
3. Callback handling validates state, browser binding, expiry, duplicate parameters, and provider
   errors before redeeming a code. Consume attempts atomically; reject replay and mismatched
   linking sessions. Treat callback codes and authorization URLs as sensitive logging data.
4. The backend exchanges the code using the registered authentication method. Use a client
   secret only for a provisioned confidential client, stored server-side. Verify ID-token
   signature, issuer, audience, nonce, and time claims with a maintained OIDC/JWT implementation;
   allowlist algorithms and configured discovery/JWKS origins. [S2]
5. Deliver the resulting Quarry session through an authenticated completion exchange bound
   to the original client attempt. Use a high-entropy completion proof retained in that client's
   memory, hashed server-side; a public attempt ID or callback URL alone cannot redeem it.
   Never redirect with Quarry or OpenAI tokens in a query or fragment. For desktop, this proof
   binds the system-browser result to the initiating webview without relying on shared cookies.
6. Add only a narrow native browser-opening capability if needed. Validate the expected main
   window/origin and exact configured authorization origin; no arbitrary URLs, shell commands,
   broad filesystem permissions, or provider credentials across IPC.

The web transaction-cookie policy must be exercised against the actual frontend/backend origin
topology; CORS is not CSRF protection. Abort on session change, close/cancel, or timeout. Partial
success must not attach a grant to another user or let a stale completion replace a newer session.

### Eligible local flow

Keep Axum as the token/inference owner. A dedicated Axum-managed callback listener binds only to
`127.0.0.1` and starts before opening the browser. Generate a stable installation host ID;
it identifies an installation, not a user or an authorization grant. Initial dynamic registration
uses the documented bootstrap client; retain the issued client ID even if later exchange fails.
Returning authorization reuses that registration and verifies that the new identity matches it.
Use fresh state, nonce, and PKCE every time; preserve the exact callback URI within an attempt.
Do not substitute this loopback scheme for a hosted partner callback. [S3]

The local runtime must be restricted to its intended local user/environment before accepting
OAuth connections. A remote Axum server cannot receive a callback to the browser machine's
loopback address. The self-hosted VM guide describes credential transfer, but that is not an
approved multi-user hosted login design; exclude manual credential copying from Quarry's UX.
[S9]

## 5. Persistence and token lifecycle

Proposed SQLite records, finalized against the live schema before coding:

- `external_identities`: local user FK, provider, issuer, client ID, subject, optional session
  reference, created/last-verified timestamps; unique verified external identity tuple.
- `provider_connections`: owning user and identity FKs, granted scopes, state, selected model,
  expiry, credential version, revocation marker, encrypted token material or a secret-store handle.
- `oauth_attempts`: hashed state/completion proof, encrypted temporary verifier/nonce, intent,
  callback, owner/session binding, expiry, and atomic completion/consumption state.
- Session linkage for issuer/client/subject and optional `sid`, allowing targeted security logout.

Provider credentials must be recoverable for outbound calls, so token digests alone are
insufficient. Use authenticated encryption with a reviewed library and a backend-owned key from
an OS secret store or deployment secret manager; store a key version with ciphertext and bind
it to owner/connection metadata. Do not store the encryption key beside ciphertext in SQLite,
`.env` committed files, `VITE_*`, browser storage, Tauri payloads, or logs. Fail startup validation
for an enabled but incompletely configured integration. An unreadable old key must not overwrite
or silently delete credentials. Define key rotation and encrypted-backup recovery before rollout.

Honor token response expiry/refresh metadata rather than assuming permanent access. Serialize
refresh per connection, atomically replace the access token and rotating refresh token, and
reject stale writes after disconnect/account switch. Start with the repository's single-host
deployment assumption; use database fencing/leases before enabling multiple token refresh owners.
Do not hold a blocking mutex or SQLite transaction across network awaits. Bound refresh retries
and distinguish permanent invalidation from temporary provider/network failure. [S6], [S7]

On explicit provider disconnect, stop new calls, cancel applicable active streams, attempt
remote renewable-session revocation, and clear local credentials. Report unconfirmed remote
revocation if offline. Ordinary Quarry logout revokes its session and cancels its streams; it
does not silently disconnect an independently authorized provider connection. Make this policy
visible and prevent background use of a disconnected grant. [S6]

## 6. Route chat through a user-scoped inference service

Introduce an injected inference router plus narrow credential/provider adapters. Carry the
authenticated actor and a validated connection selection into both persistent and ephemeral
chat. Resolve the connection with an owner predicate on the server. An opaque connection ID or
client-supplied funding choice is a selector, never authority. Freeze actor, connection, funding
source, and credential generation for each run; account switching cancels rather than retargets it.

Use an explicit credential enum or equivalent typed boundary for server-key versus delegated
calls. Keep request headers local to the request; never mutate the singleton client's key or
global default headers. Cache model discovery by owner/connection/grant generation. Obtain
displayable models from the delegated model catalog and send their `slug`; do not assume it has
the standard API-key catalog shape or that Quarry's default/custom model is available. [S4]

For delegated HTTP inference, preserve array history and instructions, `store: false`, and
`stream: true`. Maintain existing timeouts, bounded channels, file limits, cancellation, and
persisted-run finalization. Require `response.completed` for success; incomplete, failed, and
prematurely closed streams must not save a completed answer. Preserve partial output as partial.
Validate the delegated parameter allowlist separately: the preview rejects several otherwise
valid API parameters and capabilities. [S4], [S5]

| Quarry workload | First release funding policy |
| --- | --- |
| Ephemeral and persistent interactive chat | User explicitly selects ChatGPT plan or existing Quarry service funding; enable delegated calls only with the required grant and an available model. Start with text; gate attachments by model and verified input support. |
| Document embeddings and vector search preparation | Keep existing server-key embeddings. Delegated Responses access does not establish embeddings access. |
| Summary/extraction/image-description services | Keep existing server funding initially; adopt delegated streaming only through separate capability and consent work. |
| Durable notebook save generation | Keep server funding initially; do not silently spend a user's plan from durable jobs. |
| Shared template classification/embeddings and legacy Diligence Studio | Keep existing server providers. Shared worker records currently lack a suitable user-specific grant/funding contract. |
| WM AI and SharePoint | Separate credentials and capabilities; no token substitution. |

Never silently change funding after grant denial, expiry, model mismatch, limit, or provider
failure. Offer an explicit action to choose an available alternative; keep the prompt and files
for retry. ChatGPT-funded chat must work without `OPENAI_API_KEY` when correctly configured;
unrelated server-funded capabilities still fail explicitly when that key is absent.

## 7. API, UI, and failure behavior

Proposed `/api/v1` operations; finalize DTOs and exact route names before implementation:

| Operation | Contract |
| --- | --- |
| Public auth capability read | Advertise configured sign-in/plan-use modes without exposing secrets or claiming user eligibility. |
| Start sign-in / protected start link or grant consent | Return an opaque attempt and browser launch information; login and linking have distinct authorization requirements. |
| Provider callback | OAuth-only public callback, transaction bound, no-store, bounded input; never accepts an arbitrary bearer token as a login shortcut. |
| Finish attempt | One-use proof-bound completion; returns existing Quarry session or safe connection result. Rate-limit status polling and keep proofs out of URLs/logs. |
| List/select/disconnect connections | Protected, owner-scoped; expose only display identity, scopes-derived capabilities, status, and model metadata. |
| List available chat models | Protected and connection-scoped; no credential or raw upstream auth details in the DTO. |
| Existing chat/run endpoints | Add optional validated funding/connection selection with an explicit compatibility default to server funding. Preserve event names. |

Extend `contracts/quarryApi.ts`, the new HTTP auth/assistant modules, desktop API mapping, and
native relay validation together. Add optional sanitized provider `code` and recovery action to
chat failures if needed; current backend failed events expose only a static error string.
Do not map an upstream OpenAI 401 to a Quarry authentication 401: that would incorrectly clear
the user's valid Quarry session. Distinguish `reconnectProvider` from `signInToQuarry`.

UI work: add the branded accessible sign-in action and linked-account settings. Show identity
connection and plan-use permission separately, the selected provider account/model/funding
source, and a Manage usage link. Include loading, denied/cancelled, linking-conflict, disconnected,
unsupported-model, offline, and retry states. Preserve keyboard focus and existing password login.

For structured provider errors, map eligibility/policy denial to a clear stop, usage-limit errors
to Manage usage, unsupported capabilities to a corrected request, and temporary availability
failures to bounded retry. Preserve safe request IDs and machine-readable codes while redacting
raw bodies, tokens, email, document text, and callback parameters. Limits can fail after streaming
begins; do not treat received deltas as success. Repeated sign-in does not restore usage. [S8]

For partner identity sign-in, implement and register OpenAI's signed back-channel logout before
production rollout when provisioned: validate issuer, audience, signature, event, freshness, and
replay ID; revoke only matching user/session scope and associated renewable credentials. Unknown
session IDs must not trigger broad logout. Separately, plan-use disconnection currently has no
general push notification; detect invalidation through requests/refresh and local disconnect.
[S8], [S12]

## 8. Implementation sequence and exit criteria

1. **Confirm access and deployment.** Obtain the registered client/approved scopes and callbacks,
   or confirm the eligible local path. Record supported models, inputs, consent, renewal, security
   notifications, and hosted credential custody. Exit: a documented supported integration contract;
   otherwise keep user-facing features disabled and complete only preparatory design/tests.
2. **Add identity foundation.** Implement validated configuration, OAuth/JWKS adapter, one-use
   attempts, additive schema migration, external identity resolution, profile completion/linking,
   and ordinary Quarry session issuance. Exit: password and federated sessions have identical
   resource protections and existing IDs/data remain intact.
3. **Add grant storage and lifecycle.** Implement protected token custody, serialized refresh,
   connection status/select/disconnect, and registered security logout. Exit: no credentials cross
   into browser storage/logs; two users cannot inspect or spend each other's grants.
4. **Add inference selection.** Introduce actor-scoped chat routing, model discovery, strict
   delegated request formatting, and funding/error handling. Exit: both chat entry points use the
   chosen grant and cannot silently charge the service key on failure.
5. **Add shared UI and native browser flow.** Wire login, account linking, consent, models, usage,
   cancellation, and recovery on web and desktop. Exit: browser completion returns only to the
   initiating session/client, including after account switches and stale callbacks.
6. **Pilot and roll out.** Use a disposable database and explicit test account consent. Complete a
   small real request, verify completed inference and provider usage attribution, then test expiry,
   denial, disconnect, limits, and unavailable service funding. Expand only after these pass.

Apply Quarry's Axum, React, verification, and review skills. Apply the explicitly requested
`rust-skills` instructions from the installed skill: typed errors and credentials, no credential
`Debug` output, immutable request-scoped
headers, bounded concurrency, cancellation, and offloaded blocking persistence. Follow the
manifest's edition/MSRV; assess maintained OAuth/JWT and authenticated-encryption dependencies
before adding them. No new Node sidecar, Codex process, or Tauri business backend is required.

## 9. Verification, migration, and rollback

Test OAuth callback tampering/replay/expiry, wrong issuer/audience/nonce/key/algorithm, key rotation,
login CSRF and linking proof, email collision, missing profile fields, concurrent first sign-in,
session switching, and last-method unlink protection. Verify password users cannot authenticate
with absent password hashes and retain generic errors/throttling.

Use temporary SQLite databases to test upgrade from the live marker (16 today), foreign keys,
external-identity uniqueness, atomic account/session creation, attempt consumption, owner isolation,
encrypted record recovery, key rotation, and refresh/disconnect races. Migration tests must cover
all supported predecessor paths. Never reset valuable data; no Helix data-shape change is planned.

Provider tests use fake OAuth/JWKS/model/Responses endpoints: distinct tokens for two users,
expired and rotating grants, missing plan permission, no-key delegated chat, no silent fallback,
current catalog shape, pre-stream errors, mid-stream limits, incomplete/failed/EOF behavior,
partial-answer persistence, and cancellation. Test redaction throughout HTTP/SSE/activity logging.
Use maintained cryptographic libraries rather than testing a homegrown JWT implementation.

Run from `frontend/`: focused auth/API/chat tests, `npm run typecheck`, `npm run check:boundaries`,
`npm test`, `npm run build:web`, `npm run check:web-bundle`, and `npm run build:desktop-ui`.
Inspect both sign-in flows and UI failure states with keyboard/focus, light/dark themes, and
relevant viewports. Stop any task-started listeners after inspection.

Run from `backend/`: focused identity/migration/provider/architecture tests, then
`cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
`cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets`.
For changed native code, run Tauri fmt/Clippy/tests with the repository's locked all-targets gates.
Do not use ordinary backend startup or destructive reset tools as verification.

Roll out behind separate sign-in and delegated-chat flags. Preserve password and server-key
paths. Drain old writers, back up the database and required encryption keys, and deploy matching
backend/web/desktop contracts. Disabling new flags stops new OAuth/delegated admission; define
whether existing streams drain or cancel. It does not erase external identities or grants.
An additive schema change may make the previous binary refuse startup: rollback to that binary
requires the matching drained pre-upgrade backup and keys. Avoid restoring backups containing
already-rotated refresh tokens as though they were valid; require reconnection where necessary.

## 10. Architecture impact and current-document boundary

This plan changes no runtime. The canonical architecture was re-read; it should continue to
describe password sessions and server-key AI until implementation lands. The existing uncommitted
HTTP-module documentation belongs to the previous refactor and must be preserved.

Implementation must update `docs/ARCHITECTURE.md` and `docs/authentication.md` in the same change
for external identities, credential custody, provider permissions, actor-scoped inference,
new configuration/routes/DTOs/native capability, schema upgrade and rollback, and feature maturity.
Add a focused operational recovery document if needed. Do not turn this plan into a claim that
Quarry already supports Sign in with ChatGPT or that OpenAI has approved its deployment.

## 11. Research sources

Reviewed 2026-11-04. Recheck availability and preview limitations before implementation. The Help
Center explains user behavior; developer guides govern technical integration. Where commercial
and open-source contracts differ, use the contract provisioned for Quarry.

- [S1: Quickstart][S1] — integration options, separate identity and plan permission.
- [S2: On your website][S2] — partner registration, OIDC/PKCE, ID validation and local sessions.
- [S3: Registration and sign-in][S3] — eligible local dynamic registration and loopback flow.
- [S4: Models and inference][S4] — access-token authorization, model catalog, Responses completion.
- [S5: Preview limitations][S5] — request fields, supported inputs and restricted capabilities.
- [S6: Accounts and sessions][S6] — registration isolation, renewal, revocation and credential custody.
- [S7: Token reference][S7] — token response metadata, expiry and rotating refresh tokens.
- [S8: Errors and recovery][S8] — grant denial, usage limits and stream/provider failures.
- [S9: Self-hosted VMs][S9] — remote loopback distinction and credential-transfer limitations.
- [S10: Developer interest form][S10] — request commercial identity and plan-use access separately.
- [S11: Using your ChatGPT plan in other apps][S11] — user consent, eligibility and usage controls.
- [S12: Account security notification][S12] — signed OIDC back-channel logout.
- [User-provided Sign in with ChatGPT article][S13] — overview and partner availability.

[S1]: https://developers.openai.com/siwc/quickstart
[S2]: https://developers.openai.com/siwc/website
[S3]: https://developers.openai.com/siwc/token-sharing-open-source/sign-in
[S4]: https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference
[S5]: https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations
[S6]: https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions
[S7]: https://developers.openai.com/siwc/token-sharing-open-source/token-reference
[S8]: https://developers.openai.com/siwc/token-sharing-open-source/errors-and-recovery
[S9]: https://developers.openai.com/siwc/token-sharing-open-source/self-hosted-vms
[S10]: https://openai.com/form/sign-in-with-chatgpt-interest/
[S11]: https://help.openai.com/en/articles/20001542-using-your-chatgpt-plan-in-other-apps-and-sites
[S12]: https://developers.openai.com/siwc/account-security-notification
[S13]: https://help.openai.com/en/articles/20001410-sign-in-with-chatgpt
