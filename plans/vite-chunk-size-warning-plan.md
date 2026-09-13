# Quarry Vite chunk-size warning plan

Status: proposed

Scope: shared React/Vite route composition, interaction-level code splitting, bundle
measurement, and frontend bundle-size regression checks

Baseline: current working tree inspected and both production UI targets built on 2026-09-12

Primary files:

- `frontend/src/App.tsx`
- `frontend/src/pages/DataRoomPage.tsx`
- `frontend/src/components/data-room/DocumentPreviewPanel.tsx`
- `frontend/src/pages/DealRoomPage.tsx`
- `frontend/src/components/deal-room/`
- `frontend/tests/App.test.tsx` (new)
- `frontend/tests/pages/DataRoomPage.test.tsx`
- `frontend/tests/pages/DealRoomPage.test.tsx`
- `frontend/vite.config.ts`
- `frontend/package.json`
- `frontend/scripts/`
- `docs/ARCHITECTURE.md`

## Outcome

Remove Vite's oversized JavaScript chunk warning in both production UI targets by changing
when feature code is downloaded, while preserving Quarry's shared React tree and web/desktop
runtime selection. Do not silence the warning by increasing `chunkSizeWarningLimit` or merely
move the same startup bytes into arbitrary vendor chunks.

The implementation should:

- keep `LoginPage` and the minimal router/theme/transition shell eager;
- lazy-load the remaining eager workspace pages: `HubPage`, `AccountPage`, and
  `DealRoomPage`;
- preserve the six existing route boundaries for Global Vault, Vault, Explore, Logs, Deals,
  and Data Room;
- preserve `App.tsx`'s keyed page `ViewTransition` and the existing `LazyPage` skeleton rather
  than inventing a second loading system;
- restore an interaction boundary around `DocumentPreviewPanel`, which is currently imported
  statically by the already-lazy Data Room route and is the main natural owner inside its
  oversized chunk;
- split Deal Room's new deliverables/template feature or Timeline only if the measured
  Deal Room route chunk remains oversized or default Deal Room navigation fetches substantial
  inactive feature code;
- produce warning-free web and desktop UI builds without changing Vite's warning threshold;
- add a repository-native guard for both the initial entry and all emitted application chunks;
- preserve route state, direct links, redirects, PDF behavior, template-import behavior,
  web `BrowserRouter`, desktop `HashRouter`, runtime aliases, accessibility, and async states;
- avoid backend, Tauri Rust, API-contract, dependency, and lockfile changes.

## Current evidence

The warning is reproducible in both targets on the 2026-09-12 working tree. The installed
lockfile resolves Vite 7.3.6 even though `package.json` declares `^7.0.4`.

| Target | Command | Initial entry | Gzip | Data Room chunk | Gzip | Result |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| Web | `npm run build:web` | 859.59 kB | 264.34 kB | 671.43 kB | 202.55 kB | Passes with two chunks over 500 kB |
| Desktop UI | `npm run build:desktop-ui` | 858.41 kB | 264.01 kB | 671.43 kB | 202.55 kB | Passes with the same warning |

Other emitted artifacts in both builds:

| Artifact | Minified size | Interpretation |
| --- | ---: | --- |
| PDF worker asset | 1,046.21 kB | Separate `.mjs` worker emitted for PDF preview; not a Rollup application chunk |
| Shared Markdown-related chunk | 156.78 kB | Existing shared lazy dependency chunk |
| Deals Kanban chunk | 95.09 kB | Already loaded by an interaction-level dynamic import |
| Deals route chunk | 31.43 kB web / 31.94 kB desktop | Route-lazy before the Kanban boundary |
| Global Vault route chunk | 4.17 kB | Existing route boundary |
| Vault route chunk | 5.90 kB | Existing route boundary |
| Explore route chunk | 12.46 kB | Existing route boundary, still implemented by `SummarizePage` |
| Logs route chunk | 9.40 kB | Existing route boundary |

The August plan no longer matches the source in several important ways:

- `App.tsx` still has four eager pages—Login, Hub, Account, and Deal Room—but it now wraps
  route changes in a keyed React 19 `ViewTransition`.
- `LazyPage` already renders `WorkspacePageSkeleton` with accessible labels and enter/exit
  transitions. The old blank-fallback task is obsolete.
- Deal Room now serves three direct paths through one eager page:
  `/hub/deals/:dealId`, `/hub/deals/:dealId/deliverables`, and
  `/hub/deals/:dealId/deliverables/templates`.
- `DealRoomPage` statically imports `DeliverablesHeader`, `DeliverableTemplatesView`,
  `DeliverablesView`, `TemplatePreviewProvider`, and `DealTimelineView`, so template upload,
  carousel, data-grid, and timeline code participate in the eager entry graph even when the
  user opens Login or the ordinary Deal Room overview.
- `DataRoomPage` now statically imports `DocumentPreviewPanel`. That panel imports the full
  `react-pdf` viewer graph and emits the 1.05 MB pdf.js worker, so the previous preview boundary
  described by the August plan no longer exists.
- Data Room also gained File Review, document search, Arc Menu, floating-panel, and data-grid
  UI. Those features should remain with Data Room unless a post-preview-split module report
  proves another boundary is necessary.
- The frontend now uses `@tanstack/react-table` and `embla-carousel-react`, and contains much
  larger reusable data-grid and interaction primitives. Bundle decisions must be based on the
  current Rollup module graph rather than the old 528 kB entry snapshot.
- Frontend tests now live under `frontend/tests/`; new route tests must not be placed under
  `frontend/src/`.
- `check:web-bundle` checks that Tauri runtime code is absent from `dist`; it is not a size
  check and should retain that narrow responsibility.

`frontend/vite.config.ts` currently has no `manualChunks`, bundle manifest, or custom warning
limit. That is the correct starting point.

## Contracts that must remain stable

- Preserve every current route and redirect in `App.tsx`, including the deliverables and
  deliverable-template paths.
- Web continues to use `BrowserRouter`; desktop continues to use `HashRouter`.
- `frontend/vite.config.ts`, `tsconfig.web.json`, and `tsconfig.desktop.json` keep matching
  `@quarry/router` and `@quarry/runtime` aliases.
- Shared UI continues to use `@quarry/runtime`; raw Tauri imports remain isolated to
  `frontend/src/platform/runtime.desktop.ts`, and raw HTTP/SSE remains under `src/api/`.
- The outer page transition remains keyed by `location.pathname`. Lazy routes continue to use
  the existing skeleton plus `slide-down`/`slide-up` transition treatment and reduced-motion
  behavior.
- Router state carrying workspace email, extraction results, source filenames, active Deal Room
  views, and deal navigation survives every lazy boundary.
- `DealRoomPage` retains its loading and missing-deal redirects, route-backed initial views,
  `requestKey`-scoped template-preview state, header modes, and sidebar navigation.
- Data Room retains selected-document state, text/PDF loading and error behavior, page counts,
  preview focus/selection callbacks, upload-job behavior, File Review, document search, Arc Menu,
  and SharePoint-modal behavior.
- A lazy-module failure remains distinct from route-data or API failure; do not convert it into
  fixture data, an empty success screen, or an unrelated API error.
- Direct navigation and refresh keep working for both router targets under their existing
  hosting rules.
- Preserve the current React 19 canary, Vite/npm workflow, Tailwind setup, package manifest,
  lockfile, and dependency set.

## Decisions and budgets

### Optimize request timing, not the warning setting

Do not increase `build.chunkSizeWarningLimit`. Retain Vite's default 500 kB warning as an
independent build signal.

Use these provisional acceptance ceilings, then tighten the checked-in entry budget to the
measured post-split entry plus modest documented headroom:

- web initial entry: no more than 450 kB minified;
- desktop UI initial entry: no more than 450 kB minified;
- every emitted Rollup application chunk: less than 500 kB minified;
- `/login` must not request Hub, Account, Deal Room, Data Room, document-preview/pdf.js,
  deliverable-template, or Kanban implementation chunks;
- Data Room without a selected document must not request the preview chunk or PDF worker;
- ordinary Deal Room overview must not request a conditional deliverables/template or Timeline
  chunk if Phase 4 introduces those boundaries.

Record minified and gzip sizes for comparison, but enforce minified bytes because Vite uses
minified size for the warning. Also record the cache-disabled network graph; splitting a file is
not useful if the browser still fetches it during startup.

### Prefer semantic boundaries before `manualChunks`

Use dynamic imports around routes and genuinely inactive features. Do not add `manualChunks` in
the first implementation pass. A broad vendor split can change filenames and caching without
reducing first-route bytes, and can couple unrelated features through shared packages.

Consider an explicit shared chunk only after the route, preview, and measured Deal Room splits
are complete and a module report identifies a stable dependency cluster that improves the
actual request graph. Never create one catch-all vendor chunk or split React/React DOM/router in
a way that risks duplicate runtimes or ordering problems.

### Treat the PDF worker as a separate asset

The 1.05 MB pdf.js worker is not one of Vite's oversized application chunks. It should continue
to be emitted for document preview, but it must not be requested until the user opens a PDF
preview. Worker reduction, compression/CDN policy, alternate PDF rendering, and viewer
replacement remain separate work.

## Implementation sequence

### Phase 1 — Capture the implementation baseline and module ownership

1. Re-run `git status --short` and preserve all existing tracked and untracked work.
2. Run `npm run build:web`, immediately followed by `npm run check:web-bundle`, and record the
   emitted filenames, minified/gzip sizes, and warning text before another target overwrites
   `dist`.
3. Run `npm run build:desktop-ui` and record the equivalent measurements.
4. Capture Rollup module ownership for the entry and Data Room chunks. Prefer a temporary local
   `generateBundle` reporting plugin or the Vite/Rollup programmatic API; do not add an analyzer
   dependency or commit source maps/generated `dist` output merely for this report.
5. Group entry modules by behavior:
   - minimal boot/router/theme/transition/Login;
   - shared workspace shell/sidebar/session/fixtures;
   - Hub and Account;
   - Deal Room overview/data grid;
   - deliverables/templates/upload/carousel;
   - Timeline and inactive Deal Room views;
   - Data Room shell/File Review/search/Arc Menu;
   - document preview/react-pdf/pdf.js;
   - web or desktop runtime adapter.
6. With cache disabled, record JavaScript requests for direct Login, Hub, Deal Room overview,
   Deliverables, Templates, Data Room without a selection, first document preview, Deals table,
   and first Kanban selection.

Exit criteria:

- The 859.59/858.41 kB entries and 671.43 kB Data Room chunk are reproducible or any drift is
  recorded before implementation.
- The module report confirms which owners dominate both oversized chunks.
- The warning is not misattributed to CSS, an image, or the separate PDF worker.
- No report committed or shared contains secrets, document contents, real user email, local data
  paths, or generated source maps.

### Phase 2 — Move eager workspace pages behind the existing route boundary

1. In `frontend/src/App.tsx`, replace static imports of `HubPage`, `AccountPage`, and
   `DealRoomPage` with named-export-aware `lazy(() => import(...).then(...))` declarations.
2. Keep `LoginPage`, `ThemeModeProvider`, route definitions, redirects, and the outer keyed
   `ViewTransition` eager and structurally unchanged.
3. Wrap every new lazy page in the existing `LazyPage`. Use concise page-appropriate labels such
   as `Loading workspace`, `Loading account`, and `Loading deal`; do not add another Suspense or
   skeleton abstraction.
4. Apply the same lazy `DealRoomPage` declaration to all three Deal Room routes and preserve each
   route's current `initialView` prop.
5. Keep all params, `Navigate` branches, location state, and session/extraction behavior inside
   the page components. Do not combine bundle work with a state or routing refactor.
6. Add `frontend/tests/App.test.tsx` using the repository's existing Vitest and Testing Library
   setup. Cover Login and each newly lazy route, including the two `initialView` variants, without
   duplicating detailed page tests.
7. Rebuild both targets and compare entry and route chunks with Phase 1.

Exit criteria:

- Direct `/login` no longer requests Hub, Account, or Deal Room implementation code.
- Direct and in-app navigation to Hub, Account, Deal Room, Deliverables, and Templates resolves
  with the existing route transition and visible skeleton.
- Both entry chunks meet the provisional 450 kB ceiling.
- Web bundle isolation still passes.

Phase 2 is expected to fix the oversized entry, but it cannot fix the current 671.43 kB Data
Room chunk. Continue to Phase 3 even if the entry warning disappears.

### Phase 3 — Restore the document-preview interaction boundary

1. In `DataRoomPage.tsx`, change the runtime import of `DocumentPreviewPanel` to a named-export-
   aware React lazy import. Keep `DocumentPreviewPanelHandle` as a type-only import so the ref
   contract remains type-safe without pulling runtime code into the route chunk.
2. Mount the lazy panel only in the existing `selectedDocument` branch. Do not preload it merely
   because Data Room mounted.
3. Add a local Suspense fallback in the preview content area. It should preserve the current
   split-pane geometry, expose a polite loading status, avoid moving focus, and use existing
   skeleton/theme primitives.
4. Preserve `key={selectedDocument.id}`, the forwarded ref, preview result/text/error props,
   page-count updates, selection focus behavior, and document-switch cancellation/staleness
   handling.
5. Extend `frontend/tests/pages/DataRoomPage.test.tsx` to prove:
   - Data Room renders without loading the preview module when no document is selected;
   - selecting a document shows the preview pending state and then resolved panel;
   - PDF/text success and failure paths still reach the existing panel contract;
   - switching documents does not reveal stale preview state.
6. Rebuild and re-measure both targets. Confirm the Data Room route chunk is below 500 kB and the
   preview/pdf.js code is requested only after selection.

Exit criteria:

- Both builds contain no application chunk over 500 kB after Phases 2 and 3.
- Data Room opens without requesting `DocumentPreviewPanel` or the pdf.js worker.
- First preview, repeated preview, focus, loading, error, and document-switch behavior remain
  correct.

If Data Room remains over 500 kB, use the Phase 1 module report to identify the next largest
inactive owner. Document search is the next plausible interaction boundary because it lives
behind Arc Menu activation; do not split File Review or Arc Menu solely on source line count.

### Phase 4 — Split one inactive Deal Room feature only if measurement requires it

This phase is conditional. Use it if the lazy Deal Room chunk itself exceeds 500 kB or if the
cache-disabled overview request downloads substantial code used only by another view.

1. Prefer one deliverables feature boundary that owns the route-backed deliverables/templates
   surface, including its header, upload controls, carousel, and template view. This is now a
   stronger boundary than Timeline because it has dedicated URLs and newly added dependencies.
2. Preserve `TemplatePreviewProvider`'s `requestKey` reset semantics and ensure its store is not
   recreated during ordinary loading or header/content resolution. Keep Deliverables and
   Templates navigation paths and header callbacks unchanged.
3. Render the feature inside the existing `WorkspaceLayout` with an in-content accessible
   fallback so the sidebar and route transition remain stable.
4. Extend `frontend/tests/pages/DealRoomPage.test.tsx` and the existing focused deliverables
   component tests to cover route-backed initial views, loading, resolved content, template
   import/delete/retry feedback, and navigation between Deliverables and Templates.
5. Rebuild and re-measure the default Deal Room overview and both deliverables routes.
6. Only if the Deal Room chunk still fails or the module report shows Timeline as a material
   inactive owner, lazy-load `DealTimelineView` as a second boundary. Keep `timelineItems` and
   `onEventsChange` in `DealRoomWorkspace`, and test first/repeated selection plus state updates.

Do not split `DealSummaryCard`, small under-construction panels, individual icons, buttons, or
Radix primitives. If the measured chunk still fails after the semantic boundaries, investigate
accidental broad imports or duplicate runtimes before proposing `manualChunks`.

Exit criteria:

- Default Deal Room navigation does not download code owned only by any implemented conditional
  boundary.
- Route-backed deliverables/template behavior and provider lifetime remain unchanged.
- Every application chunk stays below Vite's warning threshold.

### Phase 5 — Add durable entry and application-chunk budgets

1. Enable Vite's build manifest in `frontend/vite.config.ts` so checks can identify entry,
   dynamic-entry, shared, and asset outputs without depending on hashed filenames.
2. Add a small Node script such as `frontend/scripts/check-bundle-size.mjs`, using Node built-ins,
   that reads the manifest and built files from `dist`.
3. Enforce two independent limits:
   - the target-specific initial entry budget;
   - a 500 kB ceiling for every emitted Rollup application JavaScript chunk.
4. Do not count CSS, images, or the separately emitted pdf.js worker as application chunks. Make
   classification come from manifest metadata/file type rather than a fragile hashed-name
   exception.
5. Require `--target web` or `--target desktop` and print the target, entry filename, measured
   bytes, entry budget, largest application chunks, and pass/fail reason. The explicit target
   prevents silent confusion after one build overwrites `dist`.
6. Add clear npm commands, for example `check:bundle-size:web` and
   `check:bundle-size:desktop`, that pass the target argument to one implementation.
7. Add focused script tests for manifest parsing, missing output, hashed filenames, an
   over-budget entry, an oversized dynamic chunk, and exclusion of a worker asset. Use temporary
   directories outside tracked output.
8. Document both commands and their required position immediately after the matching build in
   `docs/ARCHITECTURE.md`.

Exit criteria:

- A deliberately low test limit fails with a useful message.
- The real web and desktop outputs pass both limits.
- An oversized synthetic dynamic chunk fails even when the entry passes.
- Existing `check:web-bundle` behavior remains separate and passing.

### Phase 6 — Runtime, regression, and architecture verification

1. Run the focused tests first:

   ```sh
   npm test -- tests/App.test.tsx
   npm test -- tests/pages/DataRoomPage.test.tsx
   npm test -- tests/pages/DealRoomPage.test.tsx
   ```

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
   - direct/refresh navigation to Account, Deal Room, Deliverables, Templates, and Data Room;
   - first and repeated document preview, including PDF/text loading and failure;
   - first and repeated template view/import interaction if Phase 4 is used;
   - Deals table and first Kanban selection;
   - console/network for chunk 404s, duplicate downloads, runtime errors, and early worker
     requests;
   - keyboard/focus behavior, visible pending states, reduced motion, and narrow/wide viewports.
4. Inspect `npm run dev:desktop-ui` for hash-route chunk resolution. Use the full Tauri runtime
   only if a packaged-origin issue appears; no Tauri Rust change is planned.
5. Inspect the currently active theme. The retained dark palette is disabled by the current
   theme feature flag; verify dark mode too only if that flag is enabled before implementation.
6. Re-read the frontend route, runtime composition, technology baseline, verification, and known
   limitations sections of `docs/ARCHITECTURE.md`.
7. Update the route table to describe Hub, Account, and all Deal Room paths as lazy. Document the
   preview boundary, any implemented Deal Room feature boundary, manifest output, and the new
   size-check commands without turning architecture documentation into a hashed chunk list.
8. Run `git diff --check`, inspect the final diff for generated `dist`, source maps, secrets,
   accidental lockfile churn, and unrelated formatting, then re-run `git status --short`.

No backend or Tauri Cargo gate is required unless implementation expands into those build roots.
`npm run build:desktop` is not required for ordinary UI composition unless a packaged-app-only
chunk-resolution problem is found.

## Test matrix

| Scenario | Expected behavior |
| --- | --- |
| Fresh `/login` | Only the boot/router/theme/transition/Login graph loads |
| Login to `/hub` | Hub chunk loads once; route state and workspace shell remain correct |
| Direct `/hub/account` | Account chunk resolves; missing-email redirect and account async states remain intact |
| Direct Deal Room overview | Lazy Deal Room resolves with params, extraction state, loading, and missing-deal redirect intact |
| Direct Deliverables | Correct `initialView`, header, sidebar, and route state resolve through the same lazy page |
| Direct Templates | Template provider, loading/error/empty/success states, import, delete, and retry remain correct |
| Data Room without selection | Data Room loads below 500 kB without preview/pdf.js requests |
| Select a document | Preview fallback appears, then the text/PDF panel and worker load as needed |
| Switch documents | Ref/focus behavior and cancellation prevent stale preview content |
| Deals table | Deals route loads without the Kanban chunk |
| Select Kanban | Kanban chunk loads once and existing interaction behavior remains intact |
| Web production output | Boundary check passes; entry and every application chunk meet budgets; no Vite warning |
| Desktop production output | Entry and every application chunk meet budgets; no Vite warning; hash routes resolve |

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Lazy routes duplicate or nest transition fallbacks | Reuse the existing `LazyPage` and outer keyed `ViewTransition` exactly once per route |
| Deep links fail after splitting | Preserve route definitions and test BrowserRouter refresh plus desktop hash navigation |
| Router or extraction state is lost | Keep navigation calls and page-owned state unchanged; cover all three Deal Room initial views |
| Preview split breaks ref/focus behavior | Keep the handle type-only import, forwarded ref, key, and callbacks; test pending and resolved states |
| PDF worker is mistaken for an app chunk | Classify manifest outputs explicitly and verify worker request timing separately |
| Template provider resets during lazy resolution | Preserve `requestKey` ownership and test navigation/import state across the feature boundary |
| Shared imports pull feature code back into startup | Compare Rollup module ownership and cache-disabled request graphs, not filenames alone |
| Manual vendor chunks hide rather than solve startup work | Defer them until semantic boundaries are measured and justified |
| Too many tiny chunks add latency | Split only routes and expensive inactive interactions; stop when budgets and request timing pass |
| Size guard checks stale or wrong output | Require a target argument and run immediately after its matching build |
| New tests are placed in the source tree | Put all frontend tests under `frontend/tests/` as required by the boundary check |
| Active user changes distort measurements | Baseline and implement on the same preserved tree; never revert or reformat unrelated work |

## Explicit non-goals

- Raising `chunkSizeWarningLimit`.
- Replacing Vite/Rollup, React, the current canary, or npm.
- Adding a permanent bundle-analyzer dependency without evidence.
- Replacing `react-pdf`/pdf.js or reducing the worker asset.
- Splitting every component, icon, modal, data-grid primitive, or Radix package.
- Refactoring application state, fixtures, API adapters/contracts, backend routes, or Tauri IPC.
- Adding speculative route prefetch/preload before navigation latency is measured.
- Treating a warning-free build as sufficient without checking initial network requests.

## Definition of done

- Web and desktop UI builds complete without Vite's chunk-size warning.
- Both entry chunks pass a checked-in target-specific budget with meaningful headroom below
  500 kB, and every Rollup application chunk remains below 500 kB.
- Cache-disabled inspection proves Login does not request workspace/Deal Room/Data Room/preview/
  template/Kanban implementation code.
- Data Room does not request preview or pdf.js code until a document is selected.
- Every affected route preserves route state, redirects, async feedback, focus, error handling,
  reduced-motion behavior, and web/desktop router composition.
- Focused tests and the full frontend gates pass.
- `docs/ARCHITECTURE.md` accurately describes the final route boundaries, preview boundary,
  optional Deal Room boundary, manifest output, and verification commands.
- No generated `dist`, source maps, secrets, dependency/lockfile churn, or unrelated user changes
  are included.
- `git diff --check` is clean and final status is reviewed against the recorded dirty-tree
  baseline.
