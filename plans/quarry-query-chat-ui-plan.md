# Quarry Assistant chat UI plan

Status: proposed

Baseline: live working tree verified 2026-09-15, including the in-progress send-query SSE
contract and web/desktop transport work

Depends on: [Quarry send-query SSE plan](quarry-send-query-sse-plan.md)

Scope: replace the active `/hub/summarize` leaf with an assistant-ui-powered `/hub/assistant`
page while preserving Quarry's existing workspace sidebar, header rail, provider lifetime, and
web/desktop runtime boundary.

## Requested outcome

Clicking the existing **Assistant** item in the Research section opens a chat page at
`/hub/assistant`.

The reference screenshot applies only to the area below Quarry's current header. Preserve the
existing workspace chrome exactly:

- keep the Quarry home sidebar and its Research > Assistant item;
- keep the current 40-pixel `WorkspaceHeader` rail and its existing `title="Summarize"` value;
- do not add the screenshot's separate thread-list sidebar, New thread control, sign-in footer,
  or another page header;
- keep the inset `WorkspaceMain` surface, current theme tokens, and shared React route tree.

Below that header, the page has two visual modes.

### Empty mode

- Center a compact `How can I help you today?` heading and a wide multiline composer in the
  available route body.
- Put a small set of Quarry-relevant starter prompts below the composer, using assistant-ui
  suggestion primitives. Suggestions send the displayed prompt through the same normal composer
  path; they are not a second request mechanism.
- Use the reference image for spacing, proportions, quiet borders, rounded corners, and overall
  visual density rather than copying its model branding or unrelated controls.
- Keep the one real composer mounted. Do not render a decorative empty-state input and replace it
  later, because that would lose draft, focus, selection, and IME state.

### Active thread mode

On the first send, one coordinated transition must occur:

1. The same composer smoothly moves from the centered empty position to a persistent dock at the
   bottom of the route body.
2. The submitted user message appears immediately as a compact, right-aligned message at the top
   of the transcript.
3. An assistant typing/loading indicator appears below it while the stream is connecting and
   before the first non-empty text snapshot.
4. Assistant Markdown renders incrementally as cumulative text arrives.
5. The authoritative completion replaces any partial buffer without a visual reset.

Later user turns follow the same top-anchored behavior: the newest user message scrolls to the top
of the thread viewport, its assistant response streams below it, and the composer remains visible
at the bottom of the inset page.

## Route migration and stable contracts

The implementation must establish the new route identity while preserving the surrounding
contracts:

- route path `/hub/assistant`; remove `/hub/summarize` from the active route manifest rather than
  keeping two URLs for the same page;
- sidebar label `Assistant`, its order, and icon;
- rename the home-sidebar selection key from `"summarize"` to `"assistant"` across
  `ActiveHomeSection`, the route page, sidebar tests, and any other typed consumer;
- `WorkspaceProvider` outside the child `Routes`, so child navigation does not refetch workspace
  deals;
- the existing `page("Loading Assistant", ...)` leaf lazy boundary and outer `/hub/*` lazy
  boundary;
- the current `WorkspaceHeader title="Summarize"` header and rail layout;
- web `BrowserRouter`, desktop `HashRouter`, and the shared `frontend/src/` product tree;
- page code importing only `@quarry/runtime`, never raw `fetch`, `EventSource`, or Tauri APIs;
- `QuarryApi.queryModel({ files: [], prompt }, handlers)` as the only model request;
- server-owned defaults by omitting `model` and `systemInstructions`;
- the existing `POST /api/v1/query_model` contract and SSE event names;
- the legacy Summarize supporting components, workflow, data modules, and summary APIs after the
  route module is renamed and replaced with the Assistant chat page.

The visible transcript remains page-local and ephemeral. Completed user/assistant pairs may be
copied into a bounded request context snapshot, but the transcript is not authentication state or
a durable conversation and must not be written to fixtures, `sessionStorage`, the activity log,
SQLite, or Helix.

## Prerequisite gate

Do not begin the page integration until the predecessor plan is complete and the live tree proves:

- `QueryModelInput`, `SendQueryEvent`, `SendQueryEventHandlers`, and `QuarryApi.queryModel` exist in
  `frontend/src/contracts/quarryApi.ts`;
- browser multipart POST-SSE parsing and cancellation are covered under `frontend/tests/api`;
- desktop TypeScript and Tauri Rust send/cancel relay paths are covered and cancel upstream work;
- `POST /api/v1/query_model` is owned by the backend assistant domain and streams the documented
  event union;
- OpenAI parsing, backpressure, cancellation, no-delta completion, timeout, and sanitized failure
  behavior are tested without a live provider;
- `docs/ARCHITECTURE.md` describes the implemented query interaction and
  `OPENAI_QUERY_MODEL` configuration.

If any prerequisite is missing, finish it in the SSE plan. Do not add a page-local transport,
alternate `/api/chat` endpoint, browser-only adapter, or direct Tauri call.

## assistant-ui integration decision

Use assistant-ui as the chat runtime and UI foundation, not merely as visual inspiration.

The live 2026-09-15 documentation and registry establish the intended composition:

- [`LocalRuntime`](https://www.assistant-ui.com/docs/runtimes/custom/local-runtime) is the custom
  backend path where assistant-ui owns message, running, cancellation, and thread state while the
  application provides a `ChatModelAdapter`.
- [`ThreadPrimitive`](https://www.assistant-ui.com/docs/api-reference/primitives/thread) owns the
  scrollable transcript, top turn anchoring, measured sticky viewport footer, messages, and
  scroll-to-bottom behavior.
- [`ComposerPrimitive`](https://www.assistant-ui.com/docs/api-reference/primitives/composer) owns
  multiline input, send, cancel, keyboard, disabled, and composer state.
- [`MessagePrimitive`](https://www.assistant-ui.com/docs/api-reference/primitives/message) owns
  user/assistant message context, parts, streaming status, and message error placement.
- [`AuiIf`](https://www.assistant-ui.com/docs/api-reference/primitives/assistant-if) is the current
  conditional primitive; do not use deprecated `ThreadPrimitive.If`, `MessagePrimitive.If`, or
  `ComposerPrimitive.If` APIs.
- The current assistant-ui registry `Thread` already composes a centered welcome state, a single
  composer, top-anchored messages, a sticky `ThreadPrimitive.ViewportFooter`, cancel action,
  loading indicator, and scroll-to-bottom control.

At implementation time, install the current React-compatible `@assistant-ui/react` release and
lock the resolved version. The package version observed while updating this plan was `0.15.20`,
whose published peer range includes React 19, but the manifest and lockfile remain the authority
on implementation day.

Do not add `@assistant-ui/ai-sdk`, Vercel AI SDK packages, Assistant Cloud, or an `/api/chat`
route. Quarry already has a custom cross-platform SSE contract, so `useLocalRuntime` plus a
`ChatModelAdapter` is the narrow compatible integration.

### Required assistant-ui component map

All chat-specific interaction surfaces must be built from assistant-ui's runtime, primitives, and
reviewed registry sources. Quarry owns only the transport bridge, layout styling, and product
copy.

| UI responsibility | assistant-ui source |
| --- | --- |
| Runtime context and message lifecycle | `AssistantRuntimeProvider`, `useLocalRuntime`, `ChatModelAdapter` |
| Thread and scroll behavior | `ThreadPrimitive.Root`, `Viewport`, `Messages`, `ViewportFooter`, `ScrollToBottom` |
| Empty/thread conditional composition | `AuiIf` selectors over `thread.isEmpty`, `thread.isRunning`, and message/part status |
| Composer | `ComposerPrimitive.Root`, `Input`, `Send`, and `Cancel` |
| Starter prompts | `Suggestions(...)` configuration plus `ThreadPrimitive.Suggestions`, `SuggestionPrimitive.Trigger`, `Title`, and `Description` |
| User and assistant turns | `MessagePrimitive.Root`, `Parts` or `GroupedParts` |
| Incremental Markdown | assistant-ui's `MarkdownText` registry component backed by `@assistant-ui/react-markdown` and `remark-gfm` |
| Pre-first-token state | assistant-ui `elements-typing-indicator` rendered for the synthetic empty running text part |
| Error state | `MessagePrimitive.Error` and `ErrorPrimitive` with a real assistant-ui reload/retry action |
| Stop action | `ComposerPrimitive.Cancel`; incomplete/cancelled status remains visible with partial text |
| Copy/retry actions | assistant-ui action primitives, only where their underlying capability is enabled |

Do not hand-roll parallel message, composer, cancel, scroll-follow, or loading components. Do not
import every component in the assistant-ui catalog: model selection, attachments, voice, tools,
reasoning, branching controls, persistence, and thread lists are capabilities, not decoration,
and must remain absent until Quarry supports their contracts.

### Registry adoption procedure

The registry is mutable. Inspect it with the repository-installed CLI before changing files:

```sh
cd frontend
npm exec shadcn -- add @assistant-ui/thread --dry-run
npm exec shadcn -- add @assistant-ui/thread --diff
npm exec shadcn -- view @assistant-ui/thread
npm exec shadcn -- add @assistant-ui/elements-typing-indicator --dry-run
npm exec shadcn -- add @assistant-ui/elements-typing-indicator --diff
```

The 2026-09-15 dry run proposed 20 files, six overwrites of customized Quarry primitives, and
unused attachment, image, reasoning, tool, and follow-up dependencies. Therefore:

- never run the registry add with `--overwrite`;
- retain Quarry's existing Button, Textarea, Tooltip, Avatar, Dialog, Skeleton, and theme tokens;
- adapt the reviewed `thread.aui.tsx`, Markdown renderer, typing indicator, and only their actually
  used helpers into `frontend/src/components/assistant-ui/`;
- remove unsupported attachment, voice, model, reasoning, tool, edit, branch, export, and dynamic
  follow-up branches from the adopted Thread source;
- keep assistant-ui primitive composition intact rather than replacing it with Quarry-owned
  equivalents;
- preserve source provenance in comments;
- add direct dependencies only when an imported retained file needs them, and review every peer
  dependency against the React canary before accepting lockfile changes.

Use the assistant-ui Markdown component instead of maintaining a parallel Quarry chat Markdown
component. Keep raw HTML disabled and preserve safe link behavior. The legacy Summarize renderer
may continue using its current packages independently.

## Quarry-to-assistant-ui transport bridge

Add a small `ChatModelAdapter` that converts assistant-ui's async-generator contract into
Quarry's callback-based `queryModel` contract.

The adapter must:

1. Read only the final submitted user message and extract its text content.
2. Reject empty or non-text-only input before starting transport.
3. Call `runtime.api.queryModel({ files: [], prompt }, handlers)` with `model` and
   `systemInstructions` absent.
4. Convert callbacks into an async queue owned by that one `run` invocation.
5. Accumulate ordered `delta` values and yield the full cumulative assistant text on every
   update, because assistant-ui replaces the prior yielded content rather than appending deltas.
6. Keep the assistant message empty but running after submit and `started`, allowing the
   assistant-ui typing indicator to render until the first non-empty text snapshot.
7. On `completed`, yield `event.response` if it differs from the last snapshot, then close the
   generator. The completion payload is authoritative even after prior deltas.
8. Map server `failed` and `onConnectionError` events to sanitized thrown errors so assistant-ui
   marks the assistant message `incomplete` with reason `error` and its error primitive renders.
9. Subscribe to the adapter's `abortSignal`; abort must invoke the Quarry cleanup function once,
   end quietly, preserve partial text, and let assistant-ui own cancelled status.
10. Release the abort listener and invoke cleanup exactly once from `finally`, including unmount,
    error, synchronous terminal callback, and user cancellation paths.

Handle the synchronous-callback edge explicitly: a test adapter may invoke `completed`, `failed`,
or `onConnectionError` before `queryModel` returns its cleanup function. Record terminal state,
then immediately dispose the returned cleanup rather than retaining a settled stream.

Each assistant-ui run gets its own queue and cleanup closure, so late callbacks from a closed run
cannot write into a later run. Do not add a second page-level reducer or duplicate assistant-ui's
message/running state.

Although assistant-ui passes visible thread history to `ChatModelAdapter.run`, the Quarry adapter
must build an explicit `context` snapshot containing only completed prior user/assistant pairs.
Exclude the current user message, pending or partial assistant output, failed/stopped turns, and
all assistant-ui metadata. When the full completed history exceeds the transport limits, send the
newest suffix of whole pairs that fits, never split a pair or message, and show a non-blocking
notice that older context was omitted. Retry must reuse the same context snapshot as the original
attempt.

## Layout and motion contract

### Fill the area below the unchanged header

The chat must be the sole scroll owner below `WorkspaceHeader`:

- add a narrow `contentMode="fill"` option to `WorkspaceLayout` and pass it through
  `WorkspaceHomeShell`;
- keep the existing default page-scrolling classes unchanged for every other route;
- in fill mode, make the content container `min-h-0 flex-1 overflow-hidden p-0` and give the
  assistant-ui thread `h-full min-h-0`;
- keep Demo and workspace-error notices as shrinking-safe siblings above the thread viewport,
  with their existing semantics and actions;
- only `ThreadPrimitive.Viewport` scrolls; the page must not gain a second vertical scrollbar;
- use `ThreadPrimitive.ViewportFooter` for the docked composer so assistant-ui can measure its
  height in scroll calculations.

### Center-to-bottom composer transition

Start from the current assistant-ui Thread registry pattern: the same footer participates in the
centered empty layout and becomes sticky at the bottom when the thread is non-empty.

Use the already-installed `motion` dependency only as a layout-animation wrapper around that
single assistant-ui composer/footer host:

- keep a stable element identity and `layoutId` across empty and active modes;
- animate position, width, and border-radius with a short restrained layout transition;
- do not clone, portal, unmount, or replace `ComposerPrimitive.Root` during send;
- do not animate transcript height or streamed tokens in a way that causes reflow/flicker;
- under `prefers-reduced-motion: reduce`, disable interpolation and switch positions immediately;
- keep the composer usable by keyboard and preserve focus/selection throughout the move.

The existing outer/leaf React View Transitions remain responsible for route navigation only; do
not key or restart them for each message.

### Message placement and streaming

- Set `turnAnchor="top"` on `ThreadPrimitive.Viewport` so assistant-ui registers the newest user
  turn and its running assistant response as the top-anchor pair.
- Style `MessagePrimitive.Root` for user messages with a bounded max width, right alignment, and a
  subtle semantic-token surface.
- Render assistant messages as open left-aligned content, not a mirrored bubble.
- Detect assistant-ui's synthetic empty running text part (`status.type === "running"` and empty
  text) and render the reviewed `TypingIndicator` with an accessible status label.
- When text exists, render assistant-ui `MarkdownText` directly from the streaming part. Do not
  replay or independently reveal already-streamed words.
- Use assistant-ui's pinned-scroll behavior. Follow new output only while the reader is already at
  the bottom, and expose `ThreadPrimitive.ScrollToBottom` when unread content is below.
- Keep partial content visible after cancellation or failure.
- Never announce each delta. Announce only coarse connecting, completed, failed, and stopped
  state changes.

## Composer and suggestion behavior

- `ComposerPrimitive.Input` is multiline with placeholder `Send a message...`.
- Enter submits; Shift+Enter inserts a newline; composition/IME Enter never submits.
- Blank or whitespace-only content does not send.
- The send button has an accessible `Send message` name.
- While running, assistant-ui swaps Send for `ComposerPrimitive.Cancel`, accessible as
  `Stop generating`; do not enable message queueing in this release.
- The runtime prevents duplicate submission while running.
- Configure three to six concise Quarry-relevant starter prompts with assistant-ui's static
  `Suggestions(...)` API and render them through suggestion primitives.
- Store starter-prompt copy in a clearly named feature configuration module. It is product copy,
  not authoritative server data and not an error fallback.
- A suggestion uses `send` and follows the identical message/adapter path as typed input.

Do not show the reference image's plus button, model/effort selector, provider logo, or microphone.
Those controls would imply attachment, model-catalog, reasoning-effort, or dictation contracts
that Quarry does not yet provide. The page should match the reference composition without dead or
misleading affordances.

## Visible states

| State | Required UI | assistant-ui owner |
| --- | --- | --- |
| Empty | Centered greeting, composer, starter prompts | thread/composer/suggestion primitives |
| Connecting | Top-right user turn, docked composer, assistant typing indicator | optimistic message + running empty part |
| Streaming | Incremental assistant Markdown below the user turn | `ChatModelAdapter` cumulative yields + message parts |
| Completed | Final authoritative Markdown and copy/retry actions where enabled | runtime completion status + action primitives |
| Server failed | Partial text retained, sanitized inline error, retry | incomplete/error status + error/reload primitives |
| Connection failed | Partial text retained, distinct sanitized transport error, retry | incomplete/error status + error/reload primitives |
| Stopped | Partial text retained, stopped status, composer re-enabled | cancel action + incomplete/cancelled status |

Focus returns to the composer after completion, failure, or stop only when doing so does not steal
focus from transcript selection, the scroll-to-bottom control, or another intentional target.

## Planned file changes

| Path | Change |
| --- | --- |
| `frontend/package.json`, `frontend/package-lock.json` | Add the reviewed assistant-ui runtime/Markdown dependencies and only the retained registry helpers. |
| `frontend/src/app/WorkspaceRoutes.tsx` | Replace the `SummarizePage` lazy import/target with `Assistant` from `pages/Assistant.tsx`, change the child path from `summarize` to `assistant`, and keep the existing loading label and provider composition. Do not keep a second active `/hub/summarize` route. |
| `frontend/src/pages/SummarizePage.tsx` -> `frontend/src/pages/Assistant.tsx` | Rename the route file, replace the former summary-screen composition with the thin Assistant chat page, export `Assistant`, and use `WorkspaceHomeShell activeHomeSection="assistant"` with the unchanged `WorkspaceHeader title="Summarize"` and fill mode. |
| `frontend/src/components/hub/sidebar/HomeWorkspaceSidebar.tsx` | Change only the Assistant item's `href` from `/hub/summarize` to `/hub/assistant`; preserve its label, icon, position, and navigation-state behavior. |
| `frontend/src/components/hub/sidebar/sidebarTypes.ts` | Replace the `ActiveHomeSection` member `"summarize"` with `"assistant"`. |
| `frontend/src/components/chat/QueryChatRuntimeProvider.tsx` | Build `useLocalRuntime`, install static suggestions, and provide `AssistantRuntimeProvider`. |
| `frontend/src/components/chat/queryModelAdapter.ts` | Bridge Quarry callback SSE into an assistant-ui cumulative async generator with cancellation and cleanup safety. |
| `frontend/src/components/chat/QueryChat.tsx` | Compose the adapted assistant-ui Thread and the stable layout-animation host. |
| `frontend/src/components/assistant-ui/elements/thread.aui.tsx` | Reviewed, scoped adaptation of the current assistant-ui Thread registry source using runtime primitives and no unsupported capability branches. |
| `frontend/src/components/assistant-ui/elements/markdown-text.tsx` | Retained assistant-ui Markdown part renderer with raw HTML disabled. |
| `frontend/src/components/assistant-ui/elements/typing-indicator.tsx` | Retained assistant-ui pre-first-token indicator. |
| `frontend/src/components/assistant-ui/elements/*` | Only helpers actually imported by the three retained files; preserve provenance. |
| `frontend/src/components/hub/WorkspaceLayout.tsx` | Add a default-preserving fill content mode so the Thread viewport is the only scroll owner. |
| `frontend/src/components/hub/WorkspaceHomeShell.tsx` | Pass through fill mode and compose any deals notice above a shrinking-safe chat child. |
| `frontend/src/components/chat/queryChatSuggestions.ts` | Typed Quarry starter-prompt copy for assistant-ui `Suggestions(...)` configuration. |
| `frontend/src/index.css` | Chat-scoped semantic-token and reduced-motion rules only. |
| `frontend/tests/components/chat/queryModelAdapter.test.ts` | Adapter event ordering, authoritative completion, errors, abort, synchronous terminal, and cleanup tests. |
| `frontend/tests/components/chat/QueryChat.test.tsx` | Empty/thread layout, suggestions, composer, transition state, message placement, typing, streaming, stop, error, retry, and accessibility coverage. |
| `frontend/tests/pages/Assistant.test.tsx` | Renamed route module, shell, unchanged header, active Assistant navigation, runtime integration, and notice coexistence. |
| `frontend/tests/app/WorkspaceRoutes.test.tsx` or `frontend/tests/App.test.tsx` | Route cutover and provider-lifetime coverage. |
| `frontend/tests/components/hub/sidebar/HomeWorkspaceSidebar.test.tsx` | Assistant link target and `"assistant"` active-selection coverage with the existing hierarchy unchanged. |
| `frontend/tests/components/hub/WorkspaceHomeShell.test.tsx` | Default layout preservation and fill-mode notice behavior. |
| `docs/ARCHITECTURE.md` | After implementation, document the active Assistant page, assistant-ui LocalRuntime boundary, ephemeral state, layout ownership, and limitations. |

Explicitly unchanged:

- `frontend/src/App.tsx` and the outer `/hub/*` lazy boundary;
- `frontend/src/app/WorkspaceProvider.tsx` and workspace request lifetime;
- the current header rail height and `WorkspaceHeader title="Summarize"` copy;
- sidebar label, icon, order, and navigation-state behavior; only its route target and typed active
  key change;
- `frontend/src/components/summarize/*`, `frontend/src/hooks/useSummarizeWorkflow.ts`,
  `frontend/src/data/summarize.ts`, and the summary APIs; only the former route module is renamed
  and replaced;
- the query contract, web/desktop adapters, Tauri relay, and backend after the prerequisite is
  complete.

## Implementation sequence

### Phase 0 — Confirm prerequisite and live assistant-ui source

1. Record `git status --short` and preserve all user-owned work.
2. Verify the completed SSE contract and focused tests in every transport layer.
3. Confirm the current `/hub/summarize` route, Assistant sidebar target,
   `activeHomeSection="summarize"` selection key, `WorkspaceProvider`, loading label, and exact
   current header title before migrating them together.
4. Inspect the live assistant-ui package peer range and registry `thread`/typing source with the
   installed CLI.
5. Record every proposed file, overwrite, dependency, and CSS mutation before installing or
   adapting anything.

Exit: a delayed synthetic Quarry stream already works in web and desktop, and the assistant-ui
integration needs no transport workaround.

### Phase 1 — Install the runtime and adopt the narrow Thread source

1. Add `@assistant-ui/react` and the retained assistant-ui Markdown dependency through npm.
2. Adapt the reviewed Thread, Markdown, and typing-indicator sources without overwriting Quarry
   primitives.
3. Remove every branch for an unsupported capability while preserving assistant-ui runtime
   primitives for supported behavior.
4. Configure static welcome suggestions through assistant-ui's `Suggestions(...)` API.
5. Run both TypeScript targets and the runtime boundary check.

Exit: the assistant-ui provider/thread compiles in web and desktop, no duplicate UI system or
unused chat capability is present, and existing primitives are unchanged.

### Phase 2 — Bridge the Quarry stream

1. Implement the per-run async queue and `ChatModelAdapter`.
2. Map started/delta/completed/failed/connection-error/cancel into cumulative assistant-ui run
   results and terminal states.
3. Prove authoritative completion replacement and completion-without-deltas.
4. Prove abort, unmount, failure, and synchronous terminal callbacks clean up exactly once.
5. Prove every payload contains the newest original prompt, an immutable bounded snapshot of
   completed prior pairs, `files: []`, and no optional model/instruction keys.

Exit: assistant-ui owns message and run state; Quarry owns only the transport conversion.

### Phase 3 — Compose the reference layout and route

1. Add fill mode to the existing workspace layout with unchanged defaults.
2. Compose the empty greeting, one composer, and suggestions inside assistant-ui Thread.
3. Add the reduced-motion-aware layout transition for that one composer host.
4. Style top-right user turns, open assistant output, typing state, and bottom dock with semantic
   tokens.
5. Rename `pages/SummarizePage.tsx` to `pages/Assistant.tsx`, replace its former summary-screen
   composition with the Assistant chat route, export `Assistant`, and retain the unchanged header.
6. Repoint the leaf lazy import to `Assistant`, change its child path to `assistant`, update the
   sidebar `href` to `/hub/assistant`, and replace the typed active key with `"assistant"`.
7. Verify no active source or route still references `SummarizePage`, `/hub/summarize`, or
   `activeHomeSection="summarize"`; keep the underlying summarize components, workflow, data, and
   API contracts unchanged and compiling.

Exit: clicking Assistant opens the requested page below the unchanged Quarry header in both
targets.

### Phase 4 — Interaction and accessibility coverage

Cover:

- typed submit, click submit, Enter, Shift+Enter, and IME composition;
- blank rejection and duplicate-submit prevention;
- suggestion submit through the same composer/runtime path;
- centered empty mode and one stable composer instance;
- smooth center-to-bottom transition and reduced-motion immediate fallback;
- immediate right-aligned user turn anchored at the top;
- loading indicator before first text;
- multiple cumulative streaming snapshots and authoritative final replacement;
- completion without deltas;
- distinct sanitized server and connection errors;
- cancel retaining partial text and exactly-once cleanup;
- retry through assistant-ui runtime actions;
- ignored late and duplicate terminal callbacks;
- completed-pair context ordering, newest-whole-pair truncation, omission notice, and exclusion of
  incomplete/failed/stopped exchanges;
- conditional focus behavior, accessible names, coarse live status, and scroll-to-bottom;
- safe Markdown with raw HTML disabled;
- no second page scrollbar in no-notice, Demo-notice, and error-notice layouts;
- `/hub/assistant` selecting only `Assistant` while child navigation keeps
  `WorkspaceProvider` mounted;
- `/hub/summarize` no longer being registered as an active product route;
- unchanged sidebar hierarchy and exact header title.

Happy DOM does not prove actual sticky geometry, scroll anchoring, animation smoothness, viewport
fit, or production chunk behavior. Inspect those in a real browser.

### Phase 5 — Runtime inspection and architecture update

1. Run the full frontend verification ladder and both target-specific size guards.
2. Inspect the page in web and desktop with a delayed disposable provider path.
3. Verify empty, connecting, streaming, complete, stopped, retry, and failure states at narrow and
   wide viewports.
4. Verify keyboard/focus, scrolled-up behavior, reduced motion, semantic-token compatibility, and
   the absence of a second scrollbar.
5. Inspect browser console, network/IPC, and activity log for prompt/response leakage.
6. Update `docs/ARCHITECTURE.md` from the final implemented behavior.
7. Inspect final diff/status and confirm no generated output, unrelated formatting, secret, or
   accidental dependency churn entered the change.

If disposable backend dependencies are unavailable, do not run the backend against valuable local
data. Report live-provider inspection as skipped and rely on synthetic adapter/page tests.

## Verification commands for implementation

Use the actual created test paths:

```sh
cd frontend
npm test -- tests/components/chat/queryModelAdapter.test.ts
npm test -- tests/components/chat/QueryChat.test.tsx
npm test -- tests/pages/Assistant.test.tsx
npm test -- tests/app/WorkspaceRoutes.test.tsx
npm test -- tests/components/hub/WorkspaceHomeShell.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts
npm test -- tests/api/tauriQuarryApi.test.ts
npm test -- tests/platform/runtime.contract.test.ts
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

If route coverage belongs in the existing `tests/App.test.tsx`, run that actual path instead of a
new `tests/app/WorkspaceRoutes.test.tsx`. Run each size check immediately after its matching build
so it cannot inspect stale or wrong-target output.

Inspect both production manifests/chunks to confirm `Assistant` remains lazy, the assistant-ui code
stays in the leaf graph, the former `SummarizePage` route module is absent, and neither target
exceeds checked-in entry or application-chunk budgets.

This phase is frontend-only after the predecessor is complete. If implementation changes the query
contract, HTTP/Tauri adapter, native relay, middleware, or backend to close a prerequisite gap,
move that work back into the SSE plan and run its full cross-runtime verification matrix.

Before handoff:

```sh
git diff --check
git status --short
```

## Architecture documentation impact during implementation

Update the relevant `docs/ARCHITECTURE.md` sections after implementation:

- route map: `/hub/assistant` lazy-loads `Assistant` from `pages/Assistant.tsx`; the sidebar still
  labels it Assistant and `/hub/summarize` is no longer an active route;
- shell: the same Quarry sidebar and `WorkspaceHeader title="Summarize"` remain, while fill mode
  gives the route body to one assistant-ui thread viewport;
- frontend layers: `@assistant-ui/react` LocalRuntime and adapted registry Thread source own chat
  behavior beneath Quarry's page/shell;
- state: assistant-ui owns ephemeral messages, draft, running, cancel, error, and scroll state for
  the page mount;
- transport: a narrow `ChatModelAdapter` bridges cumulative run snapshots to
  `QuarryApi.queryModel` through `@quarry/runtime`;
- request semantics: the newest user prompt is sent with a client-managed snapshot of completed
  prior user/assistant pairs;
- accessibility/motion: assistant-ui primitives provide semantics, with reduced-motion handling
  for the composer layout transition;
- feature maturity: no persistence/resume, multi-thread UI, attachments, voice, model picker,
  tools, retrieval, citations, identity, authorization, rate limits, or quotas;
- legacy behavior: summarize components, workflow, data modules, and APIs remain, but the former
  `SummarizePage.tsx` route module is renamed/replaced and the old summary screen has no route;
- verification: adapter/component/page/route/layout coverage and both target bundle guards.

No ADR is expected because this plan preserves the runtime split, public API version, and data
owner.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Registry installation overwrites Quarry primitives or imports the full demo stack. | Dry-run/diff/view first, never use `--overwrite`, and retain only reviewed Thread/Markdown/typing source plus imported helpers. |
| assistant-ui and Quarry both own message state. | Use `useLocalRuntime` as the single chat state owner; keep only a per-run transport queue outside it. |
| Callback SSE does not match assistant-ui's generator contract. | Bridge deltas to cumulative snapshots, make completion authoritative, and test synchronous terminal/abort/finally cleanup. |
| LocalRuntime passes full visible history and UI metadata to the adapter. | Derive an explicit immutable context snapshot from completed user/assistant pairs only; test ordering, exclusion, retry stability, and newest-whole-pair truncation. |
| Centered and docked composers become two DOM instances. | Keep one `ComposerPrimitive.Root` with stable identity and animate its layout host. |
| First send snaps instead of moving smoothly. | Use Motion layout interpolation around the stable assistant-ui footer and verify in a real browser, with reduced-motion fallback. |
| Parent page and Thread both scroll. | Add a default-preserving workspace fill mode and make only `ThreadPrimitive.Viewport` scroll. |
| Demo/error notices hide the composer. | Compose notices as fixed siblings above a `min-h-0 flex-1` thread and test all notice states. |
| Loading indicator persists beside actual output. | Render it only for the synthetic empty running part; switch to Markdown on the first non-empty snapshot. |
| Streamed Markdown flickers or replays. | Yield cumulative state exactly once per Quarry event and render assistant-ui Markdown directly without a second reveal effect. |
| Unsupported controls imply capabilities Quarry lacks. | Omit thread list, model, attachment, voice, tool, reasoning, edit, and branch UI until their contracts exist. |
| New dependencies consume bundle headroom. | Keep the page lazy, inspect both manifests, run both size guards, and do not raise budgets to absorb the change. |
| Prompt or response content enters diagnostics. | Add no page logging/persistence and retain transport redaction tests. |

## Out of scope

- A second assistant thread-list sidebar, New thread action, persistence, history, replay, reconnect,
  resume, or multiple simultaneous runs.
- Changing the current header title, header rail, Quarry sidebar structure, route loading label,
  `WorkspaceProvider`, or `App.tsx` beyond the explicitly planned `/hub/assistant` route migration.
- Model/effort selection, attachments, drag/drop, paste upload, previews, dictation, speech,
  provider branding, or system-instruction editing.
- Server-side conversation persistence, provider conversation IDs, automatic compaction, or
  context derived from incomplete/failed/stopped turns.
- Branching, message editing, variants, feedback, citations, tools, reasoning panels, image
  generation, retrieval, web search, or file search.
- Editing, moving, deleting, or modernizing legacy summarize components, workflow, data modules,
  or summary API contracts beyond renaming/replacing the route module itself.
- Replacing Quarry's web/desktop query transport with an AI SDK or assistant-ui wire protocol.
- Backend, Tauri, configuration, SQLite, Helix, or authentication work beyond the completed SSE
  prerequisite.

## Definition of done

- Clicking Research > Assistant opens `/hub/assistant` and lazy-loads `Assistant` from
  `frontend/src/pages/Assistant.tsx` in web and desktop.
- `/hub/summarize`, `SummarizePage`, and the `"summarize"` home-section key are removed from the
  active route/navigation contract; the Assistant link, route, component export, and
  `"assistant"` active key agree.
- The existing Quarry sidebar, 40-pixel header rail, and `Summarize` header title are unchanged;
  the reference composition begins below them.
- Empty mode shows a centered assistant-ui greeting, real composer, and Quarry starter prompts.
- The first send smoothly moves that same composer to the bottom, immediately anchors the user
  message at the top right, shows a typing indicator, and streams assistant Markdown below it.
- Reduced motion switches the composer position without animation.
- assistant-ui LocalRuntime/primitives own message, composer, running, cancel, error, suggestion,
  scrolling, and action behavior; Quarry owns only the adapter, copy, and styling.
- The newest original prompt is sent with `files: []`, optional model/instruction keys absent, and
  a bounded immutable context snapshot containing only completed prior user/assistant pairs;
  oldest whole pairs are omitted visibly when limits require it.
- Completion is authoritative; no-delta, error, stop, retry, late-event, synchronous-terminal, and
  unmount paths are deterministic and clean up once.
- No unsupported or dead model, attachment, voice, tool, reasoning, thread-list, or persistence
  affordance is visible.
- The Thread viewport is the only route-body scroll owner and the composer remains visible with
  no notice, Demo notice, and workspace-error notice.
- Keyboard, IME, accessible names, focus, coarse live feedback, pinned scrolling,
  scroll-to-bottom, safe Markdown, and partial-output retention are verified.
- Legacy summarize components, workflow, data modules, and APIs remain unchanged and compiling;
  the former summary route UI and `SummarizePage.tsx` module are absent from active production
  chunks.
- Focused tests, full frontend gates, both builds, both size guards, manifest inspection, and
  feasible real-browser web/desktop checks pass or skipped runtime checks are reported precisely.
- `docs/ARCHITECTURE.md` reflects the implemented Assistant page and remaining limitations.
- `git diff --check` is clean and final status review distinguishes this plan/implementation work
  from pre-existing user-owned changes.
