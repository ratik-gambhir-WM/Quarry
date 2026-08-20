# Quarry shared desktop and web deployment plan

> Implementation note (2026-08-18): the user selected `Quarry-multiplatform` as the canonical
> consolidation destination. References below that propose making `Quarry-web` canonical should
> be read as historical planning context; neither source repository was modified.

Status: proposed  
Prepared: 2026-08-10  
Repositories reviewed:

- `/Users/rgamhir/Quarry` at `e36c569580b3f30dbe7785e7640a5047b2e752cd`
- `/Users/rgamhir/Quarry-web` at `8d9c249f15d7b16d70a90cb08d2eb6d325efa836`

Scope: converge Quarry's Tauri desktop app and Quarry-web's browser app into one maintained React application, one shared cloud product backend, and a thin desktop-native boundary; then produce repeatable web and signed desktop releases.

## Executive decision

The Curio plan's **one React/Vite frontend built for web and Tauri** strategy is a good fit for Quarry. Its backend strategy is not sufficient for Quarry.

Quarry should use this target shape:

1. `Quarry` becomes the canonical repository.
2. `Quarry/ui` becomes the only maintained React source tree and builds in `web` and `desktop` modes.
3. `Quarry/server` becomes the authenticated, hosted product API used by both clients.
4. A thin Tauri shell moves beside the canonical frontend and handles only OS capabilities: native file/folder selection, granted local-file reads and uploads, save/export dialogs, external links, secure token persistence where required, window/menu behavior, and signed updates.
5. The current in-process desktop product backend—local users/deals, OpenAI calls, Helix access, ingestion, summaries, and duplicated domain services—does not move wholesale. Those product operations converge on the hosted Axum API.
6. After production parity and data migration are complete, the separate `Quarry` repository becomes read-only/archived. It should not remain a second place where product UI and business logic are developed.

The desired runtime is:

```text
 Browser build                                      Tauri desktop build
 ┌─────────────────────────┐                        ┌─────────────────────────┐
 │ Shared React application│                        │ Shared React application│
 │ routes, UI, state, auth │                        │ routes, UI, state, auth │
 └────────────┬────────────┘                        └────────────┬────────────┘
              │ HTTPS                                            │ HTTPS
              │                                                  ├──────────────┐
              │                                                  │ native bridge│
              │                                                  │ files/export │
              │                                                  │ links/updater │
              │                                                  └──────────────┘
              └──────────────────────┬───────────────────────────┘
                                     │
                             Hosted Axum API
                     auth, deals, documents, summaries
                                     │
                  ┌──────────────────┼──────────────────┐
                  │                  │                  │
           relational DB       object storage      job workers
                  │                                     │
                  └────────────── Helix/search ──────────┘
```

This is a cloud-first design. It intentionally does not promise offline operation. If offline/local-only desktop behavior is a hard requirement, use the alternative in [Offline-first alternative](#offline-first-alternative) instead; that is a materially larger synchronization product.

## Why the Curio strategy needs modification for Quarry

### The frontend convergence is already close

The two current frontend trees contain:

| Measure | Current result |
|---|---:|
| Desktop React files | 94 |
| Web React files | 83 |
| Common relative paths | 82 |
| Byte-identical common files | 59 |
| Diverged common files | 23 |
| Desktop-only files | 12 |
| Web-only files | 1 (`lib/backendApi.ts`) |

The desktop's latest `feature parity` commit has already introduced useful pieces that should seed the shared app:

- `Quarry/src/lib/product/` contains a typed product interface and Tauri adapter.
- Tauri imports are mostly centralized in `src/lib/tauri/`, the upload modal, and the development playground.
- Shared routes, code splitting, activity logs, document-job UX, and current visual parity are present.
- Tauri CSP, window limits, and plugin capabilities are now restrictive and covered by tests. The older parity plan's `csp: null` finding is no longer current.

The canonical *location* should still be `Quarry-web/frontend`, but the desktop versions should be the default merge winner for the 23 diverged files where they contain the newer parity work. Browser upload and HTTP behavior remain the web-specific winner.

### The Rust backends are not one implementation

The current Rust trees contain:

| Measure | Current result |
|---|---:|
| Desktop Rust source files | 56 |
| Web backend Rust source files | 78 |
| Common relative paths | 44 |
| Byte-identical common files | 8 |
| Diverged common files | 36 |
| Desktop-only files | 12 |
| Web-only files | 34 |

Although both backends contain similarly named parsers, models, Helix queries, repositories, services, and jobs, most have already drifted. The desktop has Tauri commands, app-data SQLite, native path grants, native previews, structured IPC errors, and a retained job manager. The web backend has Axum routes/handlers, CORS/timeouts/request IDs, browser multipart uploads, WM service clients, and SSE.

Putting both complete Rust implementations into one repository would improve file ownership but would not solve the product problem: desktop and web users would still have different users, deals, documents, jobs, and persistence.

### Quarry-web is a development server, not a production web architecture yet

The hosted path has several release blockers:

- There is no authentication or authorization middleware. Supplying an email is treated as a session, and all API routes are public once reachable.
- The login flow asks users for an OpenAI API key. SQLite stores it as plaintext, and the web user response includes it.
- `backend/.env.example` currently contains a credential-shaped nonempty `OPENAI_API_KEY`. Treat it as exposed: rotate/revoke it, replace it with an empty placeholder, and purge it from Git history before additional distribution.
- The Axum backend uses process-local SQLite and in-memory document-job channels. Data and job state are tied to one process and jobs are lost on restart.
- Browser-created deals store a `browser-upload://...` marker, while uploaded bytes are request-scoped rather than durable object storage.
- Web data-room browsing resolves environment variables or three hard-coded absolute paths under `/Users/rgambhir`. It does not load uploaded data-room content from durable storage.
- The server requires a locally reachable Helix instance at startup and exits if document indexes cannot be initialized.
- There is no container, deployment manifest, production migration workflow, web release workflow, or desktop release workflow.

These are more important than the remaining React drift. Shipping the current browser build publicly would expose sensitive operations without a production identity or tenancy boundary.

## Current baseline

The current code is a sound migration baseline:

| Check | Result on 2026-08-10 |
|---|---|
| `Quarry` frontend production build | Passed |
| `Quarry` Vitest | 11 passed |
| `Quarry/src-tauri` Cargo tests | 121 passed, 4 ignored |
| `Quarry-web/frontend` production build | Passed |
| `Quarry-web/backend` Cargo tests | 79 passed |
| Git working trees before planning | Clean on `main` |

Both frontend builds warn about the approximately 1.4 MB minified PDF preview chunk. This is not a consolidation blocker, but it should receive an explicit performance budget before release.

Neither frontend currently has linting. Quarry-web's frontend also has no test script. The shared app should inherit the desktop tests and add browser/runtime contract coverage.

## Canonical repository layout

Keep the first consolidation mechanically small:

```text
Quarry-web/
├── frontend/                         # the one React/Vite application
│   ├── package.json
│   ├── vite.config.ts
│   ├── src/
│   │   ├── app/                      # bootstrap, providers, route manifest
│   │   ├── features/                 # deals, data room, summarize, account, logs
│   │   ├── components/               # shared presentation
│   │   ├── contracts/                # transport-neutral frontend/domain DTOs
│   │   ├── api/                      # versioned HTTP API client
│   │   └── platform/
│   │       ├── contracts.ts          # browser/native capability interfaces
│   │       ├── runtime.web.ts        # browser composition
│   │       └── runtime.desktop.ts    # Tauri composition; only target with Tauri imports
│   └── src-tauri/                    # thin Tauri 2 shell
│       ├── capabilities/
│       ├── src/
│       └── tauri.conf.json
├── backend/                          # hosted Axum API
│   ├── migrations/
│   └── src/
├── plans/
├── Cargo.toml                        # optional workspace for backend + src-tauri
└── README.md
```

Do not rename `frontend` and `backend` into a new `apps/` hierarchy during the functional migration. That can be a later cleanup. Moving the thin Tauri shell under `frontend/src-tauri` follows Tauri's standard Vite integration and keeps one package responsible for both UI artifacts.

## Frontend architecture

### One product API, composed with target capabilities

Expand the idea in `Quarry/src/lib/product/` into two distinct concerns:

1. `QuarryApi`: authenticated product operations over HTTPS. Browser and desktop use the same implementation.
2. `PlatformCapabilities`: operations that genuinely differ by runtime.

```ts
export interface QuarryRuntime {
  target: "web" | "desktop";
  api: QuarryApi;
  files: FileSourceCapability;
  exports: ExportCapability;
  externalLinks: ExternalLinkCapability;
}

export interface QuarryApi {
  getCurrentUser(): Promise<AccountUser>;
  listDeals(): Promise<Deal[]>;
  getDeal(dealId: string): Promise<Deal>;
  createDeal(input: CreateDealInput): Promise<Deal>;
  archiveDeal(dealId: string): Promise<void>;
  createUploadSession(input: CreateUploadSessionInput): Promise<UploadSession>;
  listDealDocuments(dealId: string): Promise<DocumentTree>;
  getDocumentPreview(documentId: string): Promise<DocumentPreview>;
  getDocumentJob(jobId: string): Promise<DocumentJob>;
  subscribeToDocumentJob(jobId: string, listener: (job: DocumentJob) => void): () => void;
  summarizeDocuments(documentIds: string[]): Promise<SummaryJob>;
}
```

Shared pages and hooks import only the contracts/runtime. They do not import `backendApi.ts`, `@tauri-apps/*`, `File`, raw filesystem paths, or Axum-shaped response details.

### File selection without leaking platform types

The upload UI should deal in a neutral selected-file descriptor:

```ts
export type SelectedUpload = {
  id: string;
  name: string;
  relativePath: string;
  sizeBytes: number;
  mimeType: string;
};
```

- The browser adapter keeps the corresponding `File` objects in a private in-memory map.
- The desktop adapter asks Rust to create short-lived opaque grants for selected local files. Shared React sees the grant ID and metadata, not an unrestricted reusable path.
- The backend creates an upload session and storage keys.
- Browser uploads use the selected `File` objects.
- Desktop Rust streams granted files to short-lived upload URLs or the authenticated API and emits progress through a Tauri channel/event. It must not accept arbitrary paths supplied by React.
- Both adapters finish by returning the same server job/document DTOs.

This preserves the desktop's native picker and avoids sending large base64 payloads through IPC while still giving both clients one durable data room.

### Routing

Use one route manifest and vary only the router:

- Web: `BrowserRouter`, with the host configured to rewrite unknown application routes to `index.html`.
- Desktop: `HashRouter`, which is appropriate for bundled static assets.
- Keep `TauriPlaygroundPage` desktop-development-only and out of the production route/navigation manifest.
- Route availability differences should be explicit metadata, not two copies of `App.tsx`.

The web currently uses `HashRouter`; changing to browser history is optional for the first cutover but recommended before public release so web URLs are conventional and refresh behavior is tested deliberately.

### Build-time runtime selection

Use Vite modes or aliases rather than runtime checks scattered across components:

```json
{
  "scripts": {
    "dev:web": "vite --mode web",
    "build:web": "tsc && vite build --mode web",
    "dev:desktop-ui": "vite --mode desktop",
    "build:desktop-ui": "tsc && vite build --mode desktop",
    "dev:desktop": "tauri dev",
    "build:desktop": "tauri build",
    "test": "vitest run",
    "lint": "eslint ."
  }
}
```

Alias a stable module such as `@quarry/runtime` to `runtime.web.ts` or `runtime.desktop.ts`. Add an ESLint `no-restricted-imports` rule so `@tauri-apps/*` is allowed only under `src/platform/runtime.desktop.ts` and explicitly approved desktop helpers.

`VITE_API_BASE_URL` is public configuration, not a secret. Production builds should fail if it is absent or not HTTPS. Do not broaden Vite's `envPrefix` to include `TAURI_`.

### API contracts

Introduce `/api/v1` before a desktop release depends on the hosted API. Desktop releases cannot be upgraded atomically with the server.

Recommended contract rules:

- Publish an OpenAPI document from the Axum route/DTO definitions and generate the TypeScript client/types in CI.
- Use one stable error envelope with `code`, safe `message`, `operationId`, and `retryable`.
- Include request IDs in all responses and logs.
- Support at least the current and previous desktop API contract during rollout.
- Add a small `/api/v1/capabilities` response so an older desktop can disable unsupported features rather than fail unpredictably.
- Use idempotency keys for deal creation, upload completion, job creation, and other retryable mutations.

## Hosted backend architecture

### Identity and authorization are Phase 1, not release polish

Replace email-in-session and user-supplied OpenAI keys with enterprise identity before exposing the API.

Recommended default for a West Monroe application:

- Microsoft Entra ID/OIDC.
- Authorization code flow with PKCE for both the SPA and desktop public client.
- System-browser authentication for desktop; never embed a desktop client secret.
- Axum middleware validates issuer, audience, signature, expiry, tenant, and required scopes/roles.
- Every repository operation derives tenant/user scope from validated claims, never from a caller-supplied `userId`.
- The user table stores the external subject/tenant/profile and application roles. Remove `api_key` from user input, API responses, SQLite/Postgres, and Helix user projections unless there is a separately approved encrypted credential use case.
- OpenAI, Helix, WM-service, database, object-storage, signing, and updater secrets remain server/CI secrets managed outside source control.

### Durable relational data

Use a managed relational database for production, preferably Postgres with SQLx migrations. Keep SQLite only for isolated local development/tests or a explicitly single-instance pilot.

Postgres should be the source of truth for:

- users/tenant membership and application roles;
- deals and deal metadata;
- document metadata, storage keys, content hashes, versions, and deal relationships;
- upload sessions and idempotency keys;
- document/summary job status, attempts, timestamps, and safe errors;
- audit metadata.

Do not store local absolute filesystem paths in product records. Provide a one-time migration/import utility for useful developer data rather than trying to sync live SQLite databases.

### Durable document storage

Store document bytes and generated previews in object storage. A document record should reference a tenant-scoped storage key, never `browser-upload://...` or `/Users/...`.

Recommended upload flow:

1. Client requests an upload session containing filenames, relative paths, sizes, content hashes when available, and deal ID.
2. API validates tenancy, file limits, supported types, and quotas, then returns short-lived upload destinations.
3. Browser or Tauri uploads directly/streamingly.
4. Client finalizes the session idempotently.
5. API creates durable job rows.
6. Worker parses, converts, chunks, embeds, writes Helix projections, and records terminal state.
7. UI receives status over SSE or polls the same durable job resource.

Preserve the current 50 MB per-file/request constraints until product requirements justify changing them. Enforce limits at the reverse proxy, API, upload session, and worker.

### Durable jobs and workers

Replace process-only `watch` maps as the source of truth. In-memory broadcasts may still accelerate live updates, but Postgres job rows must survive process restarts.

For the first production deployment:

- Use a database-backed job claim/lease with bounded worker concurrency, retries, heartbeats, and terminal status retention.
- Keep parsing/LibreOffice/OpenAI/Helix work out of Axum request handlers.
- Make ingestion idempotent by tenant/user, content hash, and processing version.
- Record safe error codes and internal diagnostic context separately.
- Support cancellation where the external operation permits it.
- Run workers as a separate process/container once API and document load need independent scaling.

### Helix's role

Keep Helix as the document graph/search projection if it remains the chosen search engine, but do not make it the sole source of truth for users, deals, upload state, or jobs.

- Store canonical metadata in Postgres and bytes in object storage.
- Update Helix through idempotent worker operations/outbox records.
- Make API startup independent of immediate Helix availability; health should distinguish `live` from `ready`.
- Scope every graph node/query by tenant and authorized user/deal.
- Provide replay/rebuild tooling from canonical document metadata.

## Desktop-native boundary

The thin shell should retain or add only capabilities that make sense on a desktop:

| Capability | Desktop implementation | Web implementation |
|---|---|---|
| Sign in | System browser + PKCE/deep-link or loopback completion | SPA PKCE redirect |
| Pick files/folders | Tauri dialog + opaque grants | Browser file/directory inputs |
| Upload local files | Rust streams granted paths to upload session; progress events/channels | `fetch`/XHR from `File` objects |
| Preview uploaded docs | Shared remote preview API | Shared remote preview API |
| Preview before upload | Optional bounded native preview | Browser object URL where supported |
| Save summary/log | Native save dialog and atomic write | Browser download |
| Open external URL | Narrow opener capability | `window.open` with validation |
| Menus/windows | Tauri only | Not applicable |
| Updates | Signed Tauri updater | Normal web deployment |

Do not retain desktop-local copies of user/deal/document truth after cutover. If a small encrypted cache improves startup, treat it as disposable cache with explicit invalidation, not a second database.

Preserve the current restrictive CSP, `freezePrototype`, narrow dialog permissions, safe window defaults, and security tests. Add only the network/updater permissions required by the deployed HTTPS origins. Do not load the production web application as a remote Tauri URL; bundle the built frontend so Tauri APIs remain available only to shipped code.

## Phased implementation

### Phase 0 — Security response and decision checkpoint

1. Rotate/revoke the credential-shaped OpenAI key in `Quarry-web/backend/.env.example` and any environment where it was reused.
2. Replace it with an empty placeholder and purge the value from Git history using the organization's approved secret-removal process.
3. Confirm cloud-first behavior: desktop and web require connectivity and share one account/data set.
4. Choose production providers for relational DB, object storage, API/worker compute, Helix, secrets, web hosting, and desktop artifact hosting.
5. Choose supported desktop targets (Windows, macOS, optional Linux) and distribution channels.
6. Record the current passing commands and representative browser/desktop smoke flows.
7. Create a recoverable checkpoint before moving frontend or Tauri files.

Exit criteria: the leaked credential is invalidated, cloud-first is approved, deployment owners/providers are known, and the current baseline is reproducible.

### Phase 1 — Versioned API, identity, and safe contracts

1. Add `/api/v1`, OpenAPI generation, generated TypeScript types/client, and contract-drift CI.
2. Normalize errors to stable safe codes with request/operation IDs.
3. Integrate Entra ID for the SPA and desktop public client using PKCE.
4. Add Axum auth middleware/extractors and tenant/user authorization to every non-health route.
5. Replace caller-provided user scope with claim-derived scope.
6. Remove OpenAI API key collection/display and migrate user DTOs away from `apiKey`.
7. Split liveness/readiness endpoints and make optional dependencies report degraded readiness instead of crashing unrelated routes.
8. Add authorization tests for cross-user/cross-tenant access, missing/expired tokens, and every file/job/deal route.

Exit criteria: an unauthenticated caller cannot access product data, both client registrations can obtain API tokens, and one generated contract compiles in the frontend.

### Phase 2 — Production persistence and document pipeline

1. Add Postgres/SQLx migrations and repository integration tests.
2. Move users, deals, metadata, upload sessions, documents, jobs, and idempotency records to Postgres.
3. Add object-storage abstractions and production implementation.
4. Replace `browser-upload://` and hard-coded data-room roots with document/storage records.
5. Implement resumable or retryable upload sessions for browser and desktop clients.
6. Move parsing/conversion/embedding/Helix work to durable jobs with bounded worker concurrency.
7. Make job status, retries, terminal state, and safe errors durable.
8. Add Helix projection/replay and ensure API startup tolerates a temporarily unavailable Helix service.
9. Add data import tooling only for current SQLite data that is worth preserving.

Exit criteria: an uploaded directory survives API restart, appears as the same deal data room on another client, and a worker restart does not lose job state.

### Phase 3 — Create the one shared frontend

1. Copy/merge the desktop parity improvements into `Quarry-web/frontend` feature by feature, using the 59 identical files as the stable base.
2. Move shared DTOs out of `backendApi.ts` and `tauriProductApi.ts` into transport-neutral contracts.
3. Create the shared `QuarryApi`, capability interfaces, and runtime provider.
4. Implement the versioned HTTP API client once for both targets.
5. Implement browser file, export, link, and auth capabilities.
6. Implement build-time aliases for web/desktop runtime composition.
7. Share one route manifest; use `BrowserRouter` for web and `HashRouter` for desktop.
8. Merge the desktop Vitest suite into the canonical package and add adapter contract tests.
9. Add ESLint, restricted Tauri imports, and a CI check that fails if another product React tree appears.
10. Add a performance budget and lazy-load PDF preview code only when needed.

Exit criteria: `Quarry-web/frontend/src` is the only maintained React tree; its web build has no Tauri code, and shared features compile against mocked web and desktop runtimes.

### Phase 4 — Build the thin Tauri shell

1. Create `Quarry-web/frontend/src-tauri` from the current shell configuration, icons, menus, window behavior, CSP, and narrow capabilities.
2. Keep only approved commands for file/folder grants, streaming uploads, native save/export, external links, auth callback/token storage if required, diagnostics, and updater integration.
3. Compose `runtime.desktop.ts` from the common HTTPS API client and native capabilities.
4. Use short-lived upload sessions/grants; never accept arbitrary read paths from React.
5. Preserve safe structured errors and path-redacting activity logs.
6. Verify the packaged app uses the production HTTPS API without developer shell environment variables.
7. Remove desktop-local OpenAI/Helix clients, product SQLite, duplicate repositories/services/parsers/jobs, and direct product commands only after remote parity is proven.
8. Add signed updater artifacts and a trusted HTTPS update endpoint.

Exit criteria: a packaged desktop build contains the shared frontend, has no bundled product secrets, and reaches the same hosted users/deals/documents/jobs as the web build.

### Phase 5 — Deployment and release automation

1. Add a production container build for the Axum API and worker, with non-root runtime, health checks, graceful shutdown, and no source `.env` files.
2. Add automated database migrations with backup/rollback policy.
3. Deploy API and worker to the approved container platform, Postgres to managed storage, documents to object storage, and Helix to its supported hosted/runtime environment.
4. Deploy `frontend/dist` to the approved static host/CDN with SPA fallback, HTTPS, CSP/security headers, and production API origin.
5. Configure strict production CORS for the deployed web origin. Desktop is not a reason to allow `*`; native HTTP clients do not rely on browser CORS.
6. Add CI for frontend, backend, integration, and desktop targets.
7. Build desktop installers on native OS runners; code-sign Windows artifacts and sign/notarize macOS artifacts.
8. Publish signed updater metadata/artifacts. Keep updater private key material only in protected CI secrets.
9. Generate SBOMs/checksums and retain release provenance/artifacts.

Exit criteria: staging and production web releases are reproducible, signed desktop installers are downloadable on approved channels, and update checks install only signed artifacts.

### Phase 6 — Cross-client parity, cutover, and repository retirement

1. Run the complete smoke matrix with the same test user and data on web and desktop.
2. Verify a deal/document created on either client is visible on the other after refresh.
3. Verify uploads, progress, retries, summaries, previews, logs, and authorization failures on both targets.
4. Verify API compatibility with the current and previous desktop release.
5. Roll out backend first, then web, then desktop; use feature flags for new contract capabilities.
6. Monitor errors/job latency/upload failures and maintain rollback paths for each tier.
7. Remove the duplicate `Quarry` frontend/backend from active development and archive the repository after a defined rollback window.
8. Update the canonical README, onboarding, architecture decision record, operational runbooks, and ownership.

Exit criteria: one repository produces both user-facing artifacts, both use one product data plane, and no production functionality depends on the retired desktop backend.

## Test and CI matrix

| Check | Shared/web | Desktop | Backend/worker |
|---|---:|---:|---:|
| TypeScript compile | Required | Same source, desktop mode required | N/A |
| ESLint/import boundary | Required | Required | N/A |
| Vitest shared UI/hooks | Required | Same suite | N/A |
| Runtime/adapter contract tests | Browser adapter | Tauri adapter | N/A |
| Production build | `build:web` | `build:desktop-ui` | Release Cargo build |
| Rust fmt/clippy/test | N/A | Thin shell required | Required |
| OpenAPI generation drift | Required | Same generated client | Required |
| Auth/tenancy integration | Required | Required | Required |
| Upload/job restart tests | Required | Required | Required |
| Object storage/DB integration | Via API | Via API | Required |
| E2E smoke | Browser automation | Packaged-app smoke | Test environment |
| Release artifact | Static `dist` | Signed installer/updater | Container image/SBOM |

Required protocol/behavior tests include:

- token expiry/refresh and logout on both targets;
- authorization and tenant isolation for every resource;
- upload interruption, retry, duplicate content, and idempotent finalize;
- worker/API restart during processing;
- SSE reconnect or polling fallback without duplicate terminal events;
- unsupported/empty/oversized files and malicious relative paths;
- Office conversion timeouts and cleanup;
- Helix unavailable/degraded/replay behavior;
- API compatibility for an older desktop client;
- updater signature rejection and rollback instructions;
- web route refresh and packaged desktop navigation;
- activity-log redaction for tokens, keys, document text, and local paths.

## Deployment environments and release order

Use separate development, staging, and production identity registrations, databases, object-storage namespaces, Helix namespaces, API origins, and updater channels.

Release in this order:

1. Backward-compatible API/database/worker changes.
2. Web frontend, because it can be rolled forward or back immediately.
3. Desktop release after the API capability is live.
4. Remove old API behavior only after the supported desktop upgrade window expires.

Database migrations must be backward compatible for at least one release window. Desktop features should be capability-gated when the server is older or degraded.

## Acceptance criteria

The migration is complete when:

- Exactly one React source tree produces both web and desktop UI artifacts.
- Shared components/hooks have no direct Tauri or raw HTTP imports.
- The normal web bundle contains no `@tauri-apps/*` code.
- Tauri contains only the reviewed native capability boundary, not a second product backend.
- Web and desktop authenticate through the approved identity provider and call the same versioned API.
- The API authorizes every deal/document/job by validated tenant/user claims.
- Users no longer enter or receive OpenAI API keys.
- No credential value is committed in example configuration or build output.
- Users, deals, documents, job states, and summaries are durable and visible from both clients.
- No production record depends on a developer absolute filesystem path or `browser-upload://` marker.
- File bytes live in durable object storage; relational metadata/jobs live in the production database.
- Helix is a rebuildable search/graph projection rather than the only source of truth.
- Web routes refresh correctly on the deployed host.
- Packaged desktop navigation, native selection/upload/export, signing, and updates work on every supported OS.
- CSP, Tauri capabilities, CORS, security headers, and secret handling use least privilege.
- CI verifies both artifacts and the shared API contract.
- The old `Quarry` repository is no longer an active implementation target.

## Risks and mitigations

### Cloud-first breaks desired offline behavior

Mitigation: obtain an explicit product decision in Phase 0. Do not quietly preserve local databases as an undocumented offline mode.

### Consolidation becomes a backend rewrite and UI migration at once

Mitigation: land versioned API/auth/persistence first, then merge frontend feature groups. Keep both current clients buildable until the shared runtime handles each migrated operation.

### Desktop releases lag the API

Mitigation: version `/api/v1`, use additive contracts, maintain the previous desktop contract, expose capabilities, and remove fields/routes only after the upgrade window.

### Large local files make desktop uploads unreliable

Mitigation: stream from Rust, use short-lived upload sessions, support retry/resume where storage permits, bound concurrency, and never base64 entire files through IPC.

### Object storage and Helix become inconsistent

Mitigation: Postgres is canonical, worker operations are idempotent, projection state is recorded, and replay/rebuild tooling exists.

### Native capabilities expand after frontend compromise

Mitigation: bundle the frontend, keep strict CSP and capabilities, use opaque file grants, validate every command, restrict window labels/origins, and add permissions only for reachable features.

### A second frontend/backend reappears

Mitigation: archive `Quarry`, document code ownership, and add CI checks for duplicate React roots, direct Tauri imports, and generated contract drift.

### Existing credential has already leaked

Mitigation: assume compromise, rotate/revoke immediately, purge history, audit access/usage, and add secret scanning/pre-commit and CI controls.

## Approaches not recommended

### Keep two repositories and manually synchronize files

This preserves the exact source of today's drift. A shared npm or Git submodule would improve React reuse but still leaves two release graphs and two Rust product backends.

### Bundle the Axum server as a desktop sidecar

This would replace Tauri commands with localhost HTTP without producing shared cloud data. It adds port/process/firewall/lifecycle complexity and still ships product secrets or local persistence.

### Keep the full Tauri backend and extract only shared Rust crates

This can reduce duplicated parser/domain code, but desktop and web data remain split unless a synchronization protocol is added. Use it only for the offline-first alternative.

### Compile the backend to WebAssembly

Filesystem access, SQLite/Helix clients, OpenAI/WM calls, LibreOffice conversion, background jobs, and server authorization are not suitable browser-WASM responsibilities. WASM would not solve shared persistence or deployment.

### Load the hosted web app directly inside Tauri

This makes desktop behavior dependent on live remote content and complicates the Tauri trust boundary. Bundle the shared static frontend and allow native APIs only to bundled code.

## Offline-first alternative

Choose this only if desktop must create/read/summarize complete data rooms without connectivity.

1. Keep a local Tauri engine and local encrypted database/object cache.
2. Extract platform-neutral Rust crates for models, validation, parsing, chunking, content identity, and Helix query construction.
3. Have Axum and Tauri depend on those crates through thin HTTP/IPC adapters.
4. Define a real synchronization protocol with globally unique IDs, per-record versions, tombstones, upload queues, conflict resolution, encryption, tenancy, and resumable object transfer.
5. Treat Helix as a server projection; queue local embeddings/graph writes for replay or decide which work can safely remain local.
6. Add migration and conflict tests across offline edits, reconnects, deletes, and desktop version skew.

This alternative is substantially more expensive and risky than the recommended cloud-first plan. It should be justified by a tested offline requirement, not by the fact that the current desktop happens to use local Rust and SQLite.

## Suggested commit sequence

1. `security: revoke committed key and sanitize repository history`
2. `api: add v1 contracts errors and request identity`
3. `auth: protect api with entra claims and tenant scope`
4. `storage: add postgres migrations and object upload sessions`
5. `jobs: persist document work and split worker runtime`
6. `frontend: add shared contracts and runtime composition`
7. `frontend: merge quarry desktop parity into canonical app`
8. `web: implement browser files auth routing and deployment build`
9. `desktop: add thin tauri files export auth and updater bridge`
10. `desktop: remove local product backend after remote parity`
11. `ci: verify contracts web api worker and signed desktop builds`
12. `release: deploy staging and run cross-client parity matrix`
13. `docs: make quarry-web canonical and archive quarry`

Each commit should leave the currently migrated target buildable. API changes should be additive until the desktop upgrade window has passed.

## Decisions required before implementation

| Decision | Recommendation | Why it matters |
|---|---|---|
| Connectivity model | Cloud-first | Determines whether local DB/sync is required |
| Canonical repository | `Quarry-web` | Prevents another split ownership model |
| Identity | Entra ID + PKCE | Required before public/internal multi-user deployment |
| Relational database | Managed Postgres | Durable multi-instance server data |
| File storage | Managed object storage | Replaces request-scoped uploads/local paths |
| Job execution | Durable DB-backed queue, separate worker when needed | Survives restart and scales independently |
| Helix | Hosted/shared projection with replay | Desktop-local Helix cannot provide shared data |
| Desktop OS targets | Decide Windows/macOS/Linux explicitly | Controls signing, runners, installers, and QA |
| Distribution | Organization-approved installer channel + signed updater | Controls trust and updates |
| Existing local data | Import once or discard as development data | Avoids accidentally designing live sync |

## Reference basis

Local evidence:

- `Quarry/src/lib/product/` — current typed Tauri product adapter seed.
- `Quarry/src-tauri/tauri.conf.json` and `capabilities/default.json` — current hardened desktop baseline.
- `Quarry-web/frontend/src/lib/backendApi.ts` — current browser HTTP/SSE adapter and transport-coupled DTOs.
- `Quarry-web/backend/src/routes/mod.rs` — CORS, request IDs, timeout, and route composition; no auth layer.
- `Quarry-web/backend/src/state.rs` — process-local SQLite and in-memory job state.
- `Quarry-web/backend/src/services/deal_service.rs` — `browser-upload://` deal marker.
- `Quarry-web/backend/src/services/data_room_service.rs` — environment and hard-coded local data-room roots.
- `Quarry-web/backend/src/services/user_service.rs` — plaintext user API-key persistence and response field.

Current first-party guidance:

- Tauri treats the bundled frontend as a static host and recommends SPA/SSG/MPA with a normal client-server API boundary: [Tauri frontend configuration](https://v2.tauri.app/start/frontend/).
- Tauri documents the Vite `beforeDevCommand`, `beforeBuildCommand`, `devUrl`, and `frontendDist` integration used by this layout: [Tauri Vite integration](https://v2.tauri.app/start/frontend/vite/).
- Tauri capabilities constrain which permissions are available to which windows/webviews, while custom Rust code still requires correct validation: [Tauri capabilities](https://v2.tauri.app/security/capabilities/).
- Tauri distribution requires platform-appropriate signing, and macOS direct distribution requires notarization: [Tauri distribution](https://v2.tauri.app/distribute/).
- Tauri updater artifacts are signed and production endpoints use HTTPS: [Tauri updater](https://v2.tauri.app/plugin/updater/).
- Tauri provides a GitHub Actions release pattern for native OS builds and release artifacts: [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/).
- Vite emits deployable static output in `dist`: [Vite static deployment](https://vite.dev/guide/static-deploy.html).
- Microsoft documents desktop applications calling protected web APIs and recommends authorization code with PKCE for desktop/public clients: [Microsoft identity platform desktop applications](https://learn.microsoft.com/en-us/entra/identity-platform/index-desktop) and [authentication flow scenarios](https://learn.microsoft.com/en-us/entra/identity-platform/authentication-flows-app-scenarios).
