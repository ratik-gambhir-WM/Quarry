# Quarry Web → Tauri Feature-Parity Analysis and Implementation Plan

**Date:** 2026-08-09  
**Baseline:** `../Quarry-web` at `8d9c249` (`main`)  
**Target:** `Quarry` at `5c26f57` (`main`)  
**Goal:** Match `Quarry-web` product behavior and UI/UX wherever practical while retaining a native Tauri 2 architecture. This is a plan only; no product implementation is included.

## 1. Executive summary

`Quarry` is not a separate implementation from scratch. It already shares much of the React UI and Rust domain code with `Quarry-web`: 40 React source files are byte-identical, 37 common React files have diverged, six React files exist only on the web, and five exist only on desktop. The desktop app also has solid native foundations: app-data SQLite, native file dialogs, local filesystem parsing and preview, Tauri events, Helix clients/query builders, document parsers, and standard desktop menus.

The main parity problem is that recent web functionality has not been carried through the desktop's React-to-Rust boundary. The highest-impact gaps are:

1. **Persisted deals are not restored in the desktop UI.** Desktop can insert a deal into SQLite, but has no `list_deals`, `get_deal`, or `archive_deal` command and React still resolves most deals from static fixtures. A newly created deal is therefore available only through navigation state until the route/app is reloaded.
2. **The desktop data-room root is disconnected from persisted deals.** A selected root is stored in `deals.main_data_room_folder`, but `list_deal_data_room` ignores SQLite and resolves only environment variables or three hard-coded fixture paths. This breaks the normal add-deal → reopen-data-room workflow.
3. **The web document-ingestion UI and current ingestion semantics are absent.** Web has a multi-file modal, validation, per-file background status, retry, elapsed time, content-hash deduplication, user-scoped document identity, and terminal `completed`/`skipped`/`failed` events. Desktop has a developer-only `files:process` event pipeline, but it does not accept a user ID, does not deduplicate, is not wired into the data room, and persists documents with an empty user scope.
4. **Recent web shell and deal-room UX is missing.** The collapsible aligned sidebar, page header bar, current typography/tokens, new Hub, suggested content, deal resources, key-question table, analyzed-file review table, and editable timeline have not been ported.
5. **The web activity log is missing.** Desktop has a useful generic `execute` wrapper and event hook, but they do not record redacted IPC activity, and there is no Logs page, filtering, session clearing, or JSON export.
6. **Several web backend capabilities have no Tauri equivalent at the IPC boundary.** This includes deal list/archive, current Helix document search, idempotent ingestion, and West Monroe file extraction/index/GraphRAG clients. Most of these are backend/integration capabilities rather than currently reachable web UI.
7. **The Tauri security baseline needs correction before expanding IPC.** `src-tauri/tauri.conf.json:21` disables CSP, custom commands accept raw filesystem paths, command errors expose internal details, and a full user API key is returned to React even though the UI only renders a masked version. The web repository also contains a committed credential-like value in `../Quarry-web/backend/.env.example:1`; rotate/revoke it, replace the example with an empty placeholder, and remove it from Git history before implementation begins.

Recommended sequence:

- **P0:** settle shared contracts and secret handling; harden Tauri; add deal read/archive commands and schema migration; connect data rooms to stored deal roots; port the current shell and route structure; replace the stale ingestion event path with a user-scoped, deduplicated job service and wire the adapted web upload modal.
- **P1:** port the Hub/deal-room/timeline/analyzed-files UX; add the native activity log; expose Helix search; align error handling and user feedback.
- **P2:** expose backend-only West Monroe research integrations if they remain product requirements; route-split desktop; finish nonfunctional/stub surfaces; remove dead developer code or gate it behind a development flag.

The fastest safe approach is **shared UI plus platform adapters**, not copying `backendApi.ts` into desktop and not embedding Axum. Pure React components should be ported nearly verbatim. File selection, persistence, jobs, export, secrets, and OS integration should remain Tauri-specific.

## 2. Scope, method, and validation

The analysis traced:

- React routes from `App.tsx` into pages, shell components, hooks, static data, stateful controls, and API/IPC clients.
- Every Axum route in `../Quarry-web/backend/src/routes/` through handlers, services, repositories, SQLite/Helix state, job state, and external clients.
- Every registered Tauri command/event in `src-tauri/src/lib.rs:103` through services, repositories, local filesystem access, SQLite/Helix state, and frontend call sites.
- User-visible persistence, validation, progress/error behavior, file formats, background processing, search/filter state, and import/export behavior.

Validation performed:

| Repository area | Result |
|---|---|
| `../Quarry-web/frontend` | `npm run build` passed. Route-split output; the PDF preview chunk remains large. |
| `Quarry` React frontend | `npm run build` passed. One approximately 2.1 MB JS bundle; no route splitting. |
| `../Quarry-web/backend` | `cargo test` passed: 79 tests. |
| `Quarry/src-tauri` | `cargo test` passed: 101 tests, 4 environment-dependent tests ignored. Six dead-code warnings remain. |
| Working trees before analysis | Both clean on `main`. Build outputs are ignored. |

Status terms used below:

- **Implemented / equivalent:** same user outcome and materially equivalent UX.
- **Partially implemented:** some behavior or UI exists, but a meaningful user outcome differs.
- **Missing:** no reachable equivalent in `Quarry`.
- **Not applicable to desktop:** the exact browser/server mechanism should not be reproduced.
- **Desktop implementation exists but differs appropriately:** the outcome is equivalent through a native mechanism.

## 3. Architecture comparison

| Concern | Quarry-web | Quarry today | Parity direction |
|---|---|---|---|
| Runtime | React in a browser plus an Axum HTTP server | React in a Tauri 2 webview plus in-process Rust | Keep Tauri in-process; do not add Axum. |
| Frontend transport | `fetch`, multipart upload, `EventSource` in `frontend/src/lib/backendApi.ts` | `invoke`, plugin dialogs, Tauri events in `src/lib/tauri/` | Introduce a typed product API whose desktop implementation uses commands/events. |
| Local data | Server-side SQLite in `backend/data` or configured directory | SQLite under Tauri app-data via `SqliteClient` | Desktop implementation is appropriate; align schema and capabilities. |
| Files | Browser uploads bytes; optional server-local path endpoints | Native path selection and local Rust filesystem access | Prefer path-backed native workflows and validate every canonical path in Rust. |
| Background jobs | Tokio tasks, job map, `watch`, SSE | Tauri listener spawns Tokio work and emits events | Replace the public frontend→backend event trigger with a validated command returning job IDs; retain backend→frontend progress events. |
| Document graph | Helix plus user-scoped content hash, ingestion state, vector/BM25 search | Helix plus older local-path parser/persistence path | Port current domain semantics, not HTTP handlers. |
| External AI | OpenAI and optional WM File/Index/GraphRAG services from server-side Rust | OpenAI from native Rust; no WM service clients | Keep all secrets and network calls in Rust; add commands only where product/API parity requires them. |
| Session/preferences | Email in `sessionStorage`; theme in `localStorage` | Same | Equivalent; consider whether desktop should remember the user across launches as a separate product decision. |
| Security boundary | CORS, timeout, HTTP input validation, server-only secrets | Tauri capability/command/CSP boundary | Replace HTTP protections with least-privilege capabilities, restrictive CSP, origin/window restrictions, typed validation, and safe serialized errors. |
| Desktop features | Not applicable | Native menus, dialogs, local conversion, Tauri playground | Preserve native menus/dialogs/conversion. Gate the playground instead of deleting it. |

Recommended target layering:

```text
React pages and shared components
            |
    typed product API
            |
  Tauri command/event adapter
            |
validated commands + safe errors + job manager
            |
domain services (deals, files, documents, research)
            |
SQLite / Helix / OpenAI / OS filesystem / WM services
```

The React layer should not know whether the equivalent web operation was an HTTP request or SSE stream. It should deal in domain operations such as `listDeals`, `startDocumentJobs`, `subscribeToDocumentJobs`, and `saveActivityLog`.

## 4. Product inventory and feature-parity matrix

### 4.1 Pages and navigation

Web routes are defined in `../Quarry-web/frontend/src/App.tsx:28`; desktop routes are in `src/App.tsx:16`.

| Web baseline feature | Evidence in Quarry-web | Quarry status | Evidence/current behavior | Priority |
|---|---|---|---|---|
| Login and new-user registration | `frontend/src/pages/LoginPage.tsx`; `/api/users` and `/api/users/by-email` | Desktop implementation exists but differs appropriately | `src/pages/LoginPage.tsx` invokes `create_user` / `user_exists_by_email`; SQLite is app-local. | P0 contract hardening only |
| Workspace email session | `frontend/src/hooks/useWorkspaceSession.ts` | Implemented / equivalent | Same hook is byte-identical. | — |
| Account page and masked API-key display | `frontend/src/pages/AccountPage.tsx` | Implemented / equivalent behavior, partial security | Desktop lookup is equivalent, but Rust returns the full key to React. | P0 security |
| Light/dark preference | `frontend/src/hooks/useThemeMode.tsx`; `ProfilePreferences.tsx` | Implemented / equivalent | Same preference mechanism; shell styling has diverged. | P0 UI |
| Hub, account, vault, summarize, deal room, and data room routes | `frontend/src/App.tsx` | Implemented / equivalent routing | All primary routes exist. | — |
| Logs route and navigation link | `frontend/src/App.tsx:41`; `frontend/src/pages/LogsPage.tsx:30` | Missing | Desktop exposes a Tauri Playground route instead. | P1 |
| Lazy route loading | `frontend/src/App.tsx:9` | Missing | Desktop eagerly imports every page and builds one approximately 2.1 MB JS chunk. | P2 |
| Aligned page header and collapsible sidebar | `WorkspaceLayout.tsx`, `WorkspaceHeader.tsx`, `sidebar/SidebarFrame.tsx` | Partially implemented | Desktop has the older fixed sidebar/layout and no collapse control. | P0 |
| Responsive/default desktop window | Web relies on browser viewport | Partially implemented | Tauri opens at 800×600 (`src-tauri/tauri.conf.json:16`), while the sidebar is hidden below the `lg` breakpoint and the data-room layout needs roughly 1,068 px before padding. There is no compact-nav alternative. | P0 |
| Native app/edit/view/window menus | Not applicable to desktop | Desktop implementation exists but differs appropriately | `src-tauri/src/lib.rs:35` builds standard menus. Preserve them. | — |
| Custom keyboard shortcuts | Escape-to-close and normal browser controls only | Implemented / equivalent plus desktop defaults | Both support Escape in modal/menu flows. Desktop has standard menu accelerators but no product-specific global shortcuts. | P2 if desired |

### 4.2 Hub, deals, and deal room

| Web baseline feature | Quarry status | Key gap | Priority |
|---|---|---|---|
| Current Hub composition: AI search, Suggested Content tabs/deal selector, and activity-stream cards | Partially implemented | Desktop renders the previous tasks/recent-files/insights grid. | P1 |
| Persisted active deals merged with fixture deals | Missing | Desktop never lists SQLite deals after creation/restart; web does this in `useWorkspaceDeals.ts:6`. | P0 |
| Create deal from a selected data-room folder | Desktop implementation exists but differs appropriately | Desktop native folder selection and automatic SOW/timeline discovery are valid improvements. Keep them, but align validation/DTOs and restore the deal later. | P0 |
| Select SOW and optional project timeline, extract key questions | Partially implemented | Desktop can select discovered local paths, but still extracts/persists `investmentThesis`, which the web model and migration removed. | P0 |
| List/get/archive active deals | Partially implemented | Desktop repository has create/get-by-ID only and registers no read/archive commands. Web repository provides list/get-with-metadata/archive (`deal_repository.rs:136`, `:232`). | P0 |
| Helix deal save/get integration | Partially implemented | Desktop has query builders/upsert repository code but no user-facing command pair equivalent to web `/deals/helix`. | P2/backend parity |
| Deal-room header with SOW, Fact Sheet, and SharePoint VDR resources | Missing UI | Desktop renders the older subtitle header. Web resource items are currently labels only, not links. | P1 |
| Deal overview and current summary layout | Partially implemented | Desktop shows the older large title/badges/thesis card and separate activity timeline. | P1 |
| Key questions as question/answer table | Missing UI | Desktop uses static question tiles and a separate thesis panel. | P1 |
| Metrics | Implemented / equivalent data, partial UI | Desktop renders the same metrics but in the prior layout. | P1 |
| Analyzed-files strip and review table | Missing | Web supports column visibility and in-memory Reviewed/Needs review toggles in `InsightsStrip.tsx:59`. Desktop only has the older insight chips. | P1 |
| Timeline calendar, task board, and activity log | Partially implemented | Desktop can add activities, but web adds cell-click creation, time, edit/move, chronological display, and an activity list (`DealTimelineView.tsx:83`). Both are in-memory only. | P1 |
| Diligence Graph, Site Visits, Synthesis Canvas | Implemented / equivalent current behavior | Both route these sections to `UnderConstructionView`. Richer dormant components are not reachable and are not baseline functionality. | — |
| Deal archive UI | Missing in both | Web has an API client/helper but no reachable archive button. Treat as backend parity until a UI decision is made. | P2 UI / P0 command |

Important baseline limitations:

- Hub activity filter buttons are visual only; they do not update filter state.
- Suggested Content tabs work, but the selected deal changes only the menu label and does not filter the static content groups.
- Task completion buttons on the Hub do not update data.
- Review statuses and timeline edits are component-local and reset on navigation/reload.
- The web's deal-resource labels are not linked to actual files or SharePoint.

Port these as UI parity first. Do not silently invent persistence or external integrations without a product decision.

### 4.3 Data room, document processing, and search

| Web baseline feature | Quarry status | Key gap | Priority |
|---|---|---|---|
| Tree navigation, folder expansion, sorted folders/files, error/loading states | Partially implemented | Components are nearly equivalent, but desktop root resolution ignores the deal's stored folder. | P0 |
| Collapse/reopen explorer and document-search panels | Implemented / equivalent | Same behavior with minor header sizing differences. | P1 visual sync |
| PDF preview with page navigation, zoom, loading/error states | Desktop implementation exists but differs appropriately | Tauri reads local PDF bytes in Rust and returns base64. | — |
| DOCX/XLSX/PPTX preview conversion | Desktop implementation exists but differs appropriately | Local LibreOffice conversion in Rust is appropriate. Preserve improved desktop error detail and discovery. | — |
| Current data-room header/editor alignment | Partially implemented | Web aligns all panel headers to 64 px and moves view controls into the header. | P1 |
| “Upload New File” menu action | Missing | Desktop menu item closes without starting a workflow. | P0 |
| Multi-file validation (PDF/DOCX, nonempty, 50 MB), selection/removal | Missing UI | `UploadFilesModal.tsx:30` exists only on web. | P0 |
| Independent background jobs, per-file status, elapsed time, retry, duplicate skip | Partially implemented | Desktop has `files:process`/progress events only in the playground; the service lacks user scope and deduplication. | P0 |
| User-scoped content identity and Helix graph | Partially implemented | Desktop parser path passes an empty user ID in `core/parsers/mod.rs`; web uses user ID + content hash and serializes same-content work. | P0 |
| Vector and keyword chunk-search capability | Partially implemented | Desktop has Helix query builders in `core/helix_queries/files/search_quarry_file.rs:23` but no repository wrappers/commands. | P1 |
| Document Search UI (query, semantic/keyword toggle, category/date filters, highlights) | Implemented / equivalent current UI | Both search static `DataRoomChip`/featured mock results; neither frontend calls the real Helix search endpoints. | —; P2 to productize |
| Report editor and view menu | Implemented / equivalent current behavior, visual drift | Data is static; Document/Outline/Table changes only menu state. | P1 visual sync |

Web ingestion is implemented in `backend/src/services/document_ingestion_service.rs:57`, `backend/src/handlers/documents/process.rs`, `backend/src/repository/document_repository.rs:124`, and `backend/src/document_jobs.rs`. The desktop equivalent should reuse/port those domain semantics but accept local paths in a validated command rather than uploaded multipart bytes.

### 4.4 Summarize, vault, logs, and export

| Web baseline feature | Quarry status | Key gap | Priority |
|---|---|---|---|
| Summarize a file, folder, selected subset, or manual path | Desktop implementation exists but differs appropriately | Desktop uses native dialogs and Rust reads local paths; this is the correct architecture. | P0 hardening only |
| Supported-file tree, selection counts, skipped files, markdown rendering | Implemented / equivalent | Core UI and behavior are materially equivalent. | — |
| Save markdown summary | Desktop implementation exists but differs appropriately | Native save dialog plus Rust write is preferable to browser download. Command must stop accepting arbitrary paths without a verified save grant. | P0 security |
| Chat tab | Implemented / equivalent current behavior | The tab is presentational; no chat backend exists in either app. | — |
| Global Vault file/folder staging | Implemented / equivalent functionality, partial UI | Both only display selected names/counts in memory and always show an empty/pending markdown preview. Web header styling is newer. | P2 |
| Session activity capture | Missing | Web logs every `backendApi` request and SSE event with duration/status/redacted payload. | P1 |
| Log search, source/status filters, counters, expandable payloads | Missing | No desktop Logs page. | P1 |
| Clear and export logs as JSON | Missing | Desktop should use the native save dialog/atomic Rust write rather than a browser download. | P1 |

### 4.5 Backend, persistence, authentication, and integrations

| Web baseline capability | Quarry status | Desktop translation | Priority |
|---|---|---|---|
| SQLite users | Desktop implementation exists but differs appropriately | Keep app-data SQLite and case-insensitive email uniqueness. | — |
| SQLite active deal list/get/archive and metadata | Partially implemented | Add repository methods and commands; migrate metadata DTOs. | P0 |
| User and deal Helix upsert/get | Partially implemented | Keep Rust clients; expose only typed commands needed by UI/integrations. | P2 |
| Idempotent document ingestion and ingestion-complete state | Missing in current desktop path | Port current hash lookup, per-document lock, batching, and completion marker. | P0 |
| User-partitioned vector/BM25 search | Partially implemented | Add repository/service/command layers and validate user ID/query/limit. | P1 |
| WM file extraction | Missing | Rust reqwest client + typed command reading granted local files. No frontend secret. | P2 |
| WM index create/status | Missing | Rust client + commands; use Tauri job/events for polling if surfaced. | P2 |
| WM GraphRAG query | Missing | Rust client + command, with secrets stored natively. | P2 |
| Axum health endpoint, CORS, request IDs, HTTP compression/timeouts | Not applicable to desktop | Replace with a native diagnostics/status command, tracing/correlation IDs, and per-operation timeouts. Do not embed Axum. | P1 |
| SSE job transport | Not applicable to desktop | Use Tauri backend→frontend events keyed by job ID. | P0 |
| Login/authentication | Implemented / equivalent baseline limitation | Both trust email existence; neither has passwords, tokens, authorization, or protected routes/commands. Do not label this real authentication. | Product decision |
| API-key handling | Partially implemented and unsafe in both | Move secrets out of SQLite/React if possible; return `hasApiKey` or a masked value. | P0 security |
| Structured public errors | Partially implemented | Web sanitizes internal errors. Desktop aliases errors to `String` and prefixes internal context. Add a serializable safe error DTO and log internal causes in Rust. | P0 |

## 5. Detailed missing and partial feature analysis

### 5.1 Persisted deal lifecycle — P0

**What web does**

- `frontend/src/hooks/useWorkspaceDeals.ts:6` calls `listDeals`, maps SQLite rows/metadata to `WorkspaceDeal`, merges them with fixtures, and exposes a loading state.
- Deal room and data room wait for persisted loading before redirecting.
- Axum exposes list, get, create, upload-create, extraction, and archive routes in `backend/src/routes/deal.rs:15`.
- `backend/src/repository/deal_repository.rs:136` lists active deals ordered by update time and joins metadata; `:232` archives rather than deleting.

**What desktop has**

- Add Deal inserts SQLite data and navigates with an extraction result in route state.
- `src-tauri/src/repository/deal_repository.rs:62` can create and `:161` can get one row by ID.
- React resolves normal routes from `workspaceDeals`, not SQLite.

**Specific gap**

- Reloading or directly navigating to a created deal redirects to the Hub.
- Active-deal navigation never includes stored deals.
- There is no native archive operation.
- Desktop metadata still requires `investment_thesis`, while web deliberately migrates it away in `backend/src/state.rs:291`.

**UI reuse**

- Port `useWorkspaceDeals.ts` and `buildWorkspaceDealFromPersisted` nearly verbatim.
- Replace only the API import with the typed Tauri product API.

**Tauri/Rust work**

- Extend `deal_repository.rs` with `DealWithMetadata`, `list_deals`, `get_deal_with_metadata`, `get_deal_metadata_by_deal_id`, and `archive_deal` using the tested web SQL adapted to `with_sqlite_db`.
- Register `list_deals`, `get_deal`, and `archive_deal` commands.
- Add a schema migration (next `user_version`) that establishes the canonical metadata shape. Preserve legacy investment-thesis data in a legacy/optional column or migration backup if the desktop-only briefing must remain; do not let it remain required by baseline DTOs.
- Make list/get database work run off the webview thread (`spawn_blocking` or an async DB strategy).

**Dependencies/risks**

- Existing desktop databases may contain investment-thesis data; migration must be forward-only and covered by a fixture test.
- Fixture IDs are strings while persisted IDs are numeric strings; keep the merge rule from web to avoid duplicates.
- Decide whether archived deals need a recovery view before exposing archive UI.

### 5.2 Stored data-room roots — P0

**What web does**

- The tree/preview route resolves deal-specific environment configuration in `backend/src/services/data_room_service.rs` and validates relative preview paths beneath the canonical root.

**What desktop has**

- Add Deal stores the chosen native directory in `main_data_room_folder`.
- Preview path traversal is correctly blocked by canonical-root checks.
- `list_deal_data_room` does not receive `AppState` and never reads the stored deal.

**Specific gap**

The normal desktop-created deal cannot reopen its selected data room unless an environment variable happens to be added separately.

**UI reuse**

No major UI rewrite is required. Existing `DataRoomPage` and explorer can remain; persisted deal loading and a corrected command fix the workflow.

**Tauri/Rust work**

- Change list/preview commands to accept `State<AppState>`.
- Resolve the root in this order: persisted deal path; explicit fixture/dev override for known fixtures; otherwise a safe not-found error.
- Store and compare a canonical root. Resolve only relative paths beneath it and reject absolute paths, `..`, symlink escapes, missing files, and unsupported preview extensions.
- Do not return full local paths unless the UI genuinely needs them; use a display name and relative paths.

**Dependencies/risks**

- Moved/unmounted folders need a clear rebind workflow, not a generic internal error.
- The web browser-upload deal stores a pseudo-root and does not persist uploaded files into the tree. This is a web baseline ambiguity, not behavior to reproduce on desktop.

### 5.3 Add Deal and extraction contract — P0

**What web does**

- Browser selects a directory, uploads its supported files, creates a deal, requires a selected SOW, optionally accepts a timeline, extracts only key questions, saves metadata, and navigates to the result (`AddDealModal.tsx:50`, `:154`).

**What desktop has**

- Native directory chooser.
- Automatic discovery of SOW/timeline candidates, with selection UI.
- Local file validation and path containment.
- OpenAI extraction of key questions plus a sell-side investment thesis.

**Specific gap**

- Persisted reload is missing.
- Desktop validation only checks nonempty common fields; web validates the supported deal-type enum and type-specific company requirements in Rust.
- Desktop response/metadata types include a removed baseline field.

**Recommended desktop behavior**

Retain native directory selection and automatic discovery as a desktop-appropriate enhancement. Port the current web form styling and type validation. Preserve the desktop-only thesis as optional supplemental functionality without requiring it in the baseline deal DTO or displacing the web key-question UI.

**Likely files**

- React: `src/components/hub/sidebar/AddDealModal.tsx`, `src/data/dealExtraction.ts`, new product API adapter.
- Rust: `src-tauri/src/commands/deal.rs`, `services/deal_service.rs`, `repository/deal_repository.rs`, `core/clients/sqlite.rs`.

### 5.4 Current shell and Hub — P0/P1

**What web does**

- 64 px aligned top header.
- Collapsible 288→80 px sidebar with new tokens/typography.
- Logs tool link.
- Current Hub composition in `frontend/src/pages/HubPage.tsx:66`.
- Suggested Content tab state and deal-picker menu in `SuggestedContentCard.tsx:71`.

**What desktop has**

- Older floating/ambient layout and fixed-width sidebar.
- Older Hub grid of critical tasks, recent files, and insights.
- Tauri Playground in primary tool navigation.

**Specific gap**

This is substantial visual/workflow drift, and the 800 px default window hides the desktop sidebar entirely.

**UI reuse**

Directly port/adapt:

- `WorkspaceLayout.tsx`
- `WorkspaceHeader.tsx`
- `WorkspaceHomeShell.tsx` after the deals hook exists
- all changed files under `components/hub/sidebar/`
- `SuggestedContentCard.tsx`
- `HubPage.tsx`
- new `Icon` variants and current `index.css` tokens

Preserve the Tauri Playground by development-gating its route/navigation (`import.meta.env.DEV`) or keeping an unlinked deep route. Do not let it replace the baseline Logs tool in production.

**Window decision**

Set an appropriate initial/minimum window size and also support a compact state. Merely increasing initial width does not solve users resizing below the breakpoint. Acceptance should cover at least 1,024×700 and the chosen normal desktop size.

### 5.5 Deal-room summary, analyzed files, and timeline — P1

**What web does**

- Header resource labels plus separate overview.
- Metrics and key-question answer table.
- Analyzed-file chips plus configurable review table.
- Calendar cell creation; time entry; editing/moving an existing event; activity list; task board.

**What desktop has**

- Older deal title/status/thesis card, older insight strip, separate overview activity timeline.
- Timeline add flow without time/edit/move.

**UI reuse**

These components are mostly platform-neutral and should be ported directly:

- `DealRoomHeader.tsx`
- `DealSummaryCard.tsx`
- `InsightsStrip.tsx`
- `DealTimelineView.tsx`
- relevant `data/workspace.ts` type changes and additional mock insight rows

**Tauri work**

None for exact baseline behavior because review/timeline edits are in-memory on web. If product chooses persistence, add a schema and commands as a separate enhancement; do not imply that web already persists them.

### 5.6 Document ingestion and upload workflow — P0

**What web does**

- `UploadFilesModal.tsx:30` validates and stages PDF/DOCX files, tracks selection, prevents close during active jobs, starts each independently, retries failures, and shows skip/completion data.
- The backend computes SHA-256 on bytes, derives a user-scoped document identity, serializes concurrent same-document work, checks Helix for completed content, parses/chunks/embeds, batch-persists, marks ingestion complete, and emits terminal job state.

**What desktop has**

- `register_file_events` listens for arbitrary frontend-emitted `files:process` payloads.
- `process_files` reads paths, parses, embeds, persists, and emits basic batch progress.
- Query builders for user-scoped search exist.

**Specific gap**

- No data-room UI opens or consumes this service.
- Public event payloads are not a validation boundary.
- No user ID is supplied; parser calls use `""` as user scope.
- No file-size/count validation, same-content skip, in-flight lock, ingestion-complete marker, bounded concurrency, or job retention/status lookup.
- A failed chunk batch can leave partial graph state without the current recovery semantics.

**Recommended Tauri design**

1. Add `start_document_jobs({ userId, paths }) -> { jobs }` as a command. Validate count, extensions, canonical paths, ownership/grant, per-file size, and total size before spawning.
2. Add managed `DocumentJobManager` state keyed by UUID. Track processing/terminal state and a per-document async lock.
3. Read local bytes in Rust and pass them to a ported common ingestion service. Use bytes for SHA-256; keep `local_path` only as optional desktop metadata.
4. Emit `documents:job` events with `{jobId, filename, status, documentId?, chunkCount?, error?}`. Scope events to the main window/app instance.
5. Keep terminal status long enough for page remount/reconnect and add `get_document_job(jobId)` if needed.
6. Adapt `UploadFilesModal` from browser `File` objects to native `SelectedLocalFile` DTOs returned by a native open dialog/metadata command. Keep the web status UX.
7. Deprecate the frontend→backend `files:process` event trigger after the command path is covered. Keep compatible backend→frontend events only if another desktop workflow consumes them.

**Dependencies/risks**

- Helix should not block application launch. Initialize indexes in a recoverable background/first-use path and surface a service-unavailable error.
- OpenAI and Helix retries need timeouts and cancellation behavior.
- Large IPC payloads should be avoided: pass paths/metadata, not base64 file bodies, between React and Rust.

### 5.7 Activity logs — P1

**What web does**

- `activityLog.ts:119` records pending/completed API entries and SSE events, truncates payloads, redacts sensitive keys/query parameters, stores up to 400 entries in session storage, and never lets logging fail the operation.
- `LogsPage.tsx:30` provides counters, search, source/status filters, expanded details, clear, and export.

**What desktop has**

- `src/lib/tauri/command.ts` centralizes `invoke` and normalizes thrown errors.
- `useTauriEvent.ts` centralizes event subscription.

**Recommended adaptation**

- Add `src/lib/activityLog.ts` based on web, with sources `ipc` and `event` rather than `api` and `sse`.
- Instrument `execute` with command name, redacted/summarized args, duration, status, and safe response shape. Never log file bytes, full filesystem paths by default, API keys, email, authorization, or OpenAI/Helix responses containing sensitive document text.
- Provide a logged event subscription wrapper for document jobs.
- Port `LogsPage` and change copy from “Browser activity” to “Desktop activity.”
- Export through native save dialog + an atomic validated Rust write. Include app version/platform/session identifier but no secrets.

### 5.8 Search and West Monroe research services — P1/P2

**Web capabilities**

- Helix vector and BM25 routes.
- WM file extraction, index creation/status, and GraphRAG query routes in `backend/src/routes/research.rs:15` and clients in `core/clients/wm_ai_services.rs`.

**Desktop current state**

- Helix search query builders exist but are not exposed.
- WM client code does not exist.
- Current data-room search UI is a local/mock filter and does not call either backend.

**Recommended implementation**

- P1: add `search_document_chunks_keyword` and `search_document_chunks_vector` service/command layers so the backend capability exists. Validate user ID, nonempty query/embedding, bounded limit, and safe result DTOs.
- P2: port WM clients into native Rust only if these endpoints remain part of the required product contract. Add `reqwest` multipart support, per-service timeouts, safe errors, and OS-native secret storage/environment fallback.
- A separate product decision is required before replacing mock Document Search results with live search. That would be a behavior enhancement beyond current web frontend parity.

### 5.9 Error handling, feedback, and diagnostics — P0/P1

**Web**

- HTTP errors are typed at the frontend and internal Axum errors are logged but returned as generic `internal server error`.
- Major flows render inline errors, spinners, empty states, and per-file statuses.

**Desktop**

- `CommandResult<T> = Result<T, String>` and `command_context` expose command/internal error text directly to React.
- React generally catches and renders errors, but there is no consistent code/retryability/category contract.

**Recommended desktop error contract**

```ts
type DesktopError = {
  code: "validation" | "not_found" | "permission" | "service_unavailable" | "conflict" | "internal";
  message: string;       // safe, actionable user text
  operationId?: string; // correlate with Rust tracing/activity log
  retryable?: boolean;
};
```

- Define a Rust `AppError` enum with internal source/context and a custom safe `Serialize` implementation.
- Validate at command boundaries, map known failures to stable codes, log internal causes with tracing, and never serialize credentials, SQL/Helix internals, raw remote bodies, or unrestricted local paths.
- Add a native `diagnostics_status` command for SQLite path/version, Helix reachability/index readiness, LibreOffice availability, and configured external services. Do not recreate `/health` HTTP.

## 6. React UI parity analysis

### 6.1 Reuse classification

| Classification | Files/features | Guidance |
|---|---|---|
| Direct port, low risk | `WorkspaceHeader`, `WorkspaceLayout`, most sidebar components, `HubPage`, `SuggestedContentCard`, `DealRoomHeader`, `DealSummaryCard`, `InsightsStrip`, `DealTimelineView`, `Icon`, CSS tokens | Copy the current web component and resolve only target-specific imports/types. Preserve semantic/ARIA behavior. |
| Port with a platform adapter | `WorkspaceHomeShell`, `DealRoomPage`, `DataRoomPage`, `LoginPage`, `ProfilePreferences`, `LogsPage` | Keep UI/state; replace `backendApi` imports with typed product operations. |
| Adapt substantially | `AddDealModal`, `UploadFilesModal`, `SummarizePage`, activity-log export | Preserve UX but use native dialogs, paths/grants, commands, and Tauri events. |
| Keep desktop-specific | Tauri app menus, native path/Office preview, native save dialogs, `src/lib/tauri/*`, app-data SQLite | Do not copy browser upload/download assumptions. |
| Development-only desktop feature | `TauriPlaygroundPage`, login-demo events, generic emit controls | Preserve behind a development flag or unlinked debug route; do not show in production navigation in place of Logs. |
| Dormant/non-baseline | `DiligenceGraphView`, `SiteVisitsView`, `DealBriefingCard`, unused critical/recent cards | Do not prioritize based only on file presence. They are not reachable baseline features. |

### 6.2 Styling differences

Web changed:

- font stacks from Aptos to Inter/SF Pro/system;
- heading sizes/weights;
- sidebar-specific theme tokens for light/dark;
- aligned 64 px headers;
- sidebar dimensions/spacing/radii;
- removal of ambient background blobs from the main workspace shell;
- log-list content visibility optimization;
- multiple component-specific spacing and card-layout changes.

Port `frontend/src/index.css` as a reviewed diff, not a blind overwrite. Keep any Tauri-specific selectors that are still required, then visually verify light and dark modes at the chosen desktop breakpoints.

### 6.3 Workflow differences that should remain platform-specific

- Browser directory uploads produce `File` objects; desktop should use native directory/file selection and pass canonical paths to Rust.
- Browser summary export uses an `<a download>`; desktop should use the native save dialog.
- Browser ingestion progress uses SSE; desktop should use Tauri events.
- Browser API activity should be labeled API/SSE; desktop logs should be labeled IPC/Event.
- Desktop Office preview conversion can use installed LibreOffice and temporary local directories; browser clients cannot.
- Tauri menus, window behavior, and development playground have no web equivalent.

## 7. Tauri-specific implementation approach

### 7.1 Typed product API

Create `src/lib/product/` (name can vary) with domain interfaces and a Tauri implementation. Avoid importing Tauri APIs directly throughout pages.

Suggested frontend surface:

```ts
interface QuarryProductApi {
  createUser(input: CreateUserInput): Promise<AccountUser>;
  getUserByEmail(email: string): Promise<AccountUser | null>;
  listDeals(): Promise<PersistedDeal[]>;
  getDeal(id: number): Promise<PersistedDeal>;
  archiveDeal(id: number): Promise<SavedDeal>;
  createDeal(input: CreateDealInput): Promise<DealSourceCandidates>;
  extractDeal(input: ExtractDealInput): Promise<DealExtractionResponse>;
  listDealDataRoom(id: number | string): Promise<DealDataRoom>;
  previewDealDocument(id: number | string, relativePath: string): Promise<DocumentPreview>;
  selectDocuments(options: SelectDocumentsOptions): Promise<SelectedLocalFile[]>;
  startDocumentJobs(input: StartDocumentJobsInput): Promise<DocumentJobStart[]>;
  summarize(input: SummarizeInput): Promise<string>;
}
```

Keep command names centralized, but derive TypeScript/Rust payload fixtures or contract tests so camelCase drift is detected.

### 7.2 Proposed commands and events

| Command/event | Purpose | Boundary rules |
|---|---|---|
| `list_deals` | Active deals + optional metadata | No raw SQL errors; run DB work off UI thread. |
| `get_deal` | One deal + metadata | Positive numeric ID; not-found code. |
| `archive_deal` | Soft archive | Positive ID; idempotency decision documented. |
| `select_deal_root` or dialog adapter | Native directory selection | Return an opaque grant/root DTO where possible. |
| `list_deal_data_room` | Tree from stored root | Resolve via SQLite; return relative paths only. |
| `preview_deal_document` | Validated PDF/Office preview | Relative path beneath canonical stored root; size limits. |
| `select_ingestion_files` | Native PDF/DOCX chooser + metadata | Supported extensions; no file bytes in IPC. |
| `start_document_jobs` | Validate and spawn ingestion | User ID, granted paths, bounds, UUID jobs. |
| `documents:job` event | Progress/terminal state | Include job ID; safe error only. |
| `get_document_job` | Recover status after remount | Job ID validation and retention limit. |
| `search_document_chunks_keyword` | Helix BM25 | User ID, bounded limit, nonempty query. |
| `search_document_chunks_vector` | Helix vector search | User ID, expected embedding dimension, bounded limit. |
| `save_activity_log` | Native JSON export | Path selected by save dialog; atomic write; payload size bound. |
| `diagnostics_status` | Native health equivalent | No secrets or full local paths in response. |

### 7.3 State and background work

- Extend `AppState` with an `Arc<DocumentJobManager>` and correlation-ID/tracing support.
- Use Tokio async mutex/RwLock for job maps and per-document locks. Do not hold the SQLite `std::sync::Mutex` across `.await`.
- Use `spawn_blocking` for filesystem scans, parsing, LibreOffice invocation, and synchronous rusqlite work.
- Bound concurrent document jobs (web uses 8; confirm desktop memory/CPU profile before adopting the same value).
- Emit terminal state before cleanup; retain it for a bounded period.
- Clean temporary preview directories on both success and failure; clean job resources when the main window is destroyed.

### 7.4 Persistence

- SQLite remains the source of truth for local users, deals, metadata, preferences that need durable native storage, and optional job history if product requires it.
- Helix remains the document graph/search store. Do not use it as a substitute for the local deal list without an explicit synchronization design.
- `sessionStorage`/`localStorage` are acceptable for exact baseline session/theme behavior; durable desktop sign-in is a separate decision.
- If review/timeline state becomes durable, add explicit tables and commands. Do not overload mock `workspace.ts` data.

### 7.5 Filesystem permissions and capabilities

Current `src-tauri/capabilities/default.json` grants dialog open/save and opener defaults, while custom Rust commands can still read/write any path they accept. Tauri plugin scope does not automatically protect custom Rust filesystem code.

Required controls:

- Configure a restrictive CSP instead of `csp: null`. Expected needs include `default-src 'self'`, inline Tailwind styles as narrowly required, `img-src 'self' data: blob:`, and `worker-src 'self' blob:` for inline PDF.js worker behavior. Keep webview `connect-src` closed if all network calls stay in Rust.
- Set `freezePrototype` if supported by the installed Tauri version/config schema.
- Remove `opener:default` unless a reachable feature needs all default opener permissions; grant the narrow opener permission actually required.
- Restrict commands to the `main` window and validate its local Tauri origin for sensitive operations.
- Never accept an arbitrary read path merely because React sent it. Establish roots via native selection/persisted deal binding and re-canonicalize every child.
- Validate save destinations obtained through a native save dialog; prefer an opaque token or perform dialog + write in one Rust operation.
- Disable shell execution. LibreOffice should remain an internally discovered executable invoked with fixed arguments, never a frontend-provided command/argument list.
- Bound preview, upload, summary, and log sizes.

### 7.6 Secret handling

- Immediately rotate/revoke the credential-like value committed in `../Quarry-web/backend/.env.example:1`, blank the example, and purge the secret from Git history. Assume compromise because it was committed.
- Do not expose `TAURI_*` secrets through Vite. Current `vite.config.ts` does not broaden `envPrefix`; keep it that way.
- Do not send full user API keys to React. Return an account DTO with `hasApiKey` and an optional pre-masked display value.
- Prefer OS credential storage (`keyring` or a reviewed Tauri Stronghold integration) for user/service secrets. Keep only a stable reference in SQLite.
- WM/OpenAI/Helix credentials stay in Rust; redact them from activity logs and serialized errors.

## 8. Prioritized implementation roadmap

### P0 — Core parity / blocking

1. **Security incident and architecture decisions**
   - Rotate/purge the committed web credential.
   - Decide canonical deal metadata and legacy thesis preservation.
   - Decide secret store and native path-grant model.
   - Define typed product API and safe error DTO.
2. **Tauri hardening and window baseline**
   - Restrictive CSP, narrow capabilities, command validation/origin/window rules.
   - Default/minimum window plus compact sidebar behavior.
3. **Deal lifecycle and schema**
   - Migration, list/get/archive repositories/commands, persisted deal hook, reload/deep-link behavior.
4. **Stored data-room binding**
   - Resolve root from SQLite; rebind errors; canonical relative preview.
5. **Current shell/navigation baseline**
   - Port header, sidebar, tokens, route layout; production Logs link; dev-gate playground.
6. **Current document ingestion**
   - User-scoped hash/dedupe/locks/batching/completion semantics.
   - Validated command + job manager + events.
   - Adapt and wire Upload Files modal.
7. **Native summary/file hardening**
   - Keep current native UX, validate grants/paths and safe save behavior.

### P1 — Important product parity

1. Port current Hub and Suggested Content UI.
2. Port deal-room header, key-question table, metrics, analyzed-file review table.
3. Port timeline time/edit/move/activity-list behavior.
4. Add desktop activity log, filters, clear, native JSON export.
5. Expose Helix vector/keyword search commands and current safe result DTOs.
6. Add structured user feedback and native diagnostics.
7. Complete visual QA in light/dark and required window sizes.

### P2 — Polish / secondary capability

1. Port WM file extraction/index/GraphRAG clients if they are still required backend capabilities.
2. Decide whether to connect live search to the currently mock Document Search UI.
3. Route-split desktop pages and tune the PDF preview chunk.
4. Port Global Vault header styling; productize the vault only with a separate requirements decision.
5. Gate/remove unused demo events and dead code; resolve Rust warnings.
6. Add real links/actions for deal-resource labels only after destinations/permissions are defined.
7. Consider persistence for timeline/review state; current web baseline is in-memory.

## 9. Detailed implementation tasks

### T0 — Resolve architecture and security decisions

- **Gap:** DTOs, secret ownership, path authority, and legacy thesis semantics differ.
- **Implementation:** Write short ADRs for product API boundary, SQLite/Helix ownership, path grants, secret storage, and legacy thesis preservation.
- **React:** Define domain DTOs independent of HTTP/Tauri transport.
- **Rust:** Define `AppError`, safe serialization, validation helpers, and operation IDs.
- **Likely files:** new `docs/adr/` or `plans/decisions/`; `src/lib/product/*`; `src-tauri/src/errors.rs`; `commands/mod.rs`.
- **Dependencies:** None; blocks T1–T7.
- **Risks:** Migrating existing desktop data; accidental secret/path exposure.
- **Acceptance:** ADRs approved; no frontend DTO contains an unmasked secret; error/path contracts have unit fixtures.

### T1 — Harden Tauri configuration and window behavior

- **Gap:** CSP disabled; broad opener default; 800×600 hides primary navigation.
- **Implementation:** Add restrictive CSP/frozen prototype where supported, narrow permissions, explicit window sizing, compact/collapsible nav behavior.
- **React:** Port collapse UI and add compact access if the sidebar is hidden.
- **Rust/config:** `tauri.conf.json`, capabilities, optional custom command permission definitions.
- **Files:** `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/components/hub/sidebar/SidebarFrame.tsx`, `WorkspaceLayout.tsx`.
- **Dependencies:** T0.
- **Risks:** PDF.js worker/CSP compatibility; accessibility at reduced width.
- **Acceptance:** Packaged debug build works with CSP; no console CSP violations in login, PDF preview, summary, logs; navigation works at default and minimum sizes.

### T2 — Align SQLite and deal repositories

- **Gap:** No list/get-with-metadata/archive; stale metadata requirement.
- **Implementation:** Forward migration, repository methods, tests with legacy DB fixture, command DTOs.
- **React:** None yet.
- **Rust:** Port/adapt web SQL; use `OptionalExtension`; keep DB work off async executor.
- **Files:** `src-tauri/src/core/clients/sqlite.rs`, `repository/deal_repository.rs`, `commands/deal.rs`, `lib.rs`, command tests.
- **Dependencies:** T0.
- **Risks:** SQLite `DROP COLUMN` version compatibility; preserving desktop-only thesis data.
- **Acceptance:** New and migrated DBs list active deals, get metadata, archive without delete, and preserve required legacy data.

### T3 — Add product API and persisted-deal React flow

- **Gap:** Pages import low-level `execute`; no durable deal navigation.
- **Implementation:** Typed Tauri adapter; port `useWorkspaceDeals`; update home/deal/data-room routing loading behavior.
- **React files:** new product API modules, `src/hooks/useWorkspaceDeals.ts`, `WorkspaceHomeShell.tsx`, `DealRoomPage.tsx`, `DataRoomPage.tsx`, `data/dealExtraction.ts`.
- **Rust:** Uses T2 commands.
- **Dependencies:** T2.
- **Risks:** duplicate fixture/persisted IDs; navigation-state compatibility immediately after extraction.
- **Acceptance:** Create a deal, restart/reload, see it in Active Deals, deep-link to deal/data room, archive it, and confirm it leaves the active list.

### T4 — Bind data rooms to stored roots

- **Gap:** Desktop-created deals cannot reopen their chosen folder.
- **Implementation:** State-aware root lookup, canonicalization, missing-root/rebind errors, relative-path-only tree/preview DTO.
- **React:** Show actionable “folder unavailable/rebind” feedback; no raw internal path in generic errors.
- **Rust files:** `commands/data_room.rs`, `services/data_room_service.rs`, `core/data_room_helpers.rs`, deal repository.
- **Dependencies:** T2/T3.
- **Risks:** symlink escapes, removable drives, renamed folders, LibreOffice availability.
- **Acceptance:** Stored folder loads after restart; traversal/symlink escape tests fail safely; moved folder displays an actionable error; PDF/DOCX/XLSX/PPTX preview still works.

### T5 — Port current shell and navigation

- **Gap:** Broad visual/nav divergence and missing Logs route.
- **Implementation:** Port current web shell/header/sidebar/Icon/CSS changes; keep dev-only playground; add route skeleton/lazy imports.
- **React files:** `App.tsx`, `index.css`, `WorkspaceLayout.tsx`, `WorkspaceHeader.tsx`, `WorkspaceHomeShell.tsx`, `components/hub/sidebar/*`, `Icon.tsx`, `data/workspace.ts`.
- **Rust/config:** Window sizing from T1.
- **Dependencies:** T1/T3.
- **Risks:** overwriting native-specific route/tool types; dark-mode contrast; hidden navigation.
- **Acceptance:** Route/header/sidebar screenshots match web at equivalent dimensions in both themes; collapsed sidebar is keyboard accessible; Logs replaces Playground in production nav.

### T6 — Align Add Deal and extraction

- **Gap:** validation/DTO drift, legacy thesis contract, no reload continuity.
- **Implementation:** Retain native folder picker and candidate discovery; port current styling/type-specific validation; use baseline key-question DTO; optionally retain thesis as supplemental desktop data.
- **React files:** `AddDealModal.tsx`, `dealExtraction.ts`.
- **Rust files:** `commands/deal.rs`, `services/deal_service.rs`, repository/migration.
- **Dependencies:** T2–T5.
- **Risks:** selecting files outside root; unsupported/missing SOW; retries creating duplicate deals.
- **Acceptance:** Every deal type validates required fields; SOW required, timeline optional; extraction failure can retry without duplicate deal; result persists and reopens.

### T7 — Replace stale ingestion with document jobs

- **Gap:** No user-facing upload and stale developer event semantics.
- **Implementation:** Port current web ingestion domain behavior; native selector metadata; job manager/commands/events; adapt upload modal; deprecate frontend-trigger event.
- **React files:** adapted `UploadFilesModal.tsx`, `DataRoomExplorer.tsx`, `NewAnalysisMenu.tsx`, `DataRoomPage.tsx`, product API/event logger.
- **Rust files:** new/updated `document_jobs.rs`, `commands/documents.rs`, `services/document_ingestion_service.rs`, `repository/document_repository.rs`, `state.rs`, `lib.rs`; parser/query updates.
- **Dependencies:** T0/T3/T4.
- **Risks:** concurrent duplicate writes, partial Helix state, OpenAI/Helix outage, cancellation/window close, large files.
- **Acceptance:** PDF/DOCX validation; per-file processing; same bytes for same user skip; same bytes for different users remain partitioned; retry failure works; progress survives modal remount; no base64 crosses IPC.

### T8 — Port Hub UI

- **Gap:** Desktop uses previous Hub layout.
- **Implementation:** Port `HubPage` and `SuggestedContentCard`; feed persisted deals to the deal menu.
- **React files:** `pages/HubPage.tsx`, new card, related data/icons/styles.
- **Rust:** None for exact baseline mock behavior.
- **Dependencies:** T3/T5.
- **Risks:** mistaking static controls for functional filters.
- **Acceptance:** Visual/interaction parity for tabs/menu/cards; documented static controls remain static unless separately scoped.

### T9 — Port deal-room and timeline UI

- **Gap:** Old summary/thesis/insights/timeline.
- **Implementation:** Port current components/types; retain optional desktop briefing without displacing baseline.
- **React files:** `DealRoomPage.tsx`, `DealRoomHeader.tsx`, `DealSummaryCard.tsx`, `InsightsStrip.tsx`, `DealTimelineView.tsx`, `workspace.ts`, `Icon.tsx`.
- **Rust:** None for baseline in-memory state.
- **Dependencies:** T3/T5/T6.
- **Risks:** desktop-only thesis regression; time formatting/timezone; lost in-memory edits.
- **Acceptance:** Key-question table, analyzed-file columns/status, add/edit/move/time timeline, activity list, and task board match web; navigation preserves no false promise of persistence.

### T10 — Add desktop activity logs

- **Gap:** Missing logs and export.
- **Implementation:** Port/redesign logger for IPC/events, port page, instrument product adapter, native export.
- **React files:** new `activityLog.ts`, `LogsPage.tsx`, `App.tsx`, workspace tool config, `lib/tauri/command.ts`, event wrapper.
- **Rust files:** safe `save_activity_log` command or combined dialog/write command.
- **Dependencies:** T0/T5; integrate with T7.
- **Risks:** logging confidential document contents/paths/secrets; logging recursively failing operations.
- **Acceptance:** command/event duration/status visible; search/filter/clear/export work; redaction tests cover keys, emails, tokens, paths, nested objects, arrays, and truncation; logger failure never breaks the observed command.

### T11 — Align summary safety and export

- **Gap:** Feature parity exists, but raw path/write commands are too trusting.
- **Implementation:** Validate native grants/canonical paths, bounded file sets, typed progress/errors, atomic save.
- **React files:** `SummarizePage.tsx`, product API.
- **Rust files:** `commands/research.rs`, `services/research_service.rs`, `core/write_summary` path.
- **Dependencies:** T0/T1.
- **Risks:** symlink escape after selection, files changing during processing, 50 MB aggregate handling.
- **Acceptance:** native file/folder/subset summary and Markdown save remain equivalent; invalid/ungranted paths fail safely; skipped/limit feedback is actionable.

### T12 — Expose Helix search

- **Gap:** query builders are unreachable from frontend/integrations.
- **Implementation:** Port current repository/service wrappers; commands; safe minimal result DTO; tests.
- **React:** Product API methods; do not connect mock search UI without product decision.
- **Rust files:** `repository/document_repository.rs`, new/updated service and `commands/documents.rs`, `lib.rs`.
- **Dependencies:** T7.
- **Risks:** embedding dimension, unbounded result payload, user-scope bypass.
- **Acceptance:** validation rejects blank user/query/embedding and zero/large limit; results are user-partitioned; command contract test passes.

### T13 — Optional WM research clients

- **Gap:** Backend API capability absent.
- **Implementation:** Port service clients/config validation; commands for file extraction, index create/status, GraphRAG query; keep secrets in Rust.
- **React:** None unless a separate UI feature is approved.
- **Rust files:** new `core/clients/wm_ai_services.rs`, `commands/research.rs`, Cargo features/config/secret provider.
- **Dependencies:** T0/T1.
- **Risks:** service-specific auth, outbound data governance, remote timeout/error bodies, large multipart files.
- **Acceptance:** request-validation/unit tests ported; secrets never enter webview/logs; service failures return safe operation IDs.

### T14 — Performance, cleanup, and release verification

- **Gap:** single desktop bundle, Rust warnings, unused demos/components, incomplete production security checklist.
- **Implementation:** lazy routes, optional manual chunks for PDF stack, dev-gate demos, remove dead code after use audit, clippy/audit/build checks.
- **Files:** `App.tsx`, Vite config, Cargo/modules, CI config if added.
- **Dependencies:** all feature work.
- **Risks:** PDF worker chunk/CSP; removing a hidden desktop workflow.
- **Acceptance:** frontend build has route chunks; `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, frontend typecheck/tests/build, Tauri packaged debug build, and dependency audit pass or have approved exceptions.

## 10. Risks and architectural decisions

| Decision/risk | Why it matters | Recommendation |
|---|---|---|
| Credential committed to web example | A credential in Git history must be treated as compromised. | Rotate/revoke and purge before feature work; add secret scanning. |
| Shared React ownership | Manual copying will drift again. | Define a documented sync boundary or extract a versioned shared UI/domain package after parity stabilizes. Do not share transport clients. |
| Legacy investment thesis | Web removed it; desktop exposes it. Constraint says not to regress desktop-specific functionality. | Preserve data and optional supplemental UX, but remove it from required baseline contracts. |
| Deal root authority | Stored local paths enable valuable desktop workflows but create a privileged boundary. | Persist canonical chosen roots; use only relative descendants; provide explicit rebind. |
| Custom commands bypass plugin FS scopes | Capability JSON alone does not protect Rust code. | Validate every command input and limit sensitive commands to the main local window/origin. |
| Local secrets | SQLite/API DTO currently expose the full user key. | Migrate to OS credential storage and never return plaintext to React. |
| Helix/OpenAI availability | Startup or long jobs can fail offline. | Keep app usable; lazy initialize, time out, report retryable service errors, and retain job states. |
| Job cancellation | Closing/remounting UI should not corrupt graph state. | Separate backend job lifetime from component lifetime; retain terminal state; decide explicit cancellation semantics. |
| SQLite sync blocking | rusqlite is synchronous and guarded by `std::sync::Mutex`. | Use `spawn_blocking` for command work; never hold guard across await. |
| Large PDF/IPC payload | Preview returns base64 and can be expensive. | Keep existing size limit now; later consider Tauri asset protocol/streamed local URL with strict scoping. |
| Web baseline stubs | Some controls imply behavior that does not exist. | Reproduce visible state only where required; mark productization as separate scope. |
| Web browser-upload data room | Pseudo-root is not a reopenable server directory. | Do not copy this limitation; desktop should use the persisted native root. Document cross-platform semantic difference. |
| Default window vs responsive UI | Current 800 px window hides nav and clips multi-panel layouts. | Set a realistic default/minimum and implement compact nav/panel defaults. |
| Platform path text | Web upload modal says “Mac.” | Use OS-neutral copy or derive platform label in desktop. |
| Authentication | Email existence is not authentication or authorization. | Keep parity, but require a separate security/product project before multi-user/remote deployments. |

## 11. Acceptance criteria for declaring feature parity

Feature parity is reached when all user-facing web matrix rows are **Implemented / equivalent** or **Desktop implementation exists but differs appropriately**, and no P0/P1 row remains missing/partial without an approved product exception.

### Product workflows

- [ ] A returning user can enter by email, create a local profile, reopen Account Info, change theme, and receive equivalent errors/feedback.
- [ ] A user can create every supported deal type through a native folder workflow, select required source documents, extract key questions, and land in the current deal-room UI.
- [ ] Created deals survive page reload and app restart, appear in Active Deals, support direct deal/data-room navigation, and can be archived through the native backend contract.
- [ ] The stored deal folder powers tree browsing and preview after restart; unavailable folders show an actionable rebind error.
- [ ] PDF/DOCX/XLSX/PPTX preview retains page navigation, zoom, conversion feedback, and safe containment.
- [ ] Data-room upload accepts supported PDF/DOCX files, validates limits, processes independently, reports progress/terminal states, retries failures, and skips unchanged content for the same user.
- [ ] Summary file/folder/subset selection, markdown rendering, errors, and native save are equivalent to web outcomes.
- [ ] Logs capture redacted IPC/events and support counts, search, filters, detail expansion, clear, and native JSON export.
- [ ] Current Hub, shell, deal overview, key-question table, analyzed-file review controls, and timeline edit/time behavior match web at equivalent viewport sizes.
- [ ] Desktop-specific native menus/dialogs/local preview remain functional; the playground remains available only in approved development contexts.

### Data and service behavior

- [ ] Desktop SQLite migration works for a fresh DB and a pre-parity DB without destructive loss.
- [ ] Deal/list/archive and user commands return typed, stable DTOs.
- [ ] Document identity uses user ID + content hash; concurrent duplicate ingestion is serialized; incomplete graph writes are not treated as complete.
- [ ] Helix vector and keyword operations enforce user partition and result bounds.
- [ ] WM service commands, if retained in scope, keep credentials/data transfer in Rust and pass validation/error tests.
- [ ] No HTTP server is embedded in desktop solely for parity.

### Security and resilience

- [ ] The committed credential is rotated/revoked, removed from current files/history, and secret scanning is enabled.
- [ ] Tauri runs with a restrictive tested CSP and least-privilege capabilities.
- [ ] All filesystem commands reject absolute/ungranted traversal, `..`, and symlink escapes; reads/writes are size bounded.
- [ ] No full API key, service credential, authorization data, raw sensitive document body, or unrestricted local path enters React logs or serialized internal errors.
- [ ] Sensitive commands are restricted to the intended local window/origin.
- [ ] OpenAI/Helix/WM outages do not crash the app and produce safe retryable feedback with correlation IDs.

### Quality gates

- [ ] Frontend unit/component tests cover platform adapter contracts, route loading, deal restoration, upload state machine, log redaction/filtering, and timeline edits.
- [ ] Rust tests cover migrations, repositories, command validation, path containment/symlinks, job transitions, deduplication/concurrency, safe error serialization, and search user partitioning.
- [ ] `npm run build`, frontend tests/typecheck, `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, Tauri packaged debug build, and dependency/secret audits pass or have documented approved exceptions.
- [ ] Light/dark visual QA passes at minimum, default, and large desktop sizes, with keyboard-only and reduced-motion checks.
- [ ] No existing desktop-native capability is removed without an explicit approved replacement or development-only gate.

## 12. Quick-win shortlist

After T0/T2 establish contracts, these have high parity value with relatively low implementation risk:

1. Port lazy routes and the Logs route shell from web `App.tsx`.
2. Port `WorkspaceHeader`, `WorkspaceLayout`, sidebar components, `Icon` variants, and CSS tokens.
3. Add the Tauri-backed `useWorkspaceDeals` hook after list/get commands exist.
4. Port the new Hub/Suggested Content because it is platform-neutral and mostly static UI.
5. Port Deal Room header/summary/analyzed-file/timeline components because their current baseline state is React-local.
6. Adapt web `activityLog.ts` around the existing desktop `execute` and event wrappers.
7. Wire “Upload New File” to an adapted modal once the validated job command is ready.

These should be implemented as reviewed ports, not parallel re-creations, to minimize another round of UI drift.
