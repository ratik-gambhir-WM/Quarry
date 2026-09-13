# Quarry query chat UI follow-up plan

Status: proposed

Baseline: live working tree verified 2026-09-13, after the frontend route/state decomposition,
workspace data-source hardening, Summarize extraction, and bundle-budget work

Depends on: [Quarry send-query SSE plan](quarry-send-query-sse-plan.md)

Scope: a lazy shared React workspace route, a small Quarry-owned chat state machine, selected
standalone assistant-ui Elements-derived presentation sources, and consumption of
`QuarryApi.queryModel`

Active route: `/hub/summarize`

Legacy page retained: [`frontend/src/pages/SummarizePage.tsx`](../frontend/src/pages/SummarizePage.tsx)

## Outcome

Replace the active Explore route's screen with an Ask Quarry chat surface that submits one prompt
at a time and renders the assistant response incrementally. Preserve the persistent workspace
provider, existing shell, sidebar hierarchy, route path, build-selected runtime boundary, and
outer/leaf lazy View Transition behavior.

The first UI release will:

- keep `/hub/summarize`, Research > Explore, `activeHomeSection="summarize"`, and the existing
  sidebar order/labels;
- lazy-load a new `QueryChatPage` from `app/WorkspaceRoutes.tsx` through its existing
  `page("Loading Explore", ...)`/`LazyPage` composition;
- call only `runtime.api.queryModel({ files: [], prompt }, handlers)`;
- omit `model` and `systemInstructions`, leaving defaults server-owned;
- show the user turn immediately, append ordered deltas, and replace partial text with the
  authoritative `completed.response`;
- allow one active request with visible connecting, streaming, stopped, failed, retry, and
  completed states;
- retain independent single-shot exchanges only for the current page mount without implying that
  earlier turns are model context;
- use safe Markdown rendering through the already-installed `react-markdown` and `remark-gfm`;
- adapt only props-driven standalone assistant-ui Element source, without adopting its runtime,
  provider, AI SDK, or transport;
- work in web and desktop from the same React tree with no raw `fetch`, `EventSource`, or Tauri
  import in page/component code.

Attachments, a model picker, system-instruction editing, persistence, retrieval, citations, tools,
and server changes are outside this follow-up.

## Prerequisite gate

Do not begin UI integration until the predecessor is implemented and the live tree proves:

- `QueryModelInput`, `SendQueryEvent`, `SendQueryEventHandlers`, and `QuarryApi.queryModel` exist in
  `frontend/src/contracts/quarryApi.ts`;
- browser multipart POST-SSE parsing and cancellation are covered in `frontend/tests/api`;
- desktop TypeScript and Tauri Rust send/cancel relay paths are covered and cancel upstream work;
- `POST /api/v1/query_model` is owned by `backend/src/domains/assistant/interactions`, streams the
  documented event union, and has explicit validation/body/timeout/buffering policies;
- OpenAI parsing, backpressure, cancellation, no-delta completion, and sanitized failure tests run
  without live OpenAI;
- `docs/ARCHITECTURE.md` documents the implemented interaction contract and `OPENAI_QUERY_MODEL`.

Run the predecessor's focused and broad gates before changing page code. If a prerequisite is
missing, finish the predecessor rather than adding a page-local fetch, mock production adapter,
direct Tauri call, or alternate chat transport.

## Live baseline and revisions from the earlier plan

| Area | Live working tree on 2026-09-13 | Planning consequence |
| --- | --- | --- |
| Route composition | `App.tsx` keeps Login eager and lazy-loads one `/hub/*` `WorkspaceRoutes` boundary. `WorkspaceRoutes.tsx` mounts a persistent `WorkspaceProvider` and lazy-loads `SummarizePage` at child path `summarize` through `page("Loading Explore", ...)`. The outer View Transition is no longer keyed by pathname. | Replace only the `SummarizePage` lazy target in `WorkspaceRoutes.tsx`. Keep `App.tsx`, the outer workspace boundary, provider lifetime, child path, loading label, page helper, redirects, and all sibling/nested routes unchanged. |
| Workspace state and shell | `WorkspaceProvider` owns session plus loading/API/demo/error deal-resource state across `/hub/*`. `WorkspaceHomeShell` consumes it, supplies the home sidebar, and may prepend a visible Demo or retryable API-error notice. `WorkspaceLayout` is split into `WorkspaceShell`/`WorkspaceMain` and already accepts `contentClassName`; `WorkspaceHomeShell` does not expose that seam. | Render inside the existing provider through `WorkspaceHomeShell` and `WorkspaceHeader`. Account for optional notices in the available-height calculation. Prefer a narrow fill-mode/content-class pass-through on `WorkspaceHomeShell` only if browser geometry proves the default padded scroll surface cannot keep the transcript and composer in one pane; do not add another provider or layout system. |
| Sidebar | Research > Explore targets `/hub/summarize`; `HomeWorkspaceSidebar.test.tsx` locks hierarchy and absence of Quick Chat. | Do not add, rename, reorder, or remove navigation items or change `ActiveHomeSection`. |
| Legacy summarize | `SummarizePage` is now a thin composition over `useSummarizeWorkflow`, `data/summarize`, extracted picker/tree components, lazy `SummaryPanel`, and the inert presentational `ChatPanel`. | Keep the complete legacy Summarize feature tree unchanged, unregistered, and compiling after cutover. Do not mine its workflow or chat-tab state into the new feature. |
| Query transport | No `queryModel`, query route, assistant interaction service, or query Tauri relay exists yet. | The SSE plan remains a hard dependency. |
| Frontend tests | Tests live under `frontend/tests`; the boundary check rejects tests under `frontend/src`. `App.test.tsx` now characterizes the eager Login/lazy workspace boundary, provider fetch lifetime, and nested Deal Room history. | Add query UI tests under `frontend/tests`, mirror production paths, and extend route coverage without weakening the existing workspace-lifetime assertions. |
| UI composition | Existing shared primitives include Button, Textarea, Avatar, Tooltip, Icon, semantic tokens, and shadcn aliases. `shadcn` is already a package dependency. | Inspect with the repository-installed CLI and never overwrite customized `components/ui` files. |
| React/theme | React/React DOM are a 19 canary; route transitions have guarded behavior. The dark palette remains in CSS but the current feature flag forces the active `slate-frost` light mode and hides the picker. | Do not add a second canary dependency or re-enable dark mode. Use semantic tokens so the retained dark palette remains compatible. |
| Markdown and bundles | `react-markdown` and `remark-gfm` are installed behind Summarize's lazy `SummaryPanel`. Both UI builds emit manifests/target stamps; checked-in guards enforce a 350,000-byte entry budget and a 500,000-byte application-chunk ceiling. | Reuse the Markdown packages without raw HTML, keep them lazy with the query route, scope chat styling, and run both target-specific size guards after their matching builds. Do not raise budgets to absorb the feature. |

The earlier plan's `App.tsx` leaf-route edit, pathname-keyed transition assumption, page-owned
workspace data, frontend test paths under `src`, generic dark-mode manual test, and
`npx shadcn@latest` commands are obsolete.

## Route and legacy-page decisions

- Add `frontend/src/pages/QueryChatPage.tsx` and lazy-load it from
  `frontend/src/app/WorkspaceRoutes.tsx` at child path `summarize`.
- Preserve the `page("Loading Explore", ...)` helper, its `LazyPage`/View Transition behavior, the
  outer `LazyPage label="Loading workspace"`, and the unkeyed app-level View Transition.
- Render `WorkspaceHomeShell activeHomeSection="summarize"` with
  `header={<WorkspaceHeader title="Ask Quarry" />}`.
- Replace only the active `WorkspaceRoutes.tsx` lazy import/element for `SummarizePage`; leave
  `App.tsx` and `WorkspaceProvider` composition unchanged.
- Keep `SummarizePage.tsx`, `components/summarize/*`, `hooks/useSummarizeWorkflow.ts`, and
  `data/summarize.ts` in source and compiling under both targets.
- Keep `QuarryApi.summarizePath`, `summarizeSelected`, and `summarizeUpload`, their web/desktop
  mappings, Axum summary routes, and tests. Removing one UI caller is not authorization to remove
  those contracts.
- Add route coverage so `/hub/summarize` cannot accidentally register both old and new pages, and
  so navigating among `/hub/*` leaves the workspace provider mounted rather than refetching deals.
- Confirm the legacy page is absent from production chunks after it loses its only active import;
  do not edit generated `dist` output.

## Interaction model

### Visible states

| State | Visible behavior | Allowed action |
| --- | --- | --- |
| Empty | Compact Ask Quarry introduction and focused multiline composer | Send a non-blank prompt |
| Connecting | User turn, assistant placeholder, and progress text | Stop |
| Streaming | Ordered partial assistant Markdown and unobtrusive progress | Stop; scroll/read remains usable |
| Completed | Authoritative final Markdown | Send another independent prompt |
| Server failed | Partial output remains with the sanitized server message | Retry the exchange or send a new prompt |
| Connection failed | Partial output remains with a distinct transport message | Retry the exchange or send a new prompt |
| Stopped | Partial output remains and is visibly marked stopped | Retry the exchange or send a new prompt |

Do not show disabled attachment, model, system-instruction, voice, feedback, or tool controls.

### State ownership

Use a discriminated reducer rather than unrelated booleans. Suggested concepts:

- a page-local ordered list of exchanges;
- stable user/assistant turn IDs;
- one active request token/generation or `null`;
- per-assistant status (`connecting`, `streaming`, `completed`, `server-failed`,
  `connection-failed`, or `stopped`);
- original prompt retained for retry;
- optional resolved model metadata from `started`;
- draft input owned by the feature hook/component, not duplicated in effects.

The transcript is ephemeral and presentation-local. It is not a conversation record and must not
be stored in fixtures, `sessionStorage`, the activity log, SQLite, or Helix.

### Submit and event mapping

On submit:

1. Test `prompt.trim()` for blank input, but send and retain the original string.
2. Create IDs and the request token before opening the stream.
3. Add the user turn and assistant placeholder atomically, clear the draft, and mark the request
   active.
4. Call `runtime.api.queryModel({ files: [], prompt }, handlers)` with optional keys absent.
5. Store the returned cleanup function in a ref, never render state.

| Callback | Reducer transition |
| --- | --- |
| `started` | `connecting` to `streaming`; record optional resolved-model metadata; do not add a turn. |
| `delta` | Append once and in arrival order to the active assistant turn. |
| `completed` | Replace all partial text with `event.response`, mark completed, and clear the active request. |
| `failed` | Preserve partial text, record the server-safe error, mark server-failed, and clear active state. |
| `onConnectionError` | Preserve partial text, record a transport message, mark connection-failed, and clear active state. |

Every callback must carry/check the request token. Late callbacks from stopped, failed, retried,
superseded, or unmounted work are no-ops.

### Cleanup and retry contract

- Stop invokes cleanup once, marks the current exchange stopped, and restores composer use.
- Route unmount invokes cleanup once without dispatching visible state afterward.
- Terminal callbacks clear the active cleanup ref.
- If a test adapter invokes a terminal callback synchronously before `queryModel` returns, dispose
  the returned cleanup immediately instead of retaining a settled stream.
- If cleanup occurs while desktop file encoding/listener setup is still pending, the adapter owns
  disposal of anything created later.
- Duplicate terminal callbacks or callbacks after cancellation do not mutate the transcript.
- Retry reuses the existing user turn and assistant slot, resets only that assistant result, and
  resends the original prompt. It does not duplicate the user turn.
- A later new prompt contains only that prompt. Earlier displayed exchanges are never concatenated
  or sent as hidden context.

## assistant-ui source adoption

Use assistant-ui only as reviewed presentation source. Select the props-driven standalone
Elements for Composer, Chat Panel, Scroll Anchor, Message Pair, Error State, and Stopped Run. Do
not initialize an assistant-ui runtime/provider, add `@assistant-ui/react`, add an AI SDK, or
create `/api/chat`.

The registry is mutable. On the implementation date, inspect proposals with the repository's
installed `shadcn` CLI rather than requesting `@latest`:

```sh
cd frontend
npm exec shadcn -- add "@assistant-ui/elements-composer" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-composer" --diff
npm exec shadcn -- add "@assistant-ui/elements-chat-panel" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-chat-panel" --diff
npm exec shadcn -- add "@assistant-ui/elements-scroll-anchor" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-scroll-anchor" --diff
npm exec shadcn -- add "@assistant-ui/elements-message-pair" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-message-pair" --diff
npm exec shadcn -- add "@assistant-ui/elements-error-state" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-error-state" --diff
npm exec shadcn -- add "@assistant-ui/elements-stopped-run" --dry-run
npm exec shadcn -- add "@assistant-ui/elements-stopped-run" --diff
```

Proceed only if those names still resolve to standalone props-driven source. Review every proposed
file, package, CSS rule, alias, and collision before installing the selected set:

```sh
cd frontend
npm exec shadcn -- add \
  "@assistant-ui/elements-composer" \
  "@assistant-ui/elements-chat-panel" \
  "@assistant-ui/elements-scroll-anchor" \
  "@assistant-ui/elements-message-pair" \
  "@assistant-ui/elements-error-state" \
  "@assistant-ui/elements-stopped-run"
```

Never use `--overwrite`. If the live registry attempts to replace a customized Quarry primitive,
requires the assistant-ui runtime, or imports unrelated model/tool/attachment features, stop and
adapt only the reviewed source manually behind Quarry-owned components; do not accept the broader
dependency graph.

Keep source provenance in comments and retain low-level adapted files under
`frontend/src/components/assistant-ui/elements/`. Feature code imports only Chat-prefixed wrappers
under `frontend/src/components/chat/`.

| Proposed source | Quarry adaptation |
| --- | --- |
| Composer shell/actions | Reuse structure, Quarry Button/Tooltip/icons/`cn`, and existing Textarea. Do not use a single-line input. |
| Chat Panel and Message Pair | Drive entirely from reducer props; remove internal runtime/reveal state that could replay streamed text. |
| Scroll Anchor | Keep pinned/unseen state page-local; follow only while already at the bottom and provide an accessible jump-to-latest control. |
| Error State and Stopped Run | Map Quarry server-failed, connection-failed, stopped, and retry props without assistant-ui run state. |
| Shared standalone surfaces/range helpers | Retain only helpers actually imported by selected files; keep them out of generic `components/ui`. |
| Markdown Text | Do not install; use `react-markdown`/`remark-gfm`. |
| Model Selector | Do not install; Quarry has no approved client model catalog and this page omits `model`. |

No new direct package is expected. If a retained source imports one, justify it, inspect peer
compatibility with the React canary, and keep `package.json`/lockfile churn focused. Do not add
Command/cmdk solely for the excluded model selector.

## Layout, accessibility, and rendering

- Use a page-local full-height flex column with `h-full min-h-0`; give only the transcript its own
  scroll area and keep the composer visible at the bottom.
- First prove that this fits inside `WorkspaceHomeShell`'s current padded scrolling surface in
  ordinary API-success, explicit-demo, and workspace-error states. `WorkspaceLayout` already has
  a `contentClassName` seam. If browser geometry demonstrates parent scrolling or composer loss,
  expose a narrow fill-mode/content-class option through `WorkspaceHomeShell`, arrange its notice
  as a shrinking-safe sibling above a `min-h-0 flex-1` content region, and add regression tests.
  Do not change the default layout used by existing pages.
- Preserve workspace chrome, the 40-pixel header rail, sidebar, semantic tokens, typography, and
  active `slate-frost` theme. Do not re-enable the currently disabled dark-theme feature.
- Use semantic tokens rather than hard-coded assistant-ui colors so the retained dark palette is
  not broken when restored.
- Render user turns as compact distinct blocks and assistant turns as open Markdown content. Use
  monospace only for code.
- Keep scrolling stable while the reader is above the bottom; do not force every delta into view.
- Scope chat Markdown, code, scrollbar, and Element selectors under chat data slots. Do not alter
  `.vault-markdown`, PDF viewer styles, ReUI tables, or generic code elements.
- Do not enable raw HTML. Preserve safe URL behavior and non-executable fenced code.
- Enter submits; Shift+Enter inserts a newline; composition/IME Enter never submits. A click path
  remains available.
- Use a real label or accessible name for the Textarea and all icon-only actions.
- Disable duplicate submission while active and expose a named Stop action.
- Move focus back to the composer after completion, failure, stop, or retry only when doing so does
  not steal focus from transcript selection or another intentional control.
- Announce coarse states (`started`, `completed`, `failed`, `stopped`) through one polite live
  region. Do not announce every delta.
- Honor `prefers-reduced-motion`; streamed text must not depend on animation for visibility.
- Validate narrow and wide layouts. The page must not create horizontal overflow or a second page
  scrollbar.

## Planned file changes

| Path | Change |
| --- | --- |
| `frontend/src/app/WorkspaceRoutes.tsx` | Replace the `SummarizePage` lazy target with `QueryChatPage` at child path `summarize`, preserving the `page("Loading Explore", ...)` helper, provider lifetime, sibling/nested routes, and redirects. |
| `frontend/src/pages/QueryChatPage.tsx` | Thin route page with existing home shell/sidebar state and Ask Quarry header. |
| `frontend/src/components/chat/QueryChat.tsx` | Compose empty/thread/composer/status views. |
| `frontend/src/components/chat/queryChatState.ts` | Typed exchange model, reducer, request-token checks, and transitions. |
| `frontend/src/components/chat/useQueryChat.ts` | Own runtime subscription, cleanup ref, synchronous-callback safety, retry, unmount, and focus handoff. |
| `frontend/src/components/chat/{ChatComposer,ChatThread,ChatMessage,ChatMarkdown}.tsx` | Chat-prefixed wrappers around adapted Elements and existing Quarry primitives. Combine files when a smaller cohesive implementation is clearer. |
| `frontend/src/components/assistant-ui/elements/*` | Only reviewed standalone source and necessary utilities from the six selected Elements. |
| `frontend/src/index.css` | Chat-scoped Element/Markdown styling using current semantic tokens and reduced-motion rules. |
| `frontend/src/components/hub/WorkspaceHomeShell.tsx` | Change only if measured layout requires exposing the existing `WorkspaceLayout.contentClassName` seam or composing workspace notices above a fill-height child. Preserve current default behavior. |
| `frontend/tests/components/chat/queryChatState.test.ts` | Pure lifecycle/stale-token transition coverage. |
| `frontend/tests/components/chat/QueryChat.test.tsx` | Interaction, keyboard, focus, announcement, streaming, error, stop, and retry coverage. |
| `frontend/tests/pages/QueryChatPage.test.tsx` | Shell/header and runtime integration coverage. |
| `frontend/tests/app/WorkspaceRoutes.test.tsx` or `frontend/tests/App.test.tsx` | Route/lazy composition coverage proving Explore selects only the new page while the workspace provider remains stable across child navigation. Prefer a focused route test if extending `App.test.tsx` would duplicate provider setup. |
| `frontend/tests/components/hub/WorkspaceHomeShell.test.tsx` | Extend only if a fill-height option or notice composition changes; preserve explicit demo/error notice coverage. |
| `frontend/package.json`, `frontend/package-lock.json` | Expected unchanged except for a directly imported dependency proven necessary by live registry inspection. |
| `docs/ARCHITECTURE.md` | Update route, feature maturity, state ownership, UI source boundary, transport consumer, limitations, and test coverage after implementation. |

Explicitly unchanged:

- `frontend/src/App.tsx`, including its eager Login route, lazy `/hub/*` boundary, and unkeyed
  app-level View Transition;
- `frontend/src/app/WorkspaceProvider.tsx` and its request lifetime/data-source contract;
- `frontend/src/pages/SummarizePage.tsx`, `frontend/src/components/summarize/*`,
  `frontend/src/hooks/useSummarizeWorkflow.ts`, and `frontend/src/data/summarize.ts`;
- sidebar labels/order/types and workspace session behavior;
- predecessor contract/transports/backend after its completion;
- existing summary contracts and routes.

## Implementation sequence

### Phase 0 — Prove the transport prerequisite

1. Record `git status --short` and preserve all user-owned work.
2. Verify every prerequisite symbol, route, adapter, Tauri command, cancellation path, and focused
   test from the SSE plan.
3. Run the predecessor's broad backend/frontend/Tauri gates.
4. Confirm `App.tsx` still owns only the lazy `/hub/*` boundary, `WorkspaceRoutes.tsx` still owns
   child path `summarize`, and `WorkspaceProvider` still wraps the child route tree.
5. Confirm Explore, `activeHomeSection="summarize"`, and `page("Loading Explore", ...)` remain the
   live route/navigation contracts.
6. Inspect the six live assistant-ui manifests with the installed CLI without mutating files.

Exit: a delayed synthetic stream already works in both adapters and the UI needs no transport
workaround.

### Phase 1 — Adopt the minimal presentation source

1. Dry-run/diff every selected Element and compare proposed files with existing Quarry primitives.
2. Install/adapt only the six standalone Elements and the helpers they actually import.
3. Keep low-level provenance under `components/assistant-ui/elements`; expose Chat-prefixed,
   props-driven wrappers to the feature.
4. Replace standalone single-line input behavior with the existing Quarry Textarea and explicit
   multiline/IME semantics.
5. Remove model, attachment, voice, suggestion, tool, runtime, and reveal branches not used by
   this release.
6. Add only chat-scoped CSS and run both TypeScript targets before page integration.

Exit: no customized shared primitive was overwritten, no assistant runtime/AI SDK/extra icon or
theme system was added, and the retained source compiles for web and desktop.

### Phase 2 — Build the deterministic lifecycle

1. Define exchange/turn types and a discriminated reducer.
2. Implement submit, started, ordered delta, authoritative completion, both failure classes, stop,
   retry, and stale/duplicate event no-ops.
3. Implement `useQueryChat` around `runtime.api.queryModel`, including exactly-once cleanup,
   synchronous terminal callbacks, unmount, and focus policy.
4. Keep each payload single-shot with `files: []` and optional keys absent.
5. Test the reducer/hook behavior before composing the route.

Exit: every terminal/cancellation path is deterministic, retry does not duplicate a user turn, and
earlier exchanges never enter later payloads.

### Phase 3 — Compose and register the page

1. Build the empty state, thread, messages, Markdown, coarse status/error/stopped views, retry,
   scroll anchor, and composer from props.
2. Create the thin `QueryChatPage` in the existing shell with the Ask Quarry header.
3. Prove the page-local full-height layout inside current `WorkspaceHomeShell` with no notice, the
   explicit Demo notice, and the retryable workspace-error notice. Expose only the existing
   `WorkspaceLayout.contentClassName` seam or a narrow fill option if measurement requires it.
4. Repoint only the `WorkspaceRoutes.tsx` `summarize` lazy target to `QueryChatPage`, preserving
   provider lifetime, child path, loading label, and both View Transition boundaries.
5. Prove no active source imports `SummarizePage` and production builds omit its chunk.

Exit: Explore opens Ask Quarry in both targets, the sidebar and other routes are unchanged, and no
inert future control is visible.

### Phase 4 — Interaction and accessibility coverage

Cover:

- blank/whitespace rejection and duplicate-submit prevention;
- click, Enter, Shift+Enter, and IME composition;
- exact `{ files: [], prompt: originalPrompt }` with optional keys absent;
- immediate user/assistant placeholder rendering;
- synchronous `started` and terminal callbacks;
- multiple deltas before completion and authoritative replacement;
- completion without deltas;
- distinct retryable server and connection failures;
- stop/unmount exactly-once cleanup;
- ignored late callbacks and duplicate terminals;
- retry without duplicate user turn and later-prompt context isolation;
- composer disabled/enabled state, focus behavior, semantic labels, and coarse live updates;
- raw HTML not rendering in Markdown;
- `/hub/summarize` selecting `QueryChatPage`, not `SummarizePage`;
- child-route navigation leaving `WorkspaceProvider` mounted and avoiding a second deals fetch;
- composer visibility with absent, demo, and error workspace notices;
- unchanged sidebar hierarchy and Explore target.

Use synthetic content only. Happy DOM does not prove actual scrolling geometry, sticky layout,
stream timing, View Transition appearance, or responsive rendering.

### Phase 5 — Runtime verification and architecture update

1. Run full frontend gates and inspect manifest/lockfile/bundles.
2. With the predecessor's delayed loopback provider and only disposable backend dependencies,
   inspect first-delta-before-completion, stop, retry, no-delta completion, and post-start failure
   in web and desktop.
3. Inspect console/network/IPC/activity logs for content leakage.
4. Inspect keyboard/focus, reduced motion, active light theme, semantic-token compatibility,
   narrow/wide viewports, scrolled-up behavior, and jump-to-latest.
5. Update `docs/ARCHITECTURE.md` from implemented behavior, then inspect final diff/status.

If disposable Helix/backend dependencies are unavailable, do not run `cargo run` against valuable
local data; report runtime inspection as skipped and rely on synthetic adapter/page tests.

## Verification commands for implementation

Use the exact test paths created under `frontend/tests`:

```sh
cd frontend
npm test -- tests/components/chat/queryChatState.test.ts
npm test -- tests/components/chat/QueryChat.test.tsx
npm test -- tests/pages/QueryChatPage.test.tsx
npm test -- tests/app/WorkspaceRoutes.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts
npm test -- tests/api/tauriQuarryApi.test.ts
npm test -- tests/platform/runtime.contract.test.ts
npm test -- tests/components/hub/WorkspaceHomeShell.test.tsx
npm test -- tests/components/hub/sidebar/HomeWorkspaceSidebar.test.tsx
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run check:bundle-size:web
npm run build:desktop-ui
npm run check:bundle-size:desktop
```

If route coverage is added to the existing `tests/App.test.tsx` rather than a focused
`tests/app/WorkspaceRoutes.test.tsx`, run that actual path instead. `check:web-bundle` and
`check:bundle-size:web` must follow `build:web`; the desktop size check must immediately follow
`build:desktop-ui` so it cannot inspect stale or wrong-target output. Inspect both production
manifests/chunks to confirm `QueryChatPage` remains lazy, `SummarizePage` is unreachable, and the
new route does not breach the checked-in entry or application-chunk budgets.

This follow-up is frontend-only once the predecessor is complete. Do not rerun backend/Tauri Rust
gates solely for page composition. If implementation changes the shared query contract, adapter,
native relay, middleware, or backend to close a prerequisite gap, move that work back into the SSE
plan and run its full cross-runtime matrix.

Before handoff:

```sh
git diff --check
git status --short
```

## Architecture documentation impact during implementation

Update the relevant `docs/ARCHITECTURE.md` sections after implementation:

- route map: `App.tsx` retains the lazy `/hub/*` boundary while `WorkspaceRoutes.tsx` lazy-loads
  `QueryChatPage` for `/hub/summarize` through the existing transition/skeleton;
- feature inventory: Explore is a single-shot streaming assistant interaction UI;
- frontend layers: adapted standalone Element source remains presentation-only beneath
  Chat-prefixed feature components;
- state: transcript, draft, request token, and cleanup are page-local and ephemeral;
- transport: the page consumes `QuarryApi.queryModel` only through `@quarry/runtime`;
- UI/styling: chat uses existing semantic tokens, active light-theme policy, reduced motion, and
  safe Markdown;
- verification: reducer/component/page/route coverage lives under `frontend/tests`;
- bundle architecture: the query page remains a leaf chunk under the existing workspace boundary
  and passes both target-specific size guards;
- known gaps: no conversation context/persistence/resume, attachments, model picker,
  system-instruction UI, retrieval/citations/tools, identity/authorization/rate limits/quotas;
- legacy behavior: `SummarizePage` and summary APIs remain in source but the page has no route.

No ADR is required because this UI plan does not change the runtime split, data owner, public API
version, or destructive rollout.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Registry source overwrites customized primitives or adds a runtime. | Use installed-CLI dry-run/diff, never `--overwrite`, retain only standalone props-driven source, and review every file/package/CSS change. |
| React canary or peer requirements drift. | Do not request latest CLI/packages; inspect the live manifest, keep dependencies minimal, and run both targets/builds. |
| Workspace padding or a Demo/error notice produces nested scroll or a disappearing composer. | Test all three notice states; use the existing `WorkspaceLayout.contentClassName` seam through a narrow `WorkspaceHomeShell` option and keep its current default for other routes. |
| Streaming Markdown flickers or incomplete fences render oddly. | Reuse safe Markdown, render the partial buffer plainly, and replace it with the authoritative completion. |
| Delta updates steal scroll/focus or flood assistive technology. | Follow only when already pinned, expose jump-to-latest, keep focus policy conditional, and announce coarse states only. |
| Cancelled/old streams mutate a new exchange. | Token requests, make cleanup idempotent, and ignore stale/duplicate callbacks. |
| Transcript looks multi-turn although requests are stateless. | Send only the current prompt, state the limitation in UI/architecture, and test payload isolation. |
| Legacy summarize returns to the route or bundle. | Test the route target, search active imports, and inspect both build outputs. |
| Query route work accidentally remounts workspace state or duplicates deal loading. | Change only the leaf target inside `WorkspaceRoutes`; retain the provider outside its inner `Routes` and assert fetch count across child navigation. |
| Chat dependencies consume the remaining entry/chunk budget. | Keep the page lazy, inspect both manifests, and run the target-specific size checks without raising their thresholds. |
| Copied CSS affects other screens or retained dark tokens. | Scope every rule under chat data slots and use existing semantic tokens. |
| Prompt/response reaches diagnostics. | Add no page logging/persistence and retain predecessor redaction/log-content tests. |

## Out of scope

- Editing, deleting, moving, renaming, or modernizing the legacy summarize page/components.
- Removing or changing summary contracts, adapters, Axum routes, or tests.
- Changing `App.tsx`, the `/hub/*` outer boundary, `WorkspaceProvider` lifetime/data-source policy,
  route path, loading labels, sidebar label/order, or `ActiveHomeSection`.
- Attachments, drag/drop, paste upload, or previews.
- Model selection, system-instruction editing, provider settings, or a model catalog.
- Sending prior displayed turns as provider context.
- Conversations, persistence, history, replay, reconnect, resume, or multiple simultaneous runs.
- Branching, edit-and-resend, variants, feedback, citations, reasoning/tool panels, image
  generation, retrieval, web search, or file search.
- Replacing safe Markdown, importing the full Elements directory, or adding assistant-ui runtime,
  provider, AI SDK, or second theme/icon system.
- Backend, Tauri, configuration, SQLite, Helix, or authentication work beyond the completed SSE
  prerequisite.

## Definition of done

- `/hub/summarize` lazy-loads `QueryChatPage` from `WorkspaceRoutes.tsx` through the existing
  outer-workspace and leaf View Transition/LazyPage flow in web and desktop; `App.tsx`, Explore,
  sibling routes, and the sidebar are otherwise unchanged.
- `WorkspaceProvider` remains mounted across workspace child navigation, deals are not refetched
  because of the route swap, and API/demo/error notices coexist with a visible composer.
- Legacy summarize source and APIs remain unchanged, unregistered, compiling, and absent from
  active production chunks.
- The page sends one original non-blank prompt with `files: []` and no model/instruction keys;
  earlier exchanges never become provider context.
- User turns appear immediately; deltas render once/in order; `completed.response` replaces the
  partial buffer; all failure/stop/retry/no-delta/stale/unmount paths are deterministic.
- Only one request is active and cleanup is exactly-once from the UI's perspective.
- Selected standalone Element source coexists with Quarry primitives without overwrites, an
  assistant runtime/provider, an AI SDK, dead affordances, or a parallel style system.
- Keyboard, IME, accessible names, focus, live feedback, scroll preservation, jump-to-latest,
  reduced motion, active theme, retained semantic tokens, and responsive layout are verified.
- No query/provider content appears in logs, storage, URLs, fixtures, screenshots, or generated
  artifacts.
- Focused tests, full frontend gates, both UI builds, both target-specific bundle-size checks,
  manifest inspection, and feasible delayed runtime checks pass or any unavailable manual check
  is reported precisely.
- `docs/ARCHITECTURE.md` reflects the active query page and dormant summarize page.
- `git diff --check` is clean and final status review distinguishes implementation changes from
  user-owned work.
