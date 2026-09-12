# Data Room state decomposition plan

Status: proposed

Revalidated: 2026-09-11 against the live Quarry working tree

Scope: shared React/Vite Data Room route composition, feature-local async state, document-preview
state, viewer/search coordination, focused frontend tests, and the corresponding architecture
ownership narrative

Primary files:

- `frontend/src/pages/DataRoomPage.tsx`
- `frontend/src/components/data-room/DataRoomWorkspace.tsx` (new)
- `frontend/src/hooks/useDataRoomContents.ts` (new)
- `frontend/src/hooks/useDocumentSession.ts` (new)
- `frontend/src/data/dataRoom.ts`
- `frontend/src/data/dataRoomPreview.ts`
- `frontend/tests/pages/DataRoomPage.test.tsx`
- `frontend/tests/components/data-room/DataRoomWorkspace.test.tsx` (new)
- `frontend/tests/hooks/useDataRoomContents.test.tsx` (new)
- `frontend/tests/hooks/useDocumentSession.test.tsx` (new)
- `docs/ARCHITECTURE.md`

## Outcome

Reduce `DataRoomPage` from a route component that directly owns several unrelated state machines
to a small route-level orchestrator with three explicit boundaries:

1. `useDataRoomContents(dealId)` owns loading and refreshing the local and stored document
   sources, stale-response protection, source errors, and derived explorer/review data.
2. `useDocumentSession(dealId)` owns the selected document, preview request, raw-text request,
   request invalidation, and document close lifecycle.
3. `DataRoomWorkspace` owns the interactive Data Room surface: explorer, file-review table,
   preview panel, arc menu, search/viewer coordination, and empty/unavailable presentation.

Keep route/deal lookup, workspace session lookup, and modal visibility in `DataRoomPage`. Do not add
a context provider: the state has one clear feature owner and is passed only across the immediate
Data Room composition boundary. A provider would make dependencies implicit without eliminating
meaningful state or prop depth.

The implementation should preserve current product behavior, API contracts, route state,
web/desktop composition, and visual structure. It should make one intentional lifecycle fix:
closing a document must release the retained preview bytes and raw text instead of leaving them in
page state until another selection or route change.

## Current-state findings

- `DataRoomPage.tsx` is 492 lines and owns route lookup, two document-source requests, request race
  protection, data mapping, document preview loading, raw-text loading, PDF/search coordination,
  modal state, error/empty presentation, and the complete workspace layout.
- All current imports, state values, and refs are compiler-used. `npm run typecheck:web` passes with
  `noUnusedLocals` and `noUnusedParameters`; the problem is ownership and lifecycle clarity, not
  syntactically dead variables.
- `localDataRoom`, `dealDocuments`, two error strings, two loading flags, a refresh counter, and two
  request refs collectively implement one Data Room contents lifecycle.
- `selectedDocument`, `preview`, `rawText`, and two request refs collectively implement one document
  session but can currently form stale combinations. In particular, `handleClosePreview` clears the
  selection without clearing the `DocumentPreviewResponse` or `DealDocumentText`, so potentially
  large PDF bytes and raw text remain retained while no preview is visible.
- `dataRoomRefreshVersion` is event-like state used only to retrigger the contents effect. A named
  `reloadAll()` operation communicates the behavior more directly.
- Page count, requested page, preview focus, search visibility, and the portal container are a
  legitimate UI bridge between `DocumentPreviewPanel` and `DataRoomArcMenu`; they are not domain or
  route state.
- `isDocumentPreviewResponse`, `buildStoredDocumentNodes`, and `storedDocumentKind` are pure data
  helpers currently embedded in the route component.
- The existing page test covers only arc-menu persistence and the portal layer across one local-file
  selection. It does not characterize source loading/error/retry behavior, stale async responses,
  stored-document preview/raw text, deal changes, or close cleanup.
- The current aggregate source policy is all-or-nothing after loading completes: an error from
  either source makes the main Data Room unavailable even if the other source returned files. This
  plan preserves that policy during the structural refactor. Partial-success UX is a separate
  product decision and must not be smuggled into a state-only change.

## Stable contracts and assumptions

- `DataRoomPage` remains exported from `frontend/src/pages/DataRoomPage.tsx` and mounted at
  `/hub/deals/:dealId/data-room` through the existing lazy route boundary.
- Web continues to use `BrowserRouter`; desktop continues to use `HashRouter`. No router, Vite,
  TypeScript-alias, or runtime-selection changes are needed.
- Shared frontend code continues to call the transport-neutral `@quarry/runtime` API. Do not add
  raw `fetch`, `EventSource`, or `@tauri-apps/*` imports.
- Preserve the existing `QuarryApi` methods and response shapes:
  `listDealDataRoom`, `listDealDocuments`, `previewDealDocument`, `getDealDocumentPdf`, and
  `getDealDocumentText`.
- Preserve the current merge order: stored documents appear under the synthetic `Saved documents`
  folder before local-tree nodes.
- Preserve the current mapping of PDF, spreadsheet, and other stored filename extensions to
  `DataRoomTreeNode.kind`.
- Preserve the current unconfigured-local-root behavior: the recognized error is normalized to an
  available empty local tree rather than displayed as a failure.
- Preserve same-deal refresh behavior: a stored-document refresh may retain the previous stored
  list while loading; changing `dealId` must not display the previous deal's documents.
- Preserve the current all-or-nothing aggregate error behavior until a separate partial-success
  design is approved.
- Preserve document-search behavior: the arc menu stays mounted while switching between the file
  table and preview, selection/close closes search, page results target the current preview, and
  focus returns to the viewer when appropriate.
- Preserve upload behavior: closing the upload modal triggers a stored-document refresh. Do not
  infer upload success from close; changing the modal callback contract is outside this refactor.
- Preserve SharePoint modal behavior and do not imply that its submission is implemented.
- Preserve accessibility, keyboard/focus behavior, loading/error feedback, theme tokens, and the
  page's current responsive layout.
- Do not add a state-management/query-cache dependency or alter `package.json` or
  `package-lock.json`.

## Target ownership and APIs

### `useDataRoomContents(dealId)`

Create a feature-specific hook under `frontend/src/hooks/`. It owns both source requests because
the UI intentionally merges their results into one explorer and review table.

Use discriminated source states rather than independent data/loading/error variables. The exact
names may follow nearby style, but the state must make loading, ready, and error mutually
exclusive while allowing the stored source to retain same-deal data during refresh:

```ts
type DataRoomResource<T> =
  | { status: "loading"; previous?: T }
  | { status: "ready"; value: T }
  | { status: "error"; message: string; previous?: T };

type DataRoomContentsState = {
  dealId?: string;
  local: DataRoomResource<DealDataRoom>;
  stored: DataRoomResource<DealDocumentSummary[]>;
};
```

Use a feature-local reducer if it makes deal changes, request start, success, failure, and refresh
transitions explicit. Do not create a generic repository-wide async reducer before another feature
demonstrates the same requirements.

The hook should return a narrow view model rather than expose its dispatch function:

```ts
type DataRoomContents = {
  errorMessage: string;
  explorerNodes: DataRoomTreeNode[];
  hasFiles: boolean;
  isEmpty: boolean;
  isLoading: boolean;
  isUnavailable: boolean;
  reloadAll: () => void;
  reloadStored: () => void;
  reviewFiles: DataRoomFileEntry[];
  rootPath?: string;
};
```

All booleans in the returned view model must be derived from the discriminated source states; do
not store another layer of synchronized flags. Preserve request sequencing or cancellation inside
the hook so an older deal/source response cannot overwrite a newer one.

Move `buildStoredDocumentNodes` and its filename-kind helper into `frontend/src/data/dataRoom.ts`.
Keep them pure and add focused mapper tests if they are not already exercised through the hook.

### `useDocumentSession(dealId)`

Create a second feature-specific hook that models whether a document is closed or open. Reuse the
existing `PreviewState` and `RawTextState` unions initially; relocating those types to
`dataRoomPreview.ts` is optional only if it improves ownership without creating import churn.

```ts
type DocumentSessionState =
  | { status: "closed" }
  | {
      status: "open";
      document: DataRoomTreeNode;
      preview: PreviewState;
      rawText: RawTextState;
    };

type DocumentSessionController = {
  closeDocument: () => void;
  requestRawText: () => Promise<void>;
  selectDocument: (document: DataRoomTreeNode) => Promise<void>;
  state: DocumentSessionState;
};
```

The hook owns preview/raw-text request IDs or equivalent cancellation. Its transitions must ensure:

- selection immediately opens the new document in preview-loading state and invalidates older
  preview/raw-text requests;
- stored documents load PDF bytes through `getDealDocumentPdf` and local documents load through
  `previewDealDocument`;
- invalid selections resolve to the current preview-error presentation;
- raw text is requested only for the current stored document;
- an older request cannot update a newer document session;
- closing invalidates outstanding work and transitions to `{ status: "closed" }`, releasing PDF
  bytes and raw text;
- a `dealId` change closes and invalidates the previous session.

Move `isDocumentPreviewResponse` to `frontend/src/data/dataRoomPreview.ts` so runtime response
validation is not owned by the page or presentation component.

### `DataRoomWorkspace`

Create `frontend/src/components/data-room/DataRoomWorkspace.tsx` as the owner of the complete
interactive Data Room surface. It should render the existing `DataRoomExplorer`, main pane,
file-review table, preview panel, search portal, arc menu, and empty/unavailable states.

It receives route-resolved display values, the contents view model, and modal-opening callbacks:

```ts
type DataRoomWorkspaceProps = {
  contents: DataRoomContents;
  dealId: string;
  dealName: string;
  dealRoomPath: string;
  email?: string;
  navigationState?: DealExtractionLocationState;
  onConnectToSharePoint: () => void;
  onUploadFiles: () => void;
};
```

The component calls `useDocumentSession(dealId)` and locally owns:

- `documentPageCount`;
- `documentSearchOpen`;
- `requestedPreviewPage`;
- the search portal callback-ref state;
- `documentPreviewRef` and focus bridging.

Wrap selection and close in workspace handlers that reset page count, search visibility, and the
requested page before delegating to the document-session hook. This preserves the current
transition behavior without an effect that synchronizes UI state after a document prop changes.

Mount `DataRoomWorkspace` with `key={deal.room.id}` from the page. A deal route change then resets
all feature-local viewer/search state as one lifecycle, while file selection within the same deal
does not remount the workspace or arc menu.

Keep `EmptyDataRoomState` and `UnavailableDataRoomState` local to this component unless independent
reuse emerges. Moving them solely to reduce a line count would not improve ownership.

### `DataRoomPage`

After extraction, `DataRoomPage` should own only:

- route params and location state;
- workspace deals/session lookup and missing-deal redirect/skeleton;
- `useDataRoomContents(dealId)`;
- one modal state, preferably `"none" | "upload" | "sharepoint"` so both dialogs cannot be open;
- upload-close behavior that closes the modal and calls `contents.reloadStored()`;
- rendering the keyed `DataRoomWorkspace` and active modal.

Do not introduce a provider. Reconsider context only if a future change creates multiple deeply
nested, independently mounted consumers that all require the same session controller and explicit
props become materially harder to follow.

## Implementation sequence

### Phase 1 — Add characterization coverage before moving ownership

1. Re-run `git status --short` and preserve all existing tracked/untracked work, especially the
   active edits in `DataRoomPage.tsx`, `DocumentPreviewPanel.tsx`, API adapters, contracts, tests,
   manifests, and architecture documentation.
2. Extend `frontend/tests/pages/DataRoomPage.test.tsx` against the current component boundary to
   characterize observable behavior rather than internal state variables.
3. Cover the initial merged local/stored tree, recognized unconfigured-local behavior, aggregate
   source error presentation, retry, local preview selection, stored preview selection/raw text,
   and deal-route replacement while a request is outstanding where practical.
4. Add a regression test demonstrating that closing a selected document removes the preview UI.
   The later hook test will verify that the retained response state itself is released.
5. Keep test data synthetic and do not include real emails, local paths, documents, or API keys.

Exit criteria:

- Existing behavior that the refactor must preserve is represented by focused tests.
- The arc-menu persistence test remains meaningful and is not weakened to implementation-detail
  assertions.
- Any existing behavior that cannot be characterized without invasive mocking is listed before
  extraction rather than silently assumed.

### Phase 2 — Extract pure Data Room mapping and validation helpers

1. Move `buildStoredDocumentNodes` and the extension-to-kind helper from the page into
   `frontend/src/data/dataRoom.ts` with names that describe their domain role.
2. Move `isDocumentPreviewResponse` into `frontend/src/data/dataRoomPreview.ts`.
3. Add focused tests for stored folder identity/order, PDF/spreadsheet/default kind mapping, and
   valid byte/base64 versus invalid preview responses.
4. Update page imports without changing request or rendering behavior.
5. Run the new data tests and the existing Data Room page/component tests.

Exit criteria:

- The page contains no pure transport-validation or tree-mapping implementation.
- Merged node identities, ordering, labels, and file kinds are unchanged.
- Preview validation accepts and rejects the same response shapes as before.

### Phase 3 — Extract `useDataRoomContents`

1. Implement the discriminated source state and reducer/transition logic in
   `frontend/src/hooks/useDataRoomContents.ts`.
2. Move both list operations, recognized unconfigured-root normalization, request invalidation,
   same-deal stored refresh behavior, full retry behavior, and derived node/file calculations into
   the hook.
3. Replace `dataRoomRefreshVersion` with `reloadAll()` and the upload-close direct loader call with
   `reloadStored()`.
4. Keep the current aggregate all-or-nothing error policy. If tests reveal ambiguity, document it
   as a follow-up rather than changing the user-visible state during extraction.
5. Add focused hook tests using `renderHook` from the installed Testing Library package and mocked
   `@quarry/runtime`. Cover successful merge, loading, unconfigured local root, each failure,
   stale responses after deal change, `reloadAll`, and `reloadStored`.
6. Re-run the page tests to prove that moving request ownership did not alter the route UI.

Exit criteria:

- `DataRoomPage` no longer owns local/stored response state, error/loading flags, request IDs, or
  a refresh counter.
- No response from an older request/deal can replace current hook state.
- No API contract, adapter, Tauri, or backend change is introduced.

### Phase 4 — Extract `useDocumentSession`

1. Implement the closed/open document-session union and request transitions in
   `frontend/src/hooks/useDocumentSession.ts`.
2. Move local/stored preview loading, preview-response validation, raw-text loading, and request
   invalidation into the hook.
3. Ensure `closeDocument()` and deal changes invalidate both request streams and remove all
   retained `DocumentPreviewResponse` and `DealDocumentText` values from hook state.
4. Add focused hook tests for local preview success/error, stored PDF mapping, invalid response,
   raw-text success/error/unavailable, selection races, close races, and deal changes.
5. Temporarily adapt `DataRoomPage` to consume the hook directly if needed, keeping each
   intermediate commit/build functional.

Exit criteria:

- Selected document, preview, and raw text cannot exist as unrelated page states.
- Closing a preview produces exactly `{ status: "closed" }` and stale async completions are ignored.
- `DocumentPreviewPanel` remains a typed display/interaction component and does not import the
  runtime API.

### Phase 5 — Introduce `DataRoomWorkspace` and simplify the page

1. Move the explorer/main-pane composition, empty/unavailable presentation, file-review/preview
   switch, search portal, and arc menu into `DataRoomWorkspace`.
2. Move document-session use and viewer/search bridge state into the workspace.
3. Preserve the existing arc-menu mount across same-deal document selections. Do not key the
   preview panel differently from current behavior unless a test demonstrates the need.
4. Key the workspace by deal ID at the page boundary so a new deal receives a clean feature
   lifecycle.
5. Replace the two modal booleans with one local modal union if doing so remains a small direct
   simplification. Do not create a modal context or general modal manager.
6. Keep the page test focused on route/deal/modal orchestration. Move explorer/preview/search
   interaction coverage to `DataRoomWorkspace.test.tsx` rather than retaining a heavily mocked
   page test that duplicates the component boundary.
7. Inspect the final `DataRoomPage.tsx`: every remaining hook and handler should have a route-level
   or modal-level responsibility.

Exit criteria:

- `DataRoomPage` is a readable route orchestrator rather than the owner of feature interaction
  state.
- `DataRoomWorkspace` owns only Data Room UI coordination; API request state remains in focused
  hooks.
- There is no new context/provider, global store, query library, dependency, or lockfile change.
- Existing visible states and navigation behavior remain unchanged except for releasing closed
  preview payloads.

### Phase 6 — Runtime, regression, and architecture verification

1. Run focused Data Room data, hook, workspace, preview-panel, and page tests.
2. Run the shared frontend verification ladder from `frontend/`:

   ```sh
   npm run typecheck
   npm run check:boundaries
   npm test
   npm run build:web
   npm run check:web-bundle
   npm run build:desktop-ui
   ```

3. Inspect web mode with `npm run dev:web` across:
   - initial Data Room loading;
   - populated merged local/stored documents;
   - empty and unavailable states;
   - retry;
   - upload modal open/close and stored-list refresh;
   - SharePoint modal open/close;
   - local and stored document preview;
   - raw-text loading/success/error;
   - search open/close, result page jump, focus restoration, and preview close;
   - navigation between two deal IDs with requests in flight;
   - keyboard operation, light/dark themes, reduced motion, and narrow/wide layouts.
4. Inspect desktop UI mode with `npm run dev:desktop-ui` for the shared composition. Use the full
   Tauri runtime only if a platform-specific preview/IPC regression cannot be assessed with the UI
   build and existing adapter/native tests; no Tauri Rust change is planned.
5. Re-read the frontend structure, state ownership, Data Room maturity, API, and verification
   sections of `docs/ARCHITECTURE.md` after implementation.
6. Update the state-ownership narrative from `DataRoomPage` ownership to
   `useDataRoomContents`/`useDocumentSession`/`DataRoomWorkspace` ownership. Keep the document-search
   behavior and current product limitations accurate.
7. Run `git diff --check`, inspect the final diff for generated `dist`, secrets, unrelated edits,
   and package/lockfile churn, then re-run `git status --short`.

No backend or Tauri Cargo gate is required unless implementation expands into those build roots.
No API adapter tests are required solely for relocating unchanged calls behind hooks, but the
existing adapter suites must continue to pass as part of the full frontend test run.

## Test matrix

| Scenario | Expected behavior |
| --- | --- |
| Initial route load | Local and stored sources load; stale data from another deal is not shown |
| Both sources succeed | Stored folder precedes local nodes; explorer and review table share the same derived nodes |
| Local root unconfigured | Local source is treated as available-empty; stored documents remain usable |
| One source fails | Existing aggregate unavailable behavior is preserved during this refactor |
| Retry | Both sources receive a new request; stale prior completions cannot overwrite the retry |
| Upload modal closes | Modal closes and only the stored-document source refreshes |
| Local file selected | Preview uses `previewDealDocument`; viewer shell loads and then displays the response |
| Stored file selected | Preview uses `getDealDocumentPdf`; raw text uses `getDealDocumentText` on demand |
| Invalid preview response | Current sanitized preview error is displayed |
| Selection race | A slower earlier preview/raw-text response cannot overwrite the latest selection |
| Preview closed | Search/viewer bridge resets; session becomes closed; PDF bytes and raw text are released |
| Deal route changes | Contents and document session reset; old requests cannot update the new deal |
| Search result activated | Available PDF-page target navigates the current viewer and clears the request after handling |
| Arc menu across selection | Arc menu instance remains mounted within the same deal |
| Missing deal | Existing skeleton-then-redirect behavior remains intact |

## Risks and mitigations

- **A refactor silently changes partial-failure behavior.** Characterize the existing aggregate
  policy first and preserve it. Design partial-success warnings separately.
- **Effects recreate stale request races.** Keep request identity/cancellation inside each owning
  hook and test out-of-order completions directly.
- **The workspace extraction remounts the arc menu.** Key only by deal ID, not by selected document,
  and retain the persistence test.
- **A hook merely hides a large bag of state.** Return a narrow typed controller/view model and keep
  transition logic inside the hook; do not export setters or reducer dispatch.
- **Context is added for convenience.** Keep explicit props at the page/workspace boundary. Add a
  provider only after concrete independent consumers demonstrate a real need.
- **Reducer code becomes more complex than the original flags.** Use one feature-specific reducer
  per lifecycle and avoid a generic async framework. If a reducer transition cannot be explained
  by an observable event, simplify it.
- **Tests become coupled to implementation details.** Assert rendered states, API calls, and public
  hook transitions; do not assert reducer action names or private state layout.
- **The dirty tree is accidentally broadened.** Review each touched file against the pre-existing
  status and avoid formatting or rewriting adjacent active work.

## Out of scope

- A global state library, query-cache library, or React context provider.
- Changes to Quarry API methods, HTTP/Tauri adapters, Axum routes, DTOs, persistence, or Tauri
  capabilities.
- Partial-success source UX, warning banners, or changing the current all-or-nothing error policy.
- Upload-modal success/cancel contract redesign.
- SharePoint submission or import behavior.
- Replacing the fixture-backed document search or file-review findings.
- Persisting Synthesis Canvas state.
- PDF viewer internals, virtualization, highlighting, or cross-document search/navigation.
- Broad page/component folder reorganization or unrelated styling cleanup.

## Acceptance criteria

- `DataRoomPage` owns route/deal/session lookup, contents-hook composition, and one modal state; it
  no longer owns document-source requests, preview/raw-text requests, or viewer/search state.
- `useDataRoomContents` owns both source lifecycles and exposes derived, read-only Data Room content
  plus explicit `reloadAll` and `reloadStored` operations.
- `useDocumentSession` represents closed/open state explicitly and releases preview bytes/raw text
  on close or deal change.
- `DataRoomWorkspace` owns explorer, main pane, document session, and viewer/search coordination
  without remounting the arc menu on same-deal selection.
- Pure node mapping and preview validation no longer live in the page.
- No provider, global store, new dependency, package change, API change, backend change, or native
  change is introduced.
- Focused tests cover source transitions, request races, close cleanup, workspace coordination,
  and route behavior; existing Data Room and PDF-viewer tests remain passing.
- Both TypeScript targets, boundary checks, full frontend tests, web build/bundle inspection, and
  desktop UI build pass.
- Manual inspection covers meaningful Data Room states and focus/keyboard behavior in web and the
  shared desktop UI composition.
- `docs/ARCHITECTURE.md` accurately describes the new feature-state owners while preserving the
  current Data Room maturity and limitation statements.
- `git diff --check` is clean, generated output is absent, and final status review distinguishes
  this refactor from pre-existing user work.

## Architecture impact

Creating this plan does not change the implemented architecture. Implementing it will change the
documented frontend state owner from `DataRoomPage` to two feature hooks plus
`DataRoomWorkspace`, so `docs/ARCHITECTURE.md` must be updated in the implementation change. The
runtime split, route, API contract, data ownership, security boundary, and feature maturity remain
unchanged.
