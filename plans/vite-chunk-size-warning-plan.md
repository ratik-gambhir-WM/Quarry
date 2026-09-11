# Quarry Vite chunk-size warning plan

Status: proposed

Scope: shared React/Vite route composition, bundle measurement, and frontend bundle-size
regression checks

Baseline: active working tree inspected and both production UI targets built on 2026-08-31

Primary files:

- `frontend/src/App.tsx`
- `frontend/src/pages/DealRoomPage.tsx`
- `frontend/vite.config.ts`
- `frontend/package.json`
- `frontend/scripts/`
- `docs/ARCHITECTURE.md`

## Outcome

Remove Vite's oversized JavaScript chunk warning by reducing code in the initial application
chunk, while preserving Quarry's one shared React tree and its web/desktop runtime selection.
Optimize what users download and parse on the first route instead of merely increasing
`chunkSizeWarningLimit` or moving the same eager bytes into arbitrary vendor chunks.

The first implementation should:

- keep `LoginPage` and the minimal application/router/theme shell eager;
- lazy-load the remaining eager workspace routes (`HubPage`, `AccountPage`, and
  `DealRoomPage`) at their existing paths;
- retain the existing lazy boundaries for Vault, Summarize, Logs, Deals, Data Room, document
  preview, and Deals Kanban;
- split a non-default feature inside Deal Room only if route splitting does not meet the agreed
  bundle budget or the Deal Room route chunk is itself unreasonably large;
- produce warning-free web and desktop UI builds without changing the warning threshold;
- add a repository-native entry-chunk budget check so a later eager import cannot silently
  recreate the problem;
- preserve route state, deep links, web `BrowserRouter`, desktop `HashRouter`, runtime aliases,
  API behavior, accessibility, and current loading/error behavior except for an intentional
  route-loading treatment;
- avoid backend, Tauri Rust, API contract, dependency, and lockfile changes.

## Current evidence

The warning is reproducible in both targets in the current dirty working tree:

| Target | Command | Oversized initial chunk | Gzip | Result |
| --- | --- | ---: | ---: | --- |
| Web | `npm run build:web` | 528.63 kB | 163.61 kB | Build passes with Vite's >500 kB warning |
| Desktop UI | `npm run build:desktop-ui` | 527.30 kB | 163.19 kB | Build passes with the same warning |

Other relevant emitted artifacts in both builds:

| Artifact | Minified size | Interpretation |
| --- | ---: | --- |
| `DocumentPreviewPanel` chunk | 485.63 kB | Large, but already lazy and below Vite's warning threshold |
| PDF worker asset | 1,046.21 kB | Separate worker asset loaded by the preview path; not the JavaScript chunk that triggered the warning |
| Shared Markdown-related chunk | 156.78 kB | Shared by lazy Markdown consumers rather than part of the initial route alone |
| Deals Kanban chunk | 95.43 kB | Already loaded on demand from the lazy Deals route |
| Data Room route chunk | 60.90 kB | Already route-lazy, with preview split again behind user interaction |

`frontend/src/App.tsx` already lazy-loads six route pages. The four eager pages are `LoginPage`,
`HubPage`, `AccountPage`, and `DealRoomPage`. Keeping all four eager means opening `/login` also
loads the workspace shell and Deal Room graph. `DealRoomPage` statically imports the large
`DealTimelineView`, `InsightsStrip`, Deal Room components, workspace fixtures, and workspace
shell even though a login visitor has not entered a deal.

The current source also has natural second-level boundaries:

- `DataRoomPage` dynamically imports `DocumentPreviewPanel`;
- `Deals` dynamically imports `DealsKanban`;
- Deal Room starts on the `deal-room` overview but statically imports the non-default timeline
  view.

This makes route-level splitting the smallest evidence-backed first change. Randomly lazy-loading
buttons, cards, icons, or other small primitives is out of scope.

## Contracts that must remain stable

- The route paths and redirect behavior in `App.tsx` do not change.
- Web continues to use `BrowserRouter`; desktop continues to use `HashRouter`.
- `frontend/vite.config.ts`, `tsconfig.web.json`, and `tsconfig.desktop.json` keep matching
  `@quarry/router` and `@quarry/runtime` aliases.
- Shared UI continues to use `@quarry/runtime`; no raw Tauri import, `fetch`, or `EventSource`
  moves into product components.
- Router state used for workspace email, extraction results, account data, and deal navigation
  remains intact across lazy boundaries.
- Direct navigation and refresh continue to work for web and desktop routes under their existing
  hosting rules.
- Lazy loading exposes a visible, accessible pending state and does not strand keyboard focus or
  create a blank page that looks broken.
- A chunk-load failure remains distinguishable from ordinary route data loading. Do not catch a
  failed module load and present fixture data or an empty successful screen.
- The existing React 19 canary, Vite version, npm workflow, package manifest, lockfile, Tailwind
  setup, and dependency set remain unchanged unless measurement proves an installed tool cannot
  support the work.

## Decisions and budgets

### Optimize the entry path, not the warning setting

Do not set `build.chunkSizeWarningLimit` above 500 kB. That would silence the observed symptom
without reducing download, parse, or compile work. Retain Vite's default warning as an independent
signal.

Use these initial acceptance budgets after measuring the implemented split:

- web initial entry chunk: at most 450 kB minified;
- desktop UI initial entry chunk: at most 450 kB minified;
- no emitted application JavaScript chunk above Vite's 500 kB threshold;
- no regression in total JavaScript loaded for `/login` relative to the recorded baseline;
- document-preview/pdf.js code is absent from the `/login` and ordinary workspace startup
  request graph and remains behind the preview interaction.

The 450 kB entry budget leaves headroom below Vite's 500 kB warning instead of declaring success
at 499 kB. Record both minified and gzip sizes, but enforce minified bytes because that is the
metric Vite uses for this warning. If route splitting produces a much smaller entry, set the final
checked-in budget near the measured result plus a modest documented allowance rather than
automatically using the full 450 kB ceiling.

### Prefer semantic dynamic imports before `manualChunks`

Dynamic imports at routes and expensive interaction boundaries change when code is fetched.
`manualChunks` often changes only file layout and cache behavior while preserving the same initial
bytes. It can also create fragile dependency coupling. Do not add `manualChunks` in the first
implementation pass.

Consider an explicit vendor/shared chunk policy only if the post-split report shows a stable,
independently cacheable dependency cluster and the browser still downloads less code on the
initial route. Never create a `vendor` chunk containing every `node_modules` package, and do not
split React, React DOM, and the router in a way that creates duplicate runtimes or ordering
problems.

### Treat the PDF worker separately

The 1.05 MB pdf.js worker is an emitted worker asset, not the oversized entry chunk. Keep it
behind the already-lazy document preview path and verify that it is not requested before a preview
opens. Worker-size reduction, alternate PDF rendering, CDN delivery, compression policy, and
document-viewer replacement are separate projects and are not required to resolve this warning.

## Implementation sequence

### Phase 1 — Capture a repeatable baseline and module ownership report

1. Re-run `git status --short` and preserve all existing tracked and untracked frontend work.
2. Run `npm run build:web`, immediately followed by `npm run check:web-bundle`, and save the
   emitted chunk names, minified sizes, gzip sizes, and warning text in the implementation notes.
3. Run `npm run build:desktop-ui` and save the same measurements before `dist` is overwritten by
   another target build.
4. Inspect the initial entry's Rollup module list and rendered byte contribution. Prefer a small
   temporary/local Vite `generateBundle` reporting plugin or the Vite/Rollup programmatic API over
   adding a permanent analyzer dependency. Do not commit source maps containing application
   source or generated `dist` output.
5. Group entry modules by ownership rather than package name alone:
   - minimal boot/router/theme/login;
   - workspace shell/sidebar/data fixtures;
   - Hub and Account pages;
   - Deal Room overview and non-default views;
   - shared UI/icon/motion/Radix dependencies;
   - web or desktop runtime adapter.
6. Use a clean browser profile or disabled cache to record which JavaScript requests occur for
   direct `/login`, post-login `/hub`, direct Deal Room, Data Room, document preview, and Deals
   Kanban. Record request bytes and the point at which each lazy chunk is requested.

Exit criteria:

- Both target baselines can be reproduced from the current tree.
- The exact modules making up the ~528 kB initial chunk are known.
- The entry warning is not misattributed to the PDF worker or a generated CSS asset.
- The report contains no secrets, document contents, user email, local absolute data paths, or
  generated source maps intended for commit.

### Phase 2 — Move eager workspace pages behind route boundaries

1. In `frontend/src/App.tsx`, replace the static imports of `HubPage`, `AccountPage`, and
   `DealRoomPage` with named-export-aware `lazy(() => import(...).then(...))` declarations,
   matching the established page-loading pattern.
2. Keep `LoginPage`, `ThemeModeProvider`, React Router composition, redirects, and the route
   manifest eager. Login remains the minimal first screen.
3. Wrap the three newly lazy page elements in the same deliberate route-level Suspense boundary
   used by existing lazy routes. Consolidate repeated route fallback markup into a small local
   component only if that improves consistent semantics; do not create a new styling system.
4. Replace the current visually blank route fallback with a stable shell-colored pending screen
   that has an accessible status such as `Loading workspace…`. Avoid focus stealing and honor
   reduced-motion preferences. If direct Deal Room navigation needs a different message, keep the
   distinction small and page-neutral.
5. Preserve each existing `Navigate`, location state, params, workspace session, extraction
   result, and data-loading branch. Do not combine route splitting with state-management or page
   refactors.
6. Add focused route composition tests using an in-memory router. Cover the eager login route and
   each newly lazy route sufficiently to prove that Suspense resolves to the correct page without
   changing paths or redirects. Use synthetic account/deal data and do not encode a real email or
   document path in fixtures.
7. Build both targets again and compare the entry and route chunk table with Phase 1.

Exit criteria:

- `/login` no longer downloads Hub, Account, or Deal Room route code.
- Direct and in-app navigation to `/hub`, `/hub/account`, and `/hub/deals/:dealId` still resolves
  in both router targets.
- The web and desktop entry chunks meet the 450 kB budget.
- Neither build prints Vite's oversized chunk warning.
- Existing lazy feature chunks remain lazy; no Tauri implementation appears in the web bundle.

If these criteria pass, skip Phase 3. Do not keep splitting solely to maximize the number of
chunks.

### Phase 3 — Add one Deal Room feature boundary only if measurement requires it

This phase is conditional. Use it if the entry budget still fails, if the new Deal Room route
chunk exceeds 500 kB, or if runtime measurement shows that navigating to the default Deal Room
downloads substantial code used only by a non-default view.

1. Change `DealTimelineView` to a dynamic import owned by `DealRoomPage` because the timeline is
   not the default `activeDealView`.
2. Render the timeline inside a local Suspense boundary within the existing workspace layout so
   the sidebar/header remain stable while that view loads.
3. Provide an in-content accessible loading state that preserves layout and keyboard behavior.
4. Keep timeline state (`timelineItems` and `onEventsChange`) in `DealRoomWorkspace`; only move the
   view implementation behind the import boundary.
5. Add or extend focused Deal Room tests to cover selecting Timeline, its loading state, resolved
   content, and state updates after the module resolves.
6. Re-measure the default Deal Room navigation and Timeline selection. Confirm the timeline chunk
   is requested only on first selection and cached thereafter.

Do not lazy-load `InsightsStrip` from the default overview merely because its source file is
large: it is needed immediately for the default Deal Room view. Do not split the small
under-construction panels, cards, icons, or modal primitives individually.

If the budget still fails after this boundary, return to the module ownership report and look for
an accidental broad barrel import, duplicate library/runtime, or eager fixture payload. Propose a
separate, evidence-backed refactor for that owner. Do not improvise a global `manualChunks`
function to make the warning line disappear.

Exit criteria:

- Default Deal Room behavior does not download timeline-only implementation code.
- Timeline behavior and state ownership remain unchanged after the lazy module resolves.
- The target budgets are met without a warning-threshold increase.

### Phase 4 — Add an entry-chunk regression guard

1. Add a small Node script under `frontend/scripts/` that reads the built `dist/index.html`,
   resolves its module entry, measures the emitted minified file, and fails above a checked-in
   byte budget. Use Node built-ins already available to the repository.
2. Name the script and npm command clearly, for example `check:bundle-size`. Keep it separate from
   `check:web-bundle`, whose current responsibility is preventing Tauri runtime code from entering
   the web bundle.
3. Make the check accept the current `dist` target so it can run immediately after either
   `build:web` or `build:desktop-ui`. Include the target/mode in the command or report to avoid
   mistaking a desktop build for a web build after Vite overwrites `dist`.
4. Print the measured entry filename, minified bytes, configured budget, and pass/fail result.
   Do not depend on hashed filenames.
5. Add focused tests for entry-tag discovery, missing output, and over-budget behavior if the
   parsing logic is more than a trivial direct script. Temporary test directories must remain
   outside tracked build output.
6. Document the command beside the standard frontend build gates in `docs/ARCHITECTURE.md`.

The guard should protect the entry path, not reject the known pdf.js worker solely because it is
larger than 500 kB. Vite's own build warning continues to cover oversized Rollup chunks.

Exit criteria:

- A deliberately lowered local test budget makes the script fail with a useful message.
- The real web and desktop builds pass the checked-in budget.
- Hash changes do not break entry discovery.
- Existing runtime-boundary checks remain unchanged and passing.

### Phase 5 — Runtime, regression, and architecture verification

1. Run focused route and Deal Room tests first.
2. Run the full shared frontend verification ladder from `frontend/`:

   ```sh
   npm run typecheck
   npm run check:boundaries
   npm test
   npm run build:web
   npm run check:web-bundle
   npm run check:bundle-size
   npm run build:desktop-ui
   npm run check:bundle-size
   ```

3. Inspect web mode with `npm run dev:web`:
   - direct `/login` load with cache disabled;
   - login-to-Hub navigation;
   - direct/refresh navigation to each newly lazy route;
   - Deal Room overview and, if Phase 3 was used, first/repeated Timeline selection;
   - Data Room preview and Deals Kanban to confirm the older feature boundaries still load;
   - browser console/network for chunk 404s, duplicate downloads, runtime errors, and eager PDF
     worker requests;
   - keyboard/focus behavior, visible loading status, light/dark themes, reduced motion, and
     narrow/wide viewport layouts.
4. Inspect desktop UI mode with `npm run dev:desktop-ui` for chunk resolution and hash-route
   behavior. Use the full Tauri runtime only if necessary to validate a packaged-origin loading
   issue; no Tauri Rust change is planned.
5. Re-read the frontend route, runtime composition, technology baseline, and verification
   sections of `docs/ARCHITECTURE.md` after the final diff.
6. Update the route table so Hub, Account, and Deal Room are described as lazy. Document the new
   bundle-size check and its ordering after a build. If Phase 3 is used, describe the Deal Room
   timeline feature boundary at the appropriate level without turning architecture documentation
   into a chunk manifest.
7. Run `git diff --check`, inspect the final diff for generated `dist`, source maps, secrets,
   accidental package/lockfile churn, and unrelated formatting, then re-run
   `git status --short`.

No backend Cargo or Tauri Cargo gate is required unless implementation expands into those build
roots. `npm run build:desktop` is not needed for this ordinary UI composition change unless a
packaged-app-only chunk resolution problem is found.

## Test matrix

| Scenario | Expected behavior |
| --- | --- |
| Fresh `/login` load | Only boot/router/theme/login and their true shared dependencies load; no workspace or PDF code |
| Login to `/hub` | Hub chunk loads once, route state is retained, and workspace shell renders |
| Direct `/hub/account` | Account chunk resolves and existing missing-email redirect/account states remain intact |
| Direct `/hub/deals/:dealId` | Deal Room chunk resolves; params, extraction state, loading, and missing-deal redirect remain intact |
| Deal Room Timeline | If Phase 3 is used, timeline code loads on first selection while shell remains usable |
| Data Room without preview | Data Room route loads without pdf.js worker or preview chunk |
| Open document preview | Preview chunk and worker load, then PDF/text loading and error states remain unchanged |
| Deals table | Deals route loads without Kanban chunk |
| Select Kanban | Kanban chunk loads and keyboard behavior remains intact |
| Web production build | No >500 kB chunk warning; entry budget and web boundary check pass |
| Desktop UI production build | No >500 kB chunk warning; entry budget passes and hash routes resolve |

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| More lazy routes create a visible blank flash | Use one accessible, theme-aligned route fallback and inspect cache-disabled navigation |
| Deep links fail after splitting | Verify direct/refresh navigation in BrowserRouter and direct hash navigation in desktop mode; retain current hosting rules |
| Router/location state is lost | Keep route paths and navigation calls unchanged; add focused lazy-route tests around existing state branches |
| Chunk-load errors become opaque | Keep module-load failure distinct from page data loading and inspect console/network failure behavior |
| Shared imports pull workspace code back into entry | Use the Rollup module report and network waterfall as acceptance evidence, not filenames alone |
| Manual vendor splitting hides rather than fixes the issue | Defer `manualChunks`; compare initial requested bytes before accepting any later cache-oriented split |
| Many tiny chunks increase request overhead | Split only at routes and expensive inactive features; stop once budgets pass |
| PDF worker is optimized accidentally as part of entry work | Treat it as a separate lazy worker asset and only verify its request timing |
| Active user changes distort the baseline | Measure and implement against the same preserved working tree; do not revert or reformat unrelated frontend files |
| A size check reads the wrong target after `dist` is overwritten | Run it immediately after each build and include the build mode in its invocation/report |

## Explicit non-goals

- Raising `chunkSizeWarningLimit`.
- Replacing Vite/Rollup or changing the React canary.
- Adding a general bundle-analyzer dependency without evidence it is needed permanently.
- Replacing `react-pdf`/pdf.js or reducing the PDF worker payload.
- Splitting every component, icon, modal, card, or Radix primitive.
- Refactoring workspace fixtures, application state, API adapters, backend routes, or Tauri IPC.
- Adding route prefetch/preload behavior before navigation latency is measured.
- Treating a warning-free build as proof of improved startup without checking the actual initial
  request graph.

## Definition of done

- Web and desktop UI builds complete without the Vite chunk-size warning.
- Both initial entry chunks pass a checked-in budget with meaningful headroom below 500 kB.
- Cache-disabled runtime inspection proves `/login` no longer requests workspace, Deal Room,
  document preview, pdf.js worker, or Kanban implementation code.
- Every affected route works through web and desktop router composition with stable route state,
  loading, redirects, focus, and error behavior.
- Focused tests and the full frontend gates pass.
- `docs/ARCHITECTURE.md` accurately describes the final eager/lazy route map and verification
  command.
- No generated `dist`, source maps, secrets, package/lockfile churn, or unrelated user changes are
  included.
- `git diff --check` is clean and the final status is reviewed against the recorded dirty-tree
  baseline.
