# Quarry frontend structure and Vite chunk-size warning plan

Status: proposed

Revalidated: 2026-09-13 against clean Quarry commit `b7815b2`

Scope: shared React/Vite route composition, URL-backed Deal Room navigation, workspace and
feature-state ownership, interaction-level code splitting, bundle measurement, and frontend
bundle-size regression checks

Inputs incorporated by this revision:

- the 2026-09-13 frontend structural review covering routing, page ownership, workspace-shell
  responsibilities, and fixture fallback;
- the current 492-line Data Room and 529-line Summarize route components;
- [`data-room-state-decomposition-plan.md`](data-room-state-decomposition-plan.md), whose detailed
  async-state extraction remains useful but must be rebased onto the nested Deal Room route;
- [`diligence-canvas-template-editor-integration-plan.md`](diligence-canvas-template-editor-integration-plan.md),
  which proposes a new template-editor route and substantial Plate/canvas code that must remain
  outside the initial and ordinary Deal Room request graphs;
- the existing 2026-09-12 web and desktop production-build measurements.

Primary files and boundaries:

- `frontend/src/App.tsx`
- `frontend/src/app/` (new route/provider composition; exact filenames follow the live seam)
- `frontend/src/pages/DealRoomPage.tsx`
- `frontend/src/pages/DataRoomPage.tsx`
- `frontend/src/pages/SummarizePage.tsx`
- `frontend/src/components/hub/WorkspaceHomeShell.tsx`
- `frontend/src/components/hub/WorkspaceLayout.tsx`
- `frontend/src/components/hub/WorkspaceSidebar.tsx`
- `frontend/src/components/hub/sidebar/DealRoomWorkspaceSidebar.tsx`
- `frontend/src/components/data-room/DocumentPreviewPanel.tsx`
- `frontend/src/components/data-room/` and `frontend/src/hooks/` Data Room owners
- `frontend/src/components/summarize/`, `frontend/src/hooks/`, and `frontend/src/data/` Summarize
  owners
- `frontend/src/components/deal-room/` route views and template-editor orchestration
- `frontend/src/hooks/useWorkspaceDeals.ts`
- `frontend/src/data/workspace.ts`
- `frontend/tests/`
- `frontend/vite.config.ts`
- `frontend/package.json`
- `frontend/scripts/`
- `docs/ARCHITECTURE.md`

## Outcome

Remove Vite's oversized JavaScript chunk warning in both production UI targets by aligning the
frontend architecture with real navigation and interaction boundaries. The work should reduce
when code is requested, not silence the warning or merely rearrange the same startup bytes.

The finished structure should:

- keep `LoginPage` and the minimal router/theme/transition shell eager;
- lazy-load every workspace route, including Hub, Account, the Deal Room layout, and its feature
  routes;
- replace Deal Room's page-local `activeDealView` router with nested URL routes and a shared
  `Outlet`-based shell/sidebar;
- use path state for durable feature navigation, query parameters only for safe and genuinely
  shareable view state, and React state only for transient UI or unsaved edits;
- keep workspace data/session ownership outside reusable layout primitives;
- make API loading, error, retry, server-data, and explicitly enabled demo-data states
  distinguishable instead of silently converting a failed deal request into fixture success;
- reduce Data Room and Summarize pages to route-level composition while preserving focused owners
  for their request and interaction lifecycles;
- restore an interaction boundary around `DocumentPreviewPanel` and defer the Markdown renderer
  until Summarize has content to render;
- keep the proposed Diligence Canvas editor and its Plate dependencies behind the selected-template
  route and an additional implementation boundary;
- produce warning-free web and desktop UI builds without increasing Vite's warning threshold;
- add a repository-native guard for the initial entry and every emitted application chunk;
- preserve the shared React tree, web `BrowserRouter`, desktop `HashRouter`, runtime aliases, API
  boundaries, route state needed for session/extraction continuity, accessibility, focus,
  reduced-motion behavior, and existing async feedback.

## Why the broader structure work belongs in this plan

The structural findings can materially improve the request graph, but only when they create
dynamic-import boundaries:

| Change | Bundle effect |
| --- | --- |
| Make Deal Activity, Analysis, Deliverables, Templates, Data Room, and the template editor real child routes | Gives Vite stable, user-visible route boundaries and prevents the Deal Room overview from statically owning every view |
| Move feature orchestration out of large page files | Makes ownership and testing clearer; has no byte effect by itself |
| Dynamically import route modules and expensive inactive panels after extraction | Changes download timing and can reduce oversized chunks |
| Split `WorkspaceHomeShell` into data/provider and layout responsibilities | Lets Login avoid workspace data, fixture, sidebar, and feature graphs; file movement alone is not sufficient |
| Replace fixture fallback with an explicit data mode | Prevents production/API error paths from retaining fixture imports and makes server failure visible; savings must still be measured |
| Lazy-load document preview, Markdown rendering, and Diligence Canvas | Keeps `react-pdf`/pdf.js, Markdown, Plate, and canvas code behind the interactions that need them |

The implementation must therefore pair every proposed structural boundary with a module-graph and
cache-disabled network check. A smaller source file, a new folder, or an `Outlet` is not accepted as
a bundle improvement without changed emitted/requested code.

## Current evidence and baseline drift

The warning was reproduced in both targets on 2026-09-12. The installed lockfile resolves Vite
7.3.6 even though `package.json` declares `^7.0.4`.

The 2026-09-13 read-only structural review also reported passing web and desktop typechecks,
runtime-boundary checks, and all 35 test files/106 tests. Those checks establish that the reviewed
structure was healthy before this plan revision; they do not replace the implementation gates.

| Target | Command | Initial entry | Gzip | Data Room chunk | Gzip | Result |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Web | `npm run build:web` | 859.59 kB | 264.34 kB | 671.43 kB | 202.55 kB | Passes with two chunks over 500 kB |
| Desktop UI | `npm run build:desktop-ui` | 858.41 kB | 264.01 kB | 671.43 kB | 202.55 kB | Passes with the same warning |

Other emitted artifacts in both builds:

| Artifact | Minified size | Interpretation |
| --- | ---: | --- |
| PDF worker asset | 1,046.21 kB | Separate `.mjs` worker emitted for PDF preview; not a Rollup application chunk |
| Shared Markdown-related chunk | 156.78 kB | Existing shared lazy dependency chunk; currently requested when Summarize loads even before a summary exists |
| Deals Kanban chunk | 95.09 kB | Already loaded by an interaction-level dynamic import |
| Deals route chunk | 31.43 kB web / 31.94 kB desktop | Route-lazy before the Kanban boundary |
| Global Vault route chunk | 4.17 kB | Existing route boundary |
| Vault route chunk | 5.90 kB | Existing route boundary |
| Explore route chunk | 12.46 kB | Existing route boundary, still implemented by `SummarizePage` |
| Logs route chunk | 9.40 kB | Existing route boundary |

The live source still has the following owners and constraints:

- `App.tsx` has four eager pages—Login, Hub, Account, and Deal Room—and wraps all route changes in
  one pathname-keyed React 19 `ViewTransition`.
- `LazyPage` already renders `WorkspacePageSkeleton` with accessible labels and enter/exit
  transitions. There is no blank-fallback problem to fix.
- `DealRoomPage` uses an `activeDealView` state machine for Deal Activity and Analysis while also
  serving three direct URL paths for overview, Deliverables, and Templates. The sidebar therefore
  mixes `NavLink` navigation with callbacks and `location.state.dealView`.
- `DealRoomPage` statically imports the timeline, deliverables, template gallery, template store,
  data-grid-backed overview, and placeholder views, putting them in one eager entry graph.
- `DataRoomPage` is 492 lines and directly owns deal lookup, two content-source requests, request
  invalidation, preview/text requests, search/viewer coordination, upload/modal state, and layout.
- `DataRoomPage` statically imports `DocumentPreviewPanel`; the panel owns the full `react-pdf`
  viewer graph and the separate pdf.js worker.
- `SummarizePage` is 529 lines and owns file selection, folder-tree construction, selected-file
  state, request lifecycle, Markdown rendering, export, and its view. Static `react-markdown` and
  `remark-gfm` imports make the renderer part of the route's first request.
- `WorkspaceHomeShell` combines workspace session/deal loading, context, fixture-backed navigation,
  sidebar construction, and layout.
- `useWorkspaceDeals` starts with fixtures, merges fixtures with a successful server result, and
  silently restores fixtures on API failure. Callers receive only `{ deals, loaded }`, so they
  cannot distinguish server, demo, and error states.
- Existing reusable `WorkspaceLayout`, `WorkspaceSidebar`, and `WorkspaceHeader` primitives are
  sound starting points; the plan should separate their data ownership rather than replace them.
- Frontend tests live under `frontend/tests/`. No new test belongs under `frontend/src/`.
- `check:web-bundle` verifies that Tauri runtime code is absent from `dist`; it is not a size check
  and must keep that narrow responsibility.
- `frontend/vite.config.ts` currently has no `manualChunks`, build manifest, or custom warning
  limit. That is the correct starting point.

### Changes added after the 2026-09-12 bundle baseline

No production frontend source changed between the plan commit `6292aa5` and the revalidated
`b7815b2` working tree, so the prior byte measurements remain the current implementation baseline
until Phase 0 rebuilds them.

One new tracked integration plan does affect the target architecture:

- `/hub/deals/:dealId/deliverables/templates/:templateId` will become a durable editor route;
- the route will add a controlled Diligence Canvas copy plus exact Plate dependencies;
- the small `DeliverableTemplateCanvasView` orchestrator may load with the route, but the canvas,
  Plate editor, and SVG implementation must remain dynamically imported;
- Login, ordinary Deal Room, Deliverables, and the template gallery must not request the editor
  implementation;
- the final size baseline and checked-in budgets must be captured after any Canvas/Plate work that
  lands in the same implementation window.

Between `6292aa5` and the current tree, the other implementation changes are backend-only
Diligence Studio client refinements. They do not change the current frontend bundle and are not
part of this plan's implementation scope.

## Target frontend shape

This is an ownership target, not a mandate to move every existing file:

```text
src/
  app/
    AppRoutes.tsx
    WorkspaceProvider.tsx
    RouteTransitionOutlet.tsx

  pages/
    LoginPage.tsx
    HubPage.tsx
    AccountPage.tsx

  components/
    ui/
    layout/
    brand/
    hub/
    deal-room/
    data-room/
    summarize/

  hooks/
    useWorkspaceDeals.ts
    useDataRoomContents.ts
    useDocumentSession.ts
    useSummarizeWorkflow.ts

  data/
    workspace.ts
    dataRoom.ts
    dataRoomPreview.ts
    summarize.ts

  fixtures/
  api/
  contracts/
  platform/
  lib/
```

Feature ownership should become explicit through small route modules, feature hooks, and view
components. Do not broadly relocate stable components into a new `features/` hierarchy just to
match an aspirational tree; moves are justified only when they establish an owner or lazy boundary.

### Target route hierarchy

```text
/login

/hub                                  shared workspace data/session provider
  home (default)                      workspace home at /hub
  account
  vault
  initiatives/vault
  summarize
  logs
  deals
  deals/:dealId                       shared Deal Room shell/sidebar and typed outlet context
    overview (default)                Deal Room overview at /hub/deals/:dealId
    activity                          Deal Activity timeline
    data-room                         Data Room
    analysis                          current Analysis placeholder
    deliverables                      Deliverables
      templates                       template gallery
        :templateId                   planned Diligence Canvas editor
```

Route helpers in `data/workspace.ts` remain the single construction point and must encode each
caller-controlled segment. Sidebar active state should derive from route matches/pathnames, not a
second page-local router.

Do not create routes for currently unreachable `diligence-graph` or Deal Room
`synthesis-canvas` branches merely to preserve dead state variants. Characterize whether a live
control can reach them; either remove unreachable composition during the route conversion or add a
route only when a real navigation contract is approved.

Path/query/local-state policy:

- paths own durable feature identity and Back/Forward history;
- query parameters may own filters, tabs, or opaque selected IDs only when refresh/deep-link
  behavior is intended;
- never place local filesystem paths, document content, emails, preview bytes, or other sensitive
  values in URLs;
- router state may continue to carry non-authoritative workspace email and fresh extraction
  results for compatibility, but must not select a durable Deal Room view;
- React state owns open dialogs, pending operations, unsaved editor drafts, focus bridges, and
  other transient interaction state.

## Contracts that must remain stable

- Preserve every currently reachable route and redirect while adding `/activity` and `/analysis`.
  Keep compatibility redirects for any previously emitted in-app URL that changes.
- Web continues to use `BrowserRouter`; desktop continues to use `HashRouter`.
- `frontend/vite.config.ts`, `tsconfig.web.json`, and `tsconfig.desktop.json` keep matching
  `@quarry/router` and `@quarry/runtime` aliases.
- Shared UI continues to import `@quarry/runtime`; raw Tauri imports remain isolated to
  `frontend/src/platform/runtime.desktop.ts`, and raw HTTP/SSE remains under `src/api/`.
- Preserve the current route-transition name, visible workspace skeleton treatment, and
  reduced-motion behavior. The keyed transition may move to the leaf outlet so providers and
  sidebars can stay mounted, but the visible navigation contract must remain equivalent.
- Preserve workspace email, fresh extraction results, SOW source name, deal selection, redirects,
  and direct-link behavior across lazy boundaries.
- Preserve extracted-deal precedence over persisted deals for a newly created deal. The routing
  refactor must not turn route state into authentication, authorization, or durable persistence.
- Preserve Deal Room overview tabs, timeline editing behavior, template catalog request-key reset
  semantics, header modes, loading/missing-deal behavior, and route-backed template import/delete/
  retry feedback.
- Preserve Data Room local/stored source behavior unless an explicitly documented state phase
  changes it; retain selection, preview, text/PDF loading and error behavior, page counts, focus,
  upload jobs, File Review, search, Arc Menu, and SharePoint-modal behavior.
- Preserve Summarize manual/file/folder inputs, selected-file validation, upload/path API mapping,
  Markdown output, save behavior, error state, and Chat/Summary selection.
- A lazy-module failure remains distinct from route-data or API failure. Never convert it into demo
  fixtures, an empty success screen, or an unrelated API error.
- Direct navigation and refresh keep working for both router targets under their existing hosting
  rules.
- Preserve the current React 19 canary, Vite/npm workflow, Tailwind setup, and current dependency
  set except for separately authorized dependencies required by the Diligence Canvas plan.

## Decisions and budgets

### Optimize request timing, not the warning setting

Do not increase `build.chunkSizeWarningLimit`. Retain Vite's default 500 kB warning as an
independent build signal.

Use these provisional acceptance ceilings, then tighten the checked-in entry budget to the
measured post-split entry plus modest documented headroom:

- web initial entry: no more than 450 kB minified;
- desktop UI initial entry: no more than 450 kB minified;
- every emitted Rollup application chunk: less than 500 kB minified;
- `/login` must not request workspace/provider, Hub, Account, Deal Room, Data Room,
  document-preview/pdf.js, Markdown, deliverables/templates, Canvas/Plate, or Kanban implementation
  chunks;
- ordinary Deal Room overview must not request Activity, Data Room, Analysis, Deliverables,
  Templates, or Canvas/Plate implementation chunks;
- Data Room without a selected document must not request the preview chunk or PDF worker;
- Summarize before a completed summary must not request the Markdown renderer chunk;
- the template gallery must not request the Canvas/Plate implementation before a template is
  opened.

Record minified and gzip sizes for comparison, but enforce minified bytes because Vite uses
minified size for the warning. Also record cache-disabled requests; an emitted split is not useful
when the browser still fetches it during startup.

### Prefer semantic boundaries before `manualChunks`

Use dynamic imports around routes and genuinely inactive features. Do not add `manualChunks` in the
first implementation pass. A broad vendor split can change filenames and caching without reducing
first-route bytes, and can couple unrelated features through shared packages.

Consider an explicit shared chunk only after the route, preview, Markdown, and Canvas boundaries
are complete and a module report identifies a stable dependency cluster that improves the actual
request graph. Never create one catch-all vendor chunk or split React/React DOM/router in a way that
risks duplicate runtimes or ordering problems.

If the Canvas implementation or Plate cluster is itself over 500 kB, use its module report to find
a real editor sub-boundary, such as an inactive rich-text or JSON-inspector surface. Use
`manualChunks` only if the remaining oversize owner is a stable dependency cluster that cannot be
split by product interaction and the measured request graph still improves.

### Treat the PDF worker as a separate asset

The 1.05 MB pdf.js worker is not one of Vite's oversized application chunks. It should continue to
be emitted for document preview, but it must not be requested until the user opens a PDF preview.
Worker reduction, compression/CDN policy, alternate PDF rendering, and viewer replacement remain
separate work.

### Make fixture mode explicit

Replace `{ deals, loaded }` with a discriminated resource that represents at least loading,
server success, explicitly enabled demo success, and API error. The final naming may follow nearby
style, but impossible state combinations should not be expressible.

Use an explicit, public build-time frontend setting such as
`VITE_WORKSPACE_DATA_SOURCE=api|demo`, defaulting to `api`. This is selection, not a secret:

- `api` calls `runtime.api.listDeals()` and surfaces failure with Retry; it does not import or
  append runtime fixtures as a fallback;
- `demo` intentionally loads fixture deals through a demo-only import and clearly labels the
  visible workspace as demo data; an API-mode build/request graph must not retain the deal fixtures
  merely as a dormant failure branch;
- invalid values fail deterministically during composition or produce an explicit configuration
  error rather than silently choosing fixtures;
- tests cover both modes and prove that an API error is not rendered as fixture-backed success;
- architecture documentation records the public config and feature-maturity consequence.

The provider exposes the deals resource; it must not replace every `/hub` child with a global
deals-error page. Routes that do not require a resolved deal, such as Account, retain their own
request and error behavior. The shell may expose a scoped workspace-deals warning/Retry while the
active child decides whether deals are required for its content.

Before implementation, verify whether production/demo environments already have an equivalent
composition flag. Reuse it if present rather than adding a second setting.

## Implementation order and cross-plan dependencies

The preferred order is:

1. characterize the current routes, state, and request graph;
2. introduce workspace/provider and nested route composition;
3. create URL-backed Deal Room child routes and lazy route modules;
4. rebase and implement Data Room ownership extraction on that route context;
5. extract Summarize ownership and defer its renderer;
6. add preview/editor interaction boundaries and any measured Deal Room split;
7. add final bundle budgets after all same-window dependencies land.

If the Data Room decomposition or Diligence Canvas plan is implemented first, re-read the live
source and preserve its owners rather than mechanically applying filenames or props from this
plan. In particular:

- the Data Room plan currently leaves deal/session lookup in `DataRoomPage`; after nested routing,
  shared deal/session lookup should come from the typed Deal Room/workspace route context;
- the Canvas plan currently proposes another `DealRoomPage initialView`; after nested routing, its
  `:templateId` editor must be a child route and must not restore `activeDealView`;
- final byte baselines are valid only after the latest of these changes is present.

Treat the phases as independently reviewable slices, each leaving both frontend targets compiling
and current routes usable. Do not combine the entire route, provider, Data Room, Summarize, Canvas,
and bundle-guard migration into one unreviewable implementation diff.

## Implementation sequence

### Phase 0 — Rebuild the live baseline and capture module ownership

1. Re-run `git status --short` and preserve all existing tracked and untracked work.
2. Run `npm run build:web`, immediately followed by `npm run check:web-bundle`, and record emitted
   filenames, minified/gzip sizes, and warning text before another target overwrites `dist`.
3. Run `npm run build:desktop-ui` and record the equivalent measurements.
4. Capture Rollup module ownership for the entry, Deal Room, Data Room, Summarize/Markdown, and—if
   present—Canvas/Plate chunks. Prefer a temporary local `generateBundle` reporting plugin or the
   Vite/Rollup programmatic API; do not add an analyzer dependency or commit generated output.
5. With cache disabled, record JavaScript requests for direct Login, Hub, Account, Deal Room
   overview, Activity, Analysis, Deliverables, Templates, Data Room without a selection, first
   document preview, Summarize before/after a result, Deals table/Kanban, and the editor when
   present.
6. Record which workspace modules and fixture files are in the initial entry and why.

Exit criteria:

- the 859.59/858.41 kB entries and 671.43 kB Data Room chunk are reproducible or drift is recorded;
- the module report identifies the dominant owners rather than attributing the warning to source
  line count, CSS, images, or the separate PDF worker;
- no report contains secrets, document contents, real user email, local data paths, or source maps.

### Phase 1 — Characterize route, shell, and data contracts

1. Add or extend route tests before changing composition. Cover direct entry, refresh-equivalent
   `MemoryRouter` initialization, sidebar activation, Back/Forward history, redirects, and router
   state for Hub, Account, Deal Room overview, Activity, Data Room, Analysis, Deliverables, and
   Templates.
2. Characterize `useWorkspaceDeals` API success, API failure, fixture merge/fallback, cancellation,
   and retry-relevant behavior. These tests document the old behavior before the intentional data
   source change.
3. Characterize that a fresh extraction result can render a deal not yet returned by `listDeals`
   and survives navigation to child routes through the existing state contract.
4. Cover current timeline edits, overview-tab selection, template provider resets, Data Room shell
   behavior, and Summarize success/error/export at observable boundaries.
5. Add a chunk-load test seam only where deterministic. Do not make tests depend on hashed output
   filenames; manifest/script tests will own emitted-file assertions.

Exit criteria:

- every state/navigation contract being moved has observable coverage;
- intentional changes—URL-backed views and explicit demo/API state—are distinguished from
  regressions before implementation starts.

### Phase 2 — Separate application composition, workspace data, and reusable layout

1. Keep `App.tsx` small and move the shared route declaration into an app-owned route module.
   `LoginPage` remains eager; the `/hub` provider/layout entry is a named-export-aware lazy import.
2. Introduce a workspace provider or typed parent-route context that owns `useWorkspaceSession`,
   the discriminated deals resource, retry, and extraction-compatible navigation state. It must not
   be part of Login's startup graph.
3. Split the existing shell responsibility without replacing working primitives:
   - the provider owns data/session lifecycle;
   - a shared shell owns the sidebar and viewport frame;
   - each route owns its header and main feature content;
   - `WorkspaceLayout`, `WorkspaceSidebar`, and `WorkspaceHeader` remain reusable visual pieces.
4. If needed for persistent parent layouts, separate `WorkspaceLayout` into a shell/sidebar frame
   and a main/header frame while retaining a compatibility composition during migration.
5. Move the pathname-keyed transition to the leaf route outlet if the current outer key would
   remount the provider/sidebar. Preserve `name="quarry-page"`, current enter/exit treatment,
   fallback behavior, and reduced-motion CSS.
6. Replace `WorkspaceHomeShell` data fetching and fixture imports with explicit props/context from
   the route composition. Remove its deals context only after every consumer has migrated.
7. Implement the explicit `api|demo` workspace data source. API error renders an error/retry state;
   demo mode renders visibly labelled fixtures. Do not duplicate fixtures into an error branch.
8. Add focused provider/hook/layout tests for one fetch per mounted provider, retry, unmount/stale
   completion, demo labelling, and absence of fallback on API failure.
9. Keep unrelated child routes usable when deal loading fails. Account and other independent
   request owners must not be blocked by a global deals-error replacement screen.

Exit criteria:

- Login does not load workspace provider, session/deal hook, sidebar, navigation fixtures, or
  workspace feature modules;
- reusable layout components do not fetch deals or import product fixtures;
- callers can distinguish loading, API success, demo success, and error;
- provider/sidebar state is not accidentally recreated by child navigation;
- route transitions and visible skeletons retain their current behavior.

### Phase 3 — Replace Deal Room's second router with nested routes

1. Create a lazy Deal Room parent layout at `/hub/deals/:dealId`. It resolves the deal from shared
   workspace/extraction state, renders loading or missing-deal navigation, owns the shared sidebar,
   and exposes a narrow typed outlet context.
2. Add route helpers and child routes for:
   - index/overview;
   - `activity` for `DealTimelineView`;
   - `data-room`;
   - `analysis` for the current Analysis placeholder;
   - `deliverables`;
   - `deliverables/templates`;
   - `deliverables/templates/:templateId` when the Canvas editor lands.
3. Convert Deal Room sidebar callback/static items into `NavLink`s using route helpers. Derive the
   active item from matched routes; remove `activeDealView`, `initialView`, and
   `location.state.dealView` once all live views have URL owners.
4. Keep the nested Overview/File Summary tab local unless shareable refresh behavior is explicitly
   required. If promoted to `?section=...`, parse/validate the value, replace rather than append
   invalid values, and cover Back/Forward behavior.
5. Keep timeline items and editor state at the narrowest parent that must survive child rendering.
   Do not lift Data Room preview state or unsaved Canvas document state into a global provider.
6. Scope `TemplatePreviewProvider` to the template catalog subtree and preserve its `requestKey`
   resets, mutually exclusive writes, refresh, and feedback semantics.
7. Give each substantial child a named-export-aware lazy route import and use the existing
   accessible skeleton treatment inside the stable workspace content area. Do not nest duplicate
   full-page Suspense/transition shells.
8. Add compatibility redirects only for an actual previously reachable URL. Local-state-only views
   have no old deep link to redirect.
9. Update route/sidebar tests to assert URLs, direct entry, refresh, active state, Back/Forward, and
   extraction-state continuity instead of component `initialView` props.

Exit criteria:

- every reachable primary Deal Room sidebar mode has a durable URL;
- direct links and Back/Forward work in web history and desktop hash routing;
- the overview route does not request Activity, Data Room, Analysis, Deliverables, Templates, or
  Canvas/Plate implementation chunks;
- no page-local state or `location.state` acts as a second router.

### Phase 4 — Decompose Data Room and restore the preview interaction boundary

Use [`data-room-state-decomposition-plan.md`](data-room-state-decomposition-plan.md) for the
detailed resource shapes, stale-result tests, pure data helpers, and lifecycle rules, with these
composition updates:

1. Obtain deal, deals, email, and navigation state from the nested workspace/Deal Room context
   rather than refetching them in `DataRoomPage`.
2. Keep `DataRoomPage` as the lazy child-route entry. It should compose modal state,
   `useDataRoomContents`, `useDocumentSession`, and `DataRoomWorkspace`; it should not rebuild the
   outer workspace shell/sidebar.
3. Preserve the Data Room plan's separation of content loading, document session, pure mapping,
   cancellation/staleness, and the intentional close cleanup for retained preview bytes/raw text.
4. Change the runtime import of `DocumentPreviewPanel` to a named-export-aware lazy import. Keep
   `DocumentPreviewPanelHandle` as a type-only import.
5. Mount the preview module only in the selected-document branch. Use a local accessible fallback
   that preserves split-pane geometry, does not move focus, and keeps filename/Close affordances
   available where the current contract does.
6. Preserve the selected-document key, forwarded ref, preview result/text/error props, page-count
   updates, requested-page and focus bridge, and document-switch invalidation.
7. Add tests proving no preview-module load without selection; pending/resolved preview; PDF/text
   success and error; Close cleanup; stale document switching; and route/deal changes.
8. Rebuild both targets and confirm the Data Room route is below 500 kB and preview/pdf.js is not
   requested before selection.

If Data Room remains over 500 kB, use the Phase 0 report to identify the next inactive owner.
Document search is a plausible interaction boundary because it opens from the Arc Menu; do not
split File Review, Arc Menu, or a component solely because it is large in source.

Exit criteria:

- Data Room route orchestration has one clear owner per state machine;
- it does not duplicate workspace/deal lookup from its parent route;
- first/repeated preview, focus, loading, error, close, and switch behavior remain correct;
- the route and every preview-owned application chunk stay below 500 kB.

### Phase 5 — Decompose Summarize and defer Markdown rendering

1. Extract pure file normalization, browser-path handling, supported-file selection, and tree
   construction into `data/summarize.ts` or a feature-local pure model with focused tests.
2. Extract `useSummarizeWorkflow` with a discriminated state for idle, ready, submitting, success,
   and error plus the selected source kind/files. Prevent stale or duplicate completion from
   overwriting a newer selection.
3. Move the file/folder picker and tree into focused display/interaction components driven by typed
   props. Keep DOM-only `webkitdirectory` handling at the input boundary.
4. Move Markdown rendering and save controls into `SummaryPanel`. Lazy-load the panel or its
   renderer only when a non-empty summary exists; do not preload it on route mount.
5. Keep the small Chat/Summary tab state local unless product requirements make it shareable. Do
   not add a URL parameter merely to satisfy the structural review.
6. Preserve API method selection for upload, selected server paths, single files, and manual paths;
   preserve validation, loading, error, output, and save behavior.
7. Add pure model, hook, picker/tree, panel, and route-composition tests, then compare the initial
   Summarize request graph before and after a result.

Exit criteria:

- `SummarizePage` primarily composes the feature rather than implementing file/tree/request/render
  behavior inline;
- moving code does not change upload/path API contracts or observable workflow;
- opening Summarize does not request `react-markdown`/`remark-gfm`; a successful summary requests
  the renderer once.

### Phase 6 — Integrate the planned template editor and measured Deal Room splits

This phase is conditional on the Diligence Canvas work landing in the same implementation window.
It also owns any additional split shown necessary by Phase 0/3 measurements.

1. Rebase the Canvas plan's route step onto the nested
   `/hub/deals/:dealId/deliverables/templates/:templateId` child route. Do not add another
   `initialView` prop or restore page-local view selection.
2. Keep `DeliverableTemplateCanvasView` small and route-owned. It may start the document request,
   but it must dynamically import the Canvas/Plate/SVG implementation and render a labelled
   in-content fallback.
3. Keep template ID in the path; keep the hydrated document, dirty flag, retry generation, and
   unsaved edits route-local. Do not put document JSON in router state, query parameters, workspace
   context, fixtures, or logs.
4. Confirm gallery selection, explicit Back to Templates, delete/import separation, direct editor
   entry, and stale/unmounted request behavior across both routers.
5. Measure ordinary Deal Room, Deliverables, gallery, and editor request graphs. Plate/canvas must
   appear only after editor entry.
6. If the editor application chunk exceeds 500 kB, split one measured inactive editor surface or a
   stable dependency cluster using the semantic-boundary policy above; do not raise the warning
   limit.
7. If the ordinary Deal Room route remains oversized or downloads inactive code, prefer the
   already route-backed Deliverables/Templates subtree as one lazy boundary. Lazy-load Timeline
   separately only if the activity child route still shares a material inactive owner.
8. Preserve template-provider lifetime and focused catalog/editor tests from the Canvas plan.

Exit criteria:

- gallery and ordinary Deal Room navigation do not download Canvas/Plate;
- direct editor navigation loads the orchestrator and implementation with visible pending state;
- every application chunk, including Canvas/Plate-related chunks, remains below 500 kB;
- nested route behavior supersedes the older `DealRoomPage initialView` proposal.

If the Canvas feature does not land with this work, record it as a required future budget gate and
set current budgets from the completed structure/preview/Summarize output.

### Phase 7 — Add durable entry and application-chunk budgets

1. Enable Vite's build manifest in `frontend/vite.config.ts` so checks can identify entry,
   dynamic-entry, shared, and asset outputs without depending on hashed filenames.
2. Add a small Node script such as `frontend/scripts/check-bundle-size.mjs`, using Node built-ins,
   that reads the manifest and built files from `dist`.
3. Enforce two independent limits:
   - the target-specific initial entry budget;
   - a 500,000-byte ceiling for every emitted Rollup application JavaScript chunk, matching Vite's
     current decimal-kilobyte warning calculation.
4. Do not count CSS, images, or the separately emitted pdf.js worker as application chunks. Derive
   classification from manifest metadata/file type rather than a fragile hashed-name exception.
5. Require `--target web` or `--target desktop` and print target, entry filename, measured bytes,
   entry budget, largest application chunks, and pass/fail reason. The target argument prevents
   confusion after one build overwrites `dist`.
6. Add `check:bundle-size:web` and `check:bundle-size:desktop` commands that call one implementation.
7. Add focused script tests for manifest parsing, missing/stale output, hashed filenames, an
   over-budget entry, an oversized dynamic chunk, and worker exclusion. Use temporary directories
   outside tracked output.
8. Set the final entry budgets only after every route/dependency change in the implementation
   window, including Canvas/Plate when present. Use measured output plus modest documented
   headroom; do not preserve an obsolete baseline merely to avoid updating the test.
9. Document both commands and their required position immediately after the matching build in
   `docs/ARCHITECTURE.md`.

Exit criteria:

- a deliberately low synthetic limit fails with a useful message;
- real web and desktop outputs pass both limits;
- an oversized synthetic dynamic chunk fails even when the entry passes;
- existing `check:web-bundle` remains separate and passing.

### Phase 8 — Runtime, regression, and architecture verification

1. Run focused tests for the implemented phases using the actual final paths. At minimum:

   ```sh
   npm test -- tests/App.test.tsx
   npm test -- tests/pages/DealRoomPage.test.tsx
   npm test -- tests/pages/DataRoomPage.test.tsx
   npm test -- tests/pages/SummarizePage.test.tsx
   ```

   Also run new provider/layout, route-view, Data Room hook/workspace, Summarize model/hook/panel,
   bundle-script, and Canvas tests when those files exist.

2. Run the shared frontend verification ladder from `frontend/`:

   ```sh
   npm run typecheck
   npm run check:boundaries
   npm test
   npm run build:web
   npm run check:web-bundle
   npm run check:bundle-size:web
   npm run build:desktop-ui
   npm run check:bundle-size:desktop
   ```

3. Inspect web mode with `npm run dev:web`, using disabled cache:
   - direct Login and Login-to-Hub navigation;
   - API workspace loading/error/retry and explicit demo mode;
   - direct/refresh and Back/Forward navigation to Account, overview, Activity, Analysis,
     Deliverables, Templates, Data Room, and the editor when present;
   - fresh-extraction navigation across Deal Room child routes;
   - first/repeated Data Room preview, PDF/text loading/failure, Close, and document switching;
   - Summarize file/folder/manual flows before and after Markdown output;
   - template gallery/editor selection and return when Phase 6 is present;
   - Deals table and first Kanban selection;
   - console/network for chunk 404s, duplicate downloads, runtime errors, and early worker/editor/
     Markdown requests;
   - keyboard/focus behavior, visible pending/error states, reduced motion, and narrow/wide
     viewports.
4. Inspect `npm run dev:desktop-ui` for hash-route direct entry and chunk resolution. Use the full
   Tauri runtime only if a packaged-origin issue appears; no Tauri Rust change is planned here.
5. Inspect the active theme. Verify the retained dark palette only if its feature flag is enabled
   before implementation.
6. Re-read the frontend route, state ownership, fixture, runtime composition, technology baseline,
   verification, feature maturity, and known-limitations sections of `docs/ARCHITECTURE.md`.
7. Update the canonical architecture in the implementation change with:
   - the nested route table and lazy loading boundaries;
   - workspace/deal/provider and page/feature state ownership;
   - explicit API/demo data-source behavior and public config;
   - Data Room preview and Summarize renderer boundaries;
   - Canvas/Plate boundary when implemented;
   - manifest output and bundle-size commands.
8. Run `git diff --check`, inspect the final diff for generated `dist`, source maps, secrets,
   accidental lockfile churn, and unrelated formatting, then re-run `git status --short`.

No backend or Tauri Cargo gate is required unless implementation expands into those build roots.
`npm run build:desktop` is not required for ordinary UI composition unless a packaged-app-only
chunk-resolution problem is found.

## Test and request matrix

| Scenario | Expected behavior and request boundary |
| --- | --- |
| Fresh `/login` | Only boot/router/theme/transition/Login loads; no workspace or feature fixtures |
| Login to `/hub` | Workspace provider and Hub load once; source is visibly API or explicit demo |
| Workspace API failure | Error and Retry render; fixture deals do not appear as successful server data |
| Direct `/hub/account` | Account resolves; missing-email redirect and request states remain intact |
| Direct Deal Room overview | Shared layout resolves params/extraction state; no inactive child implementation loads |
| `/activity` | URL and sidebar identify Deal Activity; timeline edits work and Back/Forward is meaningful |
| `/analysis` | URL and sidebar identify Analysis; current placeholder is refresh-safe |
| Direct Deliverables | Header/content resolve through the child route; gallery/editor code remains inactive |
| Direct Templates | Provider, loading/error/empty/success, import/delete/retry, and URL remain correct |
| Direct template editor | Canvas/Plate loads only here; ID, error/retry, unsaved edits, and Back behavior are correct |
| Data Room without selection | Route loads below 500 kB without preview/pdf.js requests |
| Select/switch/close a document | Preview fallback then panel/worker load; stale data is suppressed and retained bytes are released |
| Open Summarize | Picker/workflow loads without Markdown renderer |
| Complete a summary | Renderer loads once; Markdown display and save remain correct |
| Deals table then Kanban | Kanban remains absent until selected and then loads once |
| Web production output | Runtime boundary passes; entry and every app chunk meet budgets; no Vite warning |
| Desktop production output | Entry and every app chunk meet budgets; no warning; hash child routes resolve |

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A broad architecture refactor obscures the bundle goal | Measure after each phase and require a request-graph change from each claimed split |
| Nested routes remount providers or sidebars because the transition key wraps the whole tree | Place the keyed transition at leaf content after characterization; test fetch counts and preserved parent state |
| Deep links fail after route conversion | Centralize encoded route helpers and test browser-history refresh plus desktop hash direct entry |
| Router/extraction state is lost | Keep compatibility navigation state in the typed workspace/deal context and test fresh deals across child routes |
| Query parameters expose sensitive values | Permit only validated filters/tabs/opaque IDs; never serialize file paths, email, document content, or bytes |
| Fixture cleanup makes development unusable | Provide explicit demo composition and visible labelling; keep API failure distinct with Retry |
| Page extraction only moves code | Pair ownership changes with dynamic imports and module/network evidence; do not count line reduction as byte reduction |
| Lazy routes duplicate transition fallbacks | Keep one stable shell and use the existing skeleton treatment at the leaf content boundary |
| Preview split breaks ref/focus behavior | Preserve type-only handle, forwarded ref, key, and callbacks; test pending, resolved, switch, and Close states |
| Template provider resets during nested navigation | Scope it deliberately and test `requestKey`, import/delete/retry, and gallery/editor transitions |
| Canvas/Plate creates a new oversized chunk | Measure after dependency installation and split a semantic editor surface or stable cluster without raising the threshold |
| Manual vendor chunks hide startup work | Defer them until route/interaction splits are measured and justified |
| Too many small chunks add latency | Split routes and expensive inactive interactions only; stop when budgets and request timing pass |
| Size guard checks stale/wrong output | Require a target argument and run it immediately after the matching build |
| Active user changes distort measurements | Baseline and implement on the same preserved tree; never revert or broadly reformat unrelated work |

## Explicit non-goals

- Raising `chunkSizeWarningLimit`.
- Replacing Vite/Rollup, React, the current canary, npm, React Router, or the shared frontend tree.
- Moving every file into a new folder or introducing a second design/state-management system.
- Treating smaller files, hooks, or `Outlet` layouts as bundle wins without emitted/network proof.
- Putting local paths, workspace email, document data, or unsaved editor state in URLs.
- Making route state or client-side routing an authorization boundary.
- Replacing `react-pdf`/pdf.js or reducing the worker asset.
- Splitting every component, icon, modal, data-grid primitive, or Radix package.
- Adding permanent bundle-analysis dependencies without evidence.
- Refactoring backend routes, persistence, Tauri IPC, or API contracts except when separately
  authorized by the Canvas integration plan.
- Adding speculative prefetch/preload before navigation latency is measured.
- Treating a warning-free build as sufficient without checking initial requests and error states.

## Definition of done

- Web and desktop UI builds complete without Vite's chunk-size warning.
- Both entries pass checked-in target-specific budgets with meaningful headroom below 500 kB, and
  every Rollup application chunk remains below 500 kB.
- Deal Room uses nested durable routes for every reachable primary sidebar mode; no
  `activeDealView`, `initialView`, or `location.state.dealView` second-router behavior remains.
- The shared workspace/Deal Room shell is separate from data lifecycle and feature content, and
  Login does not request it.
- Workspace data exposes loading, API success, explicit demo success, error, and Retry without
  silent fixture fallback or server/fixture merging in API mode.
- Data Room and Summarize have clear route, state, and view owners; extraction preserves behavior
  and creates measured preview/Markdown interaction boundaries.
- Cache-disabled inspection proves Login, Deal Room overview, Data Room without selection,
  Summarize before output, and the template gallery do not request inactive implementation code.
- If Canvas lands in the same window, Plate/canvas loads only on the editor route and passes the
  same per-chunk budget.
- Every affected route preserves redirects, extraction/session continuity, async feedback, focus,
  error handling, reduced-motion behavior, and web/desktop router composition.
- Focused tests and the full frontend gates pass.
- `docs/ARCHITECTURE.md` accurately describes implemented route boundaries, state/data ownership,
  fixture mode, lazy interaction boundaries, bundle manifest, and verification commands.
- No generated `dist`, source maps, secrets, accidental dependency/lockfile churn, or unrelated
  user changes are included.
- `git diff --check` is clean and final status is reviewed against the recorded baseline.
