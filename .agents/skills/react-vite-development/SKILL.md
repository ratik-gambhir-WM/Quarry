---
name: react-vite-development
description: Develop, refactor, debug, review, and verify Quarry's shared React/Vite frontend under frontend/, including its web/desktop build-time composition, product pages, UI components, state, styling, and browser-side API adapters. Use for TypeScript, TSX, CSS, Vite, frontend tests, or shared UI behavior; do not use as the primary guide for backend-only Rust changes.
---

# Quarry React + Vite development

Build the smallest coherent frontend change that works in every affected Quarry distribution and
preserves the repository's active working-tree changes.

## Read before editing

1. Read the root `AGENTS.md` and the relevant frontend/runtime sections of
   `docs/ARCHITECTURE.md`.
2. Run `git status --short`; the frontend commonly contains user-owned tracked and untracked work.
3. Read `frontend/package.json`, `package-lock.json`, `vite.config.ts`, the applicable TypeScript
   configs, the route/feature entrypoint, analogous components, and nearby tests.
4. Confirm the live React/Vite versions. The current working tree uses React 19 canary APIs and an
   uncommitted `.npmrc`; do not normalize, remove, or upgrade them incidentally.

## Preserve Quarry's composition boundary

- `frontend/src/` is the one shared React source tree.
- Vite mode and matching TypeScript configs select `@quarry/router` and `@quarry/runtime`.
- Web uses `BrowserRouter` and `runtime.web`; desktop uses `HashRouter` and `runtime.desktop`.
- Shared product code imports `@quarry/runtime`, never raw `@tauri-apps/*`.
- Raw `fetch` and `EventSource` belong under `frontend/src/api/`.
- Keep `vite.config.ts`, `tsconfig.web.json`, and `tsconfig.desktop.json` aliases aligned.
- Treat every `VITE_*` value as public browser-bundle data. Server secrets never belong there.

Run `npm run check:boundaries` after changing imports, runtime composition, transport code, or
shared files that could pull platform code into the wrong target.

## Design the behavior first

Before implementation, identify:

- the success path and visible loading, empty, error, disabled, retry, and cancellation/stale states;
- the owner of request lifecycle, state transitions, validation, and rendering;
- contracts that must remain stable: routes, router state, exports, props, API methods, storage
  keys, DOM semantics, focus behavior, and platform capability behavior;
- whether the change must work in web, desktop, or both.

Keep independent state local. When values form one workflow, prefer a reducer or discriminated
state over flags that can form impossible combinations. Derive render data rather than syncing
duplicate state with effects. Effects synchronize external systems and must be repeatable,
cancel/clean up correctly, and retain complete dependencies.

## Data and transport rules

- `frontend/src/contracts/quarryApi.ts` is the product-facing transport contract.
- The HTTP and Tauri adapters currently map endpoints separately. An API change requires checking
  both adapters, Tauri relay/native behavior where relevant, Axum routes/DTOs, and contract tests.
- Preserve URL encoding, camelCase JSON, multipart field names/limits, binary/PDF handling, SSE
  event names, and error behavior.
- Do not attach new feature code directly to `runtime.target` when a capability contract or an
  explicit platform composition can express the difference.
- Put shipped mock/demo/placeholder data in explicit files under `src/fixtures/<feature>/` and
  import it into consumers. Do not define product fixture records inline in pages, components,
  hooks, or `src/data`; reserve `src/data` for domain types, mappers, selectors, and other
  non-fixture logic. Keep one-off test inputs inside their tests and shared test fixtures under
  `src/test/fixtures/`.
- Keep fixture imports easy to identify and remove when the API is ready. Do not copy fixture data
  into fallback branches or present a failed request as successful authoritative data; the UI must
  visibly distinguish server state from development fixtures.
- Avoid placing server data already owned by a request/cache/router into duplicate local state
  unless it is an intentional edit buffer.

## UI and accessibility rules

- Reuse `frontend/src/components/ui/`, feature primitives, `components.json` aliases, and semantic
  tokens in `src/index.css` before creating another design system.
- Preserve light and dark themes, responsive behavior, and `prefers-reduced-motion` behavior.
- Prefer semantic elements and native controls. Maintain labels, accessible names, keyboard
  interaction, focus visibility/restoration, and live feedback for async actions.
- Destructive or async actions need clear state and duplicate-submission prevention.
- Keep display components driven by typed props/callbacks and place orchestration in the owning
  page, feature hook, or data module.
- Do not edit generated `dist` output.

## Type and dependency discipline

- Keep strict TypeScript boundaries. Avoid unexplained `any`, `@ts-ignore`, non-null assertions,
  broad lint suppressions, and unsafe casts.
- Use npm and the committed lockfile. Do not mix package managers.
- Add a dependency only when it materially improves the requested outcome and no installed tool
  fits. Keep manifest and lockfile changes focused.
- There is no ESLint/Prettier script. Do not invent `npm run lint` or run a broad formatter across
  the dirty tree.
- Canary-only React features need a guarded/fallback path when the existing code establishes one,
  focused tests, and both target typechecks.

## Verification ladder

Use the narrowest applicable step first, then broaden in proportion to the change:

```sh
cd frontend
npm test -- src/path/to/affected.test.ts
npm run typecheck:web
npm run typecheck:desktop
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

- Use `npm run typecheck` when shared code is affected.
- `npm run check:web-bundle` must follow `npm run build:web` because it inspects `dist`.
- Run the desktop UI build when shared runtime aliases, desktop contracts, or bundle behavior
  changed.
- For native Rust changes, also run the Tauri crate checks from the code-verification skill.

For meaningful UI work, run the appropriate development target and inspect the changed route,
console/network failures, keyboard/focus behavior, relevant viewports, both themes, and each async
state. Test both distributions when platform behavior or shared contracts changed.

## Architecture completion gate

After implementation, re-read the relevant sections of `docs/ARCHITECTURE.md`. Update it in the
same change if routes, feature maturity, state ownership, UI/runtime boundaries, contracts,
transport, platform capabilities, configuration, security posture, verification commands, or
known limitations changed. If no edit is required, hand off with
`Architecture impact: none — <reason>`.

Report exact checks and outcomes, skipped runtime inspection, remaining uncertainty, and
pre-existing failures separately.
