# Microsoft authentication API implementation plan

Status: proposed

## Goal and source-compatible behavior

Implement the Microsoft OAuth flow from the linked `microsoftAuth.ts` in Quarry's Axum API, using Quarry's domain architecture. The Rust slice must preserve its observable behavior:

- authorization-code OAuth 2.0 with PKCE;
- scopes `openid profile email offline_access User.Read Files.Read Sites.Read.All`;
- a one-time state plus PKCE verifier that expires after 10 minutes;
- token exchange at the selected Entra tenant endpoint, then Graph `/me?$select=id,displayName,mail,userPrincipalName`;
- an opaque HttpOnly, `SameSite=Lax` session cookie with an eight-hour `Max-Age`;
- in-memory sessions storing access token, expiry, optional refresh token, and a `{ id, name, email }` principal;
- refresh when fewer than 60 seconds remain, retaining an old refresh token when the provider does not return a replacement;
- `GET /login`, `GET /callback`, `GET /me`, and `POST /logout` semantics, including redirects and `{ authenticated: false }` for an unauthenticated `/me` request; and
- a server-only equivalent of `getMicrosoftAccessToken(request)` for later Graph use cases. It must never expose OAuth tokens through `/me` or another frontend endpoint.

This creates sign-in/session capability only. It does **not** turn current caller-supplied `email`, `userId`, `workspaceId`, deal ID, or filesystem input into authenticated authorization. Existing user-profile endpoints and data ownership remain unchanged.

## Verified baseline and design decisions

| Area | Current Quarry state | Decision |
| --- | --- | --- |
| Identity owner | `backend/src/domains/identity/mod.rs` is explicitly a comment-only placeholder. | Make `identity` the owning vertical domain. Do not place auth in `users` or recreate a global `AppState`. |
| API mounting | Feature routers bind their own state in `bootstrap.rs`; `app/http` mounts API at `/api/v1` and compatibility `/api`. | Add relative `/auth/*` routes: public clients use `/api/v1/auth/*` and the automatic `/api/auth/*` compatibility mount remains available. |
| Config | `AppConfig::from_values` is the only environment parser and secrets use `SecretString`. | Add typed Microsoft config there. No identity-domain code reads the environment. |
| Provider transport | Bootstrap owns the shared `reqwest::Client` and all concrete construction. | Add an `adapters/microsoft` client built from that client/config and inject it through bootstrap. |
| Session persistence | Jobs/caches are process-local; SQLite owns local profiles, deals, and documents. | Match the source: pending authorizations and sessions stay in a Tokio-safe in-memory store and disappear on restart. Do not persist tokens or sessions in SQLite. |
| Existing web login | `LoginPage` accepts an email, queries/creates a development profile, and writes email to `sessionStorage`. | Replace that web interaction with Microsoft sign-in and a `/me` bootstrap. Do not infer role/profile creation from Graph claims. |
| Web transport | Vite runs on port 1420 and proxies `/api` same-origin. | Navigate to relative `/api/v1/auth/login`, then use cookie-aware `/me`/logout calls. Explicit cross-origin `VITE_API_BASE_URL` deployments need credentialed fetch/CORS. |
| Desktop transport | Tauri relays requests through a separate `reqwest` client; it does not receive browser/webview cookies. | Do not claim the browser-cookie implementation authenticates desktop. Keep desktop login on its current development flow or make it visibly unavailable until a native OAuth handoff is separately implemented. |

## Public API contract

| Method | Path | Success | Source-compatible failure behavior |
| --- | --- | --- | --- |
| GET | `/api/v1/auth/login` | `302` to `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize` with client ID, configured redirect URI, scopes, random `state`, `S256` challenge, `response_type=code`, and `response_mode=query`. | Missing client credentials produce the source-compatible configured-error JSON response, not a redirect. |
| GET | `/api/v1/auth/callback` | Validate/consume state, exchange code, read Graph user, set cookie, `302` to configured frontend origin. | Provider error, invalid/missing/expired state or code, token error, or Graph error redirects to `FRONTEND_ORIGIN/login?authError=<sanitized message>`. Never forward provider response bodies, codes, or tokens. |
| GET | `/api/v1/auth/me` | `200 { authenticated: true, user: { id, name, email } }`. As in source, a session with an expired token but refresh token is still considered present here. | Missing session, or expired session without refresh capability, removes stale state and returns `401 { authenticated: false }`. |
| POST | `/api/v1/auth/logout` | Remove session, expire the matching cookie, `204`. | Idempotent: absent/malformed cookie still returns `204`. |

The service facade supplies the source's exported access-token helper. It reads the session cookie, returns a valid token or refreshes it, and emits a typed unauthenticated/unavailable failure to a consuming handler. It is not an HTTP API.

## Configuration contract

Add an optional `MicrosoftAuthConfig` capability state in `backend/src/app/config.rs`. Preserve the source names:

| Variable | Default / validation | Handling |
| --- | --- | --- |
| `MICROSOFT_CLIENT_ID` | Required when auth is invoked. | Identifier only; do not expose it through browser configuration. |
| `MICROSOFT_CLIENT_SECRET` | Required when auth is invoked. | `SecretString`; never log, return, or bundle it. |
| `MICROSOFT_REDIRECT_URI` | Default `http://127.0.0.1:1420/api/v1/auth/callback`; absolute HTTP(S), no credentials/query/fragment. | Must exactly match the Entra app registration. The default deliberately returns through Vite's normal proxy so the browser owns one first-party cookie. |
| `MICROSOFT_TENANT_ID` | `organizations`; reject empty/control/path-separator/URL-delimiter values. | Used only as a validated authority path component. |
| `MICROSOFT_FRONTEND_ORIGIN` | `http://127.0.0.1:1420`; absolute origin only, no path/credentials/query/fragment. | The sole post-callback/error redirect target; no request-provided return URL. |
| `MICROSOFT_COOKIE_SECURE` | `true` enables the `Secure` attribute; any other/missing value preserves local source behavior. | Must be true in an HTTPS deployment. |

Keep missing client-credential failure request-time compatible with the source, while failing startup for malformed supplied URLs/tenant input. This permits local non-auth work without Microsoft configuration but prevents unsafe configuration from starting.

For an explicit cross-origin `VITE_API_BASE_URL`, centralize `credentials: "include"` in the web request helper and enable `CorsLayer::allow_credentials(true)` while retaining the current explicit-origin allowlist. Do not use wildcard origins with credentials. `SameSite=None` is not part of the copied source behavior and requires a separate security review.

## Module and implementation sequence

Create this small vertical slice:

```text
backend/src/
├── adapters/microsoft/
│   ├── mod.rs
│   └── client.rs                    # token exchange and Graph /me mechanism
└── domains/identity/
    ├── mod.rs                       # public principal/access-token facade
    ├── model.rs                     # DTOs and private session/state types
    ├── route.rs                     # binds IdentityHttpState
    ├── handler.rs                   # extract/validate/respond/redirect
    └── service.rs                   # PKCE, state, session, refresh workflow
```

1. Add direct dependencies only where required: reuse `sha2` and `base64` for PKCE; add a direct cryptographic-entropy crate compatible with the locked toolchain for the state, verifier, and session bytes; add an Axum-0.8-compatible `axum-extra` cookie feature for standards-based cookie parsing/building. Update `Cargo.lock` only through Cargo.
2. Implement `adapters::microsoft::client::MicrosoftOAuthClient` from the shared `reqwest::Client`. It sends form-encoded token requests containing source-equivalent client/grant/scope fields, validates a successful nonempty `access_token`, and validates Graph `id` plus `mail` or `userPrincipalName`. `displayName` falls back to email then `Microsoft user`. Provider bodies, tokens, and secrets become sanitized adapter errors only.
3. Implement `MicrosoftAuthService` with `tokio::sync::Mutex<HashMap<...>>` maps for pending authorization and session entries. Never hold a mutex guard over outbound `.await`: remove/clone the required value, call Microsoft, then reacquire only to update/delete it. Session IDs are random opaque values, never principals/tokens.
4. `begin_login` generates 32 random bytes as hex `state` and 48 random bytes base64url `code_verifier`, computes the unpadded-base64url SHA-256 `code_challenge`, stores the verifier/expiry, and produces a `url::Url` authorization URL with query-pair APIs rather than string concatenation.
5. `complete_login` accepts only one string `error`, `state`, and `code`. It deletes a matched state on any invalid/expired use, consumes state before token exchange, and uses the generic source error messages for login redirects. On success it creates the source-compatible session, adds `HttpOnly; Path=/; SameSite=Lax; Max-Age=28800` plus configured `Secure`, then redirects to the sole configured frontend origin. Log a generic request-correlated provider failure only.
6. Model `/me` exactly: it only checks session presence and whether an expired session has refresh capability; it does not contact Microsoft. The public access-token facade refreshes at the 60-second threshold, preserves a returned-or-old refresh token, and deletes sessions when refresh is impossible/fails. Logout removes the map item and writes an otherwise matching `Max-Age=0` cookie.
7. Keep `handler.rs` thin: use `Query`, `CookieJar`, state, redirects, and `AppError` mapping; it must not read env or construct adapters. Register `identity::route::routes(identity)` in `backend/src/app/bootstrap.rs`, add `pub mod identity` to `domains/mod.rs`, and extend constructor/dependency architecture tests if needed. No SQLite migration is required.

## Web integration sequence

1. Add a `MicrosoftAuthenticatedUser` type plus start-login, current-session, and logout operations to `frontend/src/contracts/quarryApi.ts`. Start login is browser navigation to `/api/v1/auth/login`, not JSON fetch, and accepts no return URL.
2. Update `frontend/src/api/httpQuarryApi.ts` to make `/me` and `/logout` cookie-aware through the centralized request helper. Do not add tokens, cookies, callback parameters, or Graph responses to the activity log.
3. Replace the email/profile-creation path in `frontend/src/pages/LoginPage.tsx` and `frontend/src/components/login/LoginCard.tsx` with an accessible `Continue with Microsoft` control, initial `/me` check, redirect-in-progress state, and visible `authError` message consumed from the URL. Store at most non-secret display identity in a client cache; never persist tokens in session/local storage.
4. Keep `users`, `WorkspaceAccountUser`, roles, and development API keys out of the new authentication path. Provisioning/updating a local profile and using the verified principal to enforce every resource route are separate, explicitly scoped follow-ups.

## Desktop decision gate

The browser cookie flow cannot authenticate current desktop API requests: the Tauri relay's `reqwest` client does not share the browser/webview cookie jar, and generic relay commands must not forward arbitrary cookies, headers, or OAuth tokens. Before enabling Microsoft login on `runtime.desktop`, choose and document one bounded design:

- system-browser authorization with a Tauri-owned loopback/deep-link callback and native session; or
- webview-only authorization with a narrow platform-owned session bridge that forwards authenticated requests without exposing tokens to IPC.

That later work must validate window/origin/callback input, restrict URLs and capabilities, keep tokens out of TypeScript, and add Tauri Rust and UI tests. Until then, desktop must visibly state that Microsoft sign-in is unavailable or retain the current development-only profile login; it must not show a misleading sign-in success path.

## Tests and verification

### Backend

1. Add config tests for absent capability, blank values, safe defaults, complete config, unsafe URL/tenant rejection, and `SecretString` redaction.
2. Add identity service tests for PKCE encoding, state single-use/expiry, provider-denied callback, token request inputs, Graph display/email fallbacks, session creation, `/me` semantics, refresh threshold/success/failure, old refresh-token retention, logout idempotence, and state loss after process restart.
3. Add Microsoft adapter tests against a local Axum mock server. Assert form/header behavior; reject malformed/error/missing-token and invalid-Graph responses; prove returned errors contain neither secrets nor provider payload/token text. Never use real Microsoft/Graph calls.
4. Extend `backend/tests/integration/http_tests.rs` for both `/api/v1` and `/api` paths: redirects, state consumption, `Set-Cookie` shape, `/me` status/body, callback login-error redirect, logout `204`, and no secret/token in headers or bodies.
5. Mirror each unit test in `backend/tests/unit/...` and include it through its production module because `autotests = false`. Preserve architectural dependency tests.

### Frontend and desktop

1. Add focused web contract/adapter/LoginPage tests for navigation, no session, authenticated session, logout, callback error, loading/error/focus behavior, and explicit cross-origin credentialed-fetch configuration.
2. If desktop remains unsupported, test the explicit unavailable state. Do not write a false-positive desktop cookie test; a later native design needs its own Tauri callback/origin/token-nonexposure coverage.

When implementation begins, run focused tests followed by:

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
```

Do not use `cargo run` as a smoke test: it can migrate the configured SQLite database and requires Helix. A manual staging test, if authorized, uses a dedicated Entra application/test account, HTTPS callback, and `Secure` cookies; it verifies sign-in, provider denial, refresh, logout, browser restart, and the expected invalidation after backend restart.

## Documentation and acceptance criteria

The implementation must update `docs/ARCHITECTURE.md` with the implemented identity owner, `/api/v1/auth/*` contract, OAuth/browser callback flow, in-memory session lifetime, config/Entra registration/CORS/TLS requirements, token redaction, lack of resource authorization, and the unresolved desktop handoff.

The slice is complete when a configured web deployment can sign in, return only its stable principal from `/me`, refresh tokens internally for server-side Graph use, and log out without writing tokens to browser storage or SQLite. It must preserve the linked flow's route/session semantics, make no production authorization claim, pass the scoped/full gates, and contain no `.env`, token, local-database, generated-output, or accidental lockfile changes beyond deliberately added Rust dependencies.

