# React Flow branching AI chat implementation plan

Date: 2026-10-27. Status: proposed; no application changes made.

## Outcome

Turn Quarry's existing Assistant into a conversation canvas: prompts and AI answers are connected
nodes, and any completed answer can become the starting point for another line of questioning.
Keep the existing chat runtime, Axum/OpenAI integration, persisted threads, workspace shell, and
Quarry theme. React Flow supplies the canvas interaction and rendering layer.

The supplied image is a visual reference for top-to-bottom conversation lineage, curved edges,
and branching. Its labels are illustrative content, not commands or required product copy.
Keep its spatial model without copying its lavender background, glow, or monospace typography.

Recommended first release: a **Canvas / Thread** switch inside `/hub/assistant`, with Canvas as
the new primary experience after the gates below pass. Both views share one runtime and selected
branch. Keep the floating Data Room/Synthesis Canvas Assistant in its compact thread view initially.

## 1. What already exists and what needs to change

These findings describe the live working tree, including existing uncommitted work.

| Existing implementation | Reuse / required change |
| --- | --- |
| [Assistant.tsx](../frontend/src/pages/Assistant.tsx) owns the provider and workspace shell. | Mount the canvas inside this existing route; preserve All chats/Freeform scope and sidebar tabs. The current code says `Assistant`; an architecture paragraph still describes the older `Summarize` title. Follow code. |
| [AgentRuntime.tsx](../frontend/src/components/chat/AgentRuntime.tsx) owns drafts, selection, runs, retry, cancellation, and thread lists. | Extend this owner rather than creating another chat engine. Currently history is reduced to `activeMessageBranch`, and retry slices the visible message array. Retain all messages before exposing branching. |
| [chatModel.ts](../frontend/src/components/chat/chatModel.ts) has linear UI messages without parent IDs. | Add graph-aware message metadata while preserving existing thread renderer contracts through selectors. |
| [threadModelAdapter.ts](../frontend/src/components/chat/threadModelAdapter.ts) bridges streaming into cumulative text snapshots. | Reuse its terminal-event authority and transport cleanup. Stop inferring a graph run's parent from a global array position. |
| [quarryApi.ts](../frontend/src/contracts/quarryApi.ts) already returns `parentMessageId`, `sequence`, statuses, and stable IDs. `RunAssistantThreadInput` already accepts `parentMessageId`. | No new endpoint or request field is needed for core branching. |
| [HTTP adapter](../frontend/src/api/httpQuarryApi.ts), [Tauri adapter](../frontend/src/api/tauriQuarryApi.ts), and [native relay](../frontend/src-tauri/src/quarry_api/query_stream.rs) already forward the parent. | Exercise existing mappings in contract tests; change transport code only if an actual gap is found. |
| [Chat repository](../backend/src/domains/assistant/chat/repository.rs) persists parent links and sibling assistant retries. | Reuse the schema. Strengthen parent validation and ancestor loading before arbitrary branching. |
| [Chat service](../backend/src/domains/assistant/chat/service.rs) follows parent links in `bounded_provider_context`. | Its input is currently the latest 64 completed thread messages, restricted before a reused prompt on retry. Busy siblings can exclude an old selected branch. Load bounded ancestors of the selected parent instead. |
| [QueryChatThread.tsx](../frontend/src/components/chat/QueryChatThread.tsx), [MarkdownText.tsx](../frontend/src/components/chat/MarkdownText.tsx), and `TypingIndicator.tsx` implement presentation. | Extract reusable composer/message pieces so Canvas and Thread use the same formatting and action behavior. |
| [index.css](../frontend/src/index.css) and [useThemeMode.tsx](../frontend/src/hooks/useThemeMode.tsx) define the theme. | Use semantic tokens. Light mode is currently enforced (`DARK_THEME_ENABLED = false`); preserve dark-token compatibility without re-enabling the picker. |

The frontend manifest/lockfile currently resolve React `19.3.0-canary-ff7445e6-20260831` and Vite 7.
React Flow and a layout engine are not currently installed. Add `@xyflow/react` through npm during
implementation, after checking its then-current peer requirements against the canary. Preserve
the existing dependency policy and review lockfile changes; do not solve a peer conflict by
upgrading React or bypassing checks incidentally.

## 2. The conversation experience

1. An empty canvas shows the existing welcome/composer experience. Sending creates a user node
   and a connected assistant node with visible pending state; the answer streams into that node.
2. Completed answers offer **Continue here**, **Ask another direction**, **Regenerate**, and
   **Copy**. Continue and Ask another direction both create a fresh user child of the selected
   answer; the latter makes the branch choice explicit when that answer already has descendants.
3. One composer is attached visually below the selected answer using a node toolbar or an
   equivalent anchored surface. It is UI state, not a persisted message. On narrow screens it docks
   at the bottom with a visible “Replying to…” preview. Do not mount one textarea per node.
4. Selecting an old answer sets the reply target without deleting its descendants. Sending from
   that target creates another visible branch beside them. Merely selecting a node never calls AI.
5. Regenerate on an assistant answer reuses its parent user prompt and adds another assistant
   child of that user. Existing answers and downstream branches stay intact. Editing an old user
   prompt, if added later, must create a new user sibling rather than mutating recorded history.
6. Compact cards show role, a bounded text preview, status, and actions. A selected card can show
   full Markdown in a detail panel; this keeps long answers readable without enormous graph nodes.
   Use text-derived previews initially; do not make extra AI calls for node titles.
7. A subtle dotted canvas, curved top-to-bottom edges, and highlighted selected ancestry reproduce
   the reference's structure. Add zoom, fit, and “Center selection”; defer the minimap until useful.
8. Keep one generation active per runtime initially. Browsing nodes is allowed during generation,
   but sending/regenerating is disabled. Stop always addresses the actual running node, even if
   another node is selected. Preserve the existing block on thread/scope changes during a run or
   deliberately stop and await cleanup before switching; never let an old stream update a new chat.

“Kept” in the image does not require a new saved/bookmarked status. Omit that feature from the
first release; canonical content is already saved. Branch merging, arbitrary wiring, multi-model
parallel generation, document retrieval, tools, and attachments are also outside this change.

## 3. Message graph and state ownership

Use one persisted message per node; edges come from the child's `parentMessageId`:

```text
user U1
  ├─ assistant A1
  │    ├─ user U2 → assistant A2
  │    └─ user U3 → assistant A3
  └─ assistant A1b   (regenerate U1)
```

The store is a tree/forest with one parent per message, not an arbitrary workflow DAG. Old
threads may contain multiple roots; preserve and display them. React Flow's `parentId` means
visual containment, so do not use it for conversational ancestry.

Extend the runtime with a typed message map and ordered IDs, selected message ID, reply-target
assistant ID, and an active run record containing thread/user/assistant/parent IDs and cancellation
identity. Keep streaming/terminal status as a discriminated state. Store all loaded branches;
derive the Thread view's active ancestry and the canvas projection from this canonical state.
Use stable message IDs for React Flow node IDs and `parent:child` for edge IDs.

Keep canvas-only positions, collapsed subtrees, viewport, and details-panel state separate from
message content. Layout is derived initially; no database migration or graph-layout API is needed.
Remember view state in memory per selected thread for the mounted session. After reload, use
deterministic layout and select the newest valid terminal path by sequence. Clear caches on
identity/scope changes and deletion. Do not put message content in localStorage.

Add explicit runtime operations such as `selectMessage`, `replyTo`, and `regenerate`. For runs,
pass the chosen parent and prompt identity explicitly through `ModelRunOptions`; the persisted
adapter must not infer ancestry from the last element of a graph-wide message collection.
For the legacy ephemeral path, derive only the chosen path before applying existing context limits.

Normal follow-up: new user ID + new assistant ID, parent = selected completed assistant.
Regenerate: existing user ID + new assistant ID, parent = that user's preceding assistant or absent
for a root. Keep assistant ID/request ID stable for replay of the same attempted run; an intentional
regeneration gets a new ID. Update streaming content only on its matching assistant node.

Build graph selectors with missing-parent/cycle guards and deterministic sibling ordering by
sequence. Invalid history should show a recoverable diagnostic, never hang traversal or silently
attach a message to the wrong answer. Existing normalized legacy self-parent records remain supported.

## 4. Backend correctness gate before enabling branch creation

The schema already has parent links and a `(thread_id, parent_message_id)` index. No schema
change is expected for the first release; do not reset SQLite or Helix.

- In the owned thread's start-run transaction, validate that a supplied parent exists in the
  same thread, is an assistant, and is completed. A global foreign key does not establish that
  relationship. Reject self-links, invalid roles, and failed/cancelled/pending parents before
  inserting a new turn. Retain current retry identity/content checks and completed replay behavior.
- Keep multipart/path/identifier validation in the HTTP layer. Enforce persisted relationship
  invariants in the repository transaction and context policy in the service. Return sanitized
  typed errors through the existing boundary.
- Replace thread-global latest-message sampling with bounded parent traversal beginning at the
  requested assistant parent. Resolve complete user/assistant pairs in that same thread, nearest
  first, then reverse for provider order. Use bound queries through the existing SQL builder/client;
  at most 32 complete pairs / 64 messages and the existing character limits. Stop at the bound or
  root, detect cycles, and distinguish invalid ancestry from intentional context truncation.
- Handle legacy self-parent normalization consistently with existing reads/retry compatibility;
  do not rewrite valuable records to make the new traversal work.
- Verify an old branch remains usable even after more than 64 newer sibling messages. Its context
  must contain its ancestors only, never its sibling's text. An absent parent means a new root.
- Retain server prompts, same-owner deal association/reference context, stream status persistence,
  idempotent replay, cancellation, and existing SSE events. A stale `streaming` row after a crash
  remains a known limitation; show “Interrupted / status unavailable” on reload rather than a
  spinner suggesting a live connection. Full stream recovery is a separate feature.

## 5. React Flow integration and theme

Use a feature-local `ReactFlowProvider` under the existing chat provider and a controlled flow
whose message graph comes from runtime selectors. Define memoized custom user/assistant nodes
and stable `nodeTypes` outside render. Nodes contain typed IDs/status/previews and callbacks;
the React Flow store does not own requests or canonical conversation data. This follows React
Flow's [custom-node model](https://reactflow.dev/learn/customization/custom-nodes).

Disable free connection creation, edge reconnection, message deletion shortcuts, and node dragging
for the first release. Edges describe recorded context and cannot be rewired by dragging.
Enable pan/zoom and keyboard selection. Give the canvas a real flex-fill size (`min-h-0` plus a
nonzero height); preserve workspace notices and sidebar layout. Lazy-load the canvas module so
other product routes and the floating thread do not pay for it.

Start with fixed-size preview cards (approximately 300–340px wide, bounded height), top/bottom
handles, about 48px sibling spacing, and 72px between ranks. Implement a small deterministic
tree layout: subtree width is the larger of card width and summed child subtree widths plus gaps;
center each parent over its children. Use iterative/postorder traversal with guards for deep trees.
This restricted tree does not initially need Dagre/ELK. Full Markdown stays in the detail surface,
so streaming cannot change node dimensions. Recompute positions on topology/collapse changes,
not every token; pan only when the user requests it or a new run begins outside the viewport.
Fit once after history loads, not after each delta. Preserve existing node placement where
possible when adding a sibling; animate only when reduced motion allows it.

Import React Flow's required CSS once in a deliberate cascade position and override it within a
canvas wrapper using Quarry variables. Choose base styles if replacing all default decoration;
verify handles, focus outlines, controls, and attribution remain legible. React Flow supports
[CSS-variable theming](https://reactflow.dev/learn/customization/theming).

| Canvas element | Existing Quarry source |
| --- | --- |
| Canvas surface | `--theme-workspace-surface` |
| Answer card | `--theme-panel`, `--theme-panel-border` |
| Prompt card | `--theme-surface-muted`, `--theme-border` |
| Body / muted text | `--theme-text`, `--theme-text-muted` |
| Selected ancestry / edge | `--theme-interactive`, with role/selection labels as well as color |
| Focus | Existing shared focus-ring treatment, verified contrast against adjacent surfaces |
| Error | `--theme-error`, `--theme-error-surface` |
| Typography / controls | Inter/body tokens, shared Button/Tooltip/Skeleton primitives and radii |

Use the existing application theme state; do not let OS detection independently restyle the canvas.
Keep active-path emphasis subtle, skip the reference glow, and avoid a parallel color palette.

## 6. Suggested implementation slices and files

All new paths below are proposals; reuse existing files where their responsibility already fits.

| Slice | Work / exit condition |
| --- | --- |
| 1. Parent/context correctness | Update backend chat repository/service and their existing tests. Old-branch context and invalid-parent tests pass before branch actions ship. No endpoint/schema expansion expected. |
| 2. Graph-capable runtime | Extend `chatModel.ts`, `AgentRuntime.tsx`, and `threadModelAdapter.ts`; add pure graph/path selectors under `frontend/src/data/chat/`. Existing Thread UI behaves as before, while retries retain all siblings internally. |
| 3. Canvas foundation | Add `frontend/src/components/chat/canvas/ConversationCanvas.tsx`, `MessageNode.tsx`, and scoped styles; add pure projection/layout modules under `frontend/src/data/chat/`. Install only `@xyflow/react` initially. Render real saved threads, pending nodes, and all branches. |
| 4. Interaction | Add `BranchComposer.tsx` and `MessageDetails.tsx` in the canvas feature. Extract shared composer/message primitives from `QueryChatThread.tsx`; wire explicit reply/regenerate/stop/copy, selection, and focus. |
| 5. Route rollout | Add Canvas/Thread controls to `Assistant.tsx` under the same provider; keep sidebar, scope, deal badges, previous chats, and compact floating Assistant intact. Verify web and desktop before making Canvas default. |
| 6. Completion | Add behavioral tests and architecture updates, run gates, inspect real browser/Tauri rendering, and review dependency/bundle changes. |

Do not broadly refactor the chat feature or touch unrelated active frontend/backend work. No
new product route is necessary, so router aliases and manifests should remain unchanged.
If illustrative data is needed, place it explicitly under `frontend/src/fixtures/chat/` and mark
it as demo content; use actual persisted threads in the integrated feature and explicit errors
when the backend fails.

## 7. Accessibility, async behavior, and scale

- Preserve built-in keyboard support and customize labels for conversation roles/actions. Avoid
  a node-wide button role around nested controls; keep a visible focus indicator and logical
  traversal order. Provide Thread view as an equivalent reading/composing path. See React Flow's
  [accessibility guidance](https://reactflow.dev/learn/advanced-use/accessibility).
- Apply `nodrag`, `nopan`, and `nowheel` where appropriate to inputs, buttons, and scrollable
  detail content so text selection and scrolling work normally. See [utility classes](https://reactflow.dev/learn/customization/utility-classes).
- Selecting “Continue here” focuses the single composer. Closing details returns focus to its
  node/action. Branch creation announces the new response; live announcements report lifecycle
  transitions rather than reading each token. Escape closes contextual UI without deleting content.
- Preserve loading/history skeleton, retryable history error, thread-create/send error with draft
  retained, provider unavailable, partial failure, stopped response, and context-limit messaging.
  Do not present truncation inferred from global client messages as authoritative server context.
  If exact server truncation feedback needs a new field/event, scope that as a coordinated API
  change with web/desktop/native/backend tests rather than silently extending SSE.
- Collapse/expand affects only display. It never removes history from persistence or alters the
  context parent. Keep a visible hidden-descendant count and allow returning to the selected path.
- Keep node previews bounded and memoized. Preserve identity for unchanged nodes/edges, coalesce
  render updates if profiling warrants it, and avoid subscribing every card to the whole transcript.
  Test long/deep histories and a representative 500-node branched thread. Measure before adding
  virtualization/layout workers or another state library. Message-history pagination is not
  provided by this plan; thread-list pagination remains unchanged.

## 8. Tests and acceptance criteria

Add graph/layout tests for linear history, sibling answers, sibling follow-ups, multiple roots,
missing parents, cycles, deep trees, stable IDs, deterministic ordering, and collapse projection.
Add runtime tests showing regenerate retains the old subtree, selected-parent requests are
explicit, branch selection makes no request, stopped/late streams cannot overwrite another
node/thread, and identity/scope changes discard cached state.

Extend the existing adapter tests to assert parent forwarding on both targets. Backend repository
and service tests must cover foreign-thread/nonexistent/user/incomplete parents, replay/conflicting
IDs, legacy parent normalization, and ancestor-only context despite many newer siblings.

UI tests should cover creating the first turn, continuing an old answer, regenerating a prompt,
switching views without losing draft/run/selection, opening saved branches after reload, keyboard
actions, focus restoration, copy, failed-history retry, failed generation retry, and stop. Use small
graph-rendering mocks for interaction tests where DOM measurement is unavailable, plus actual
browser inspection for layout/gesture confidence; mocks alone cannot prove canvas behavior.

Implementation checks (not executed by this planning task):

```sh
# frontend/ — start with existing affected suites, then add the new actual test paths
npm test -- tests/components/chat/QueryChatRuntimeProvider.test.tsx tests/components/chat/queryModelAdapter.test.ts tests/components/chat/QueryChat.test.tsx tests/pages/Assistant.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts tests/api/tauriQuarryApi.test.ts
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# backend/ — existing assistant suites are manually included; use name filters first
cargo test --locked persisted_context
cargo test --locked retry_reuses_the_user_message
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

If native Rust changes become necessary, also run Tauri fmt/clippy/tests from `frontend/src-tauri/`.
Inspect with `npm run dev:web` and the desktop target: real streaming, node placement, pan/zoom,
browser console/network, keyboard-only use, reduced motion, narrow viewport, current light theme,
and retained dark tokens. Record and stop only processes started for verification. Do not start
Axum against valuable data just to test this UI; use mocked APIs or deliberately isolated data.

Ready when the image's question → answer → follow-up → sibling branches can be created with real
AI responses, each branch uses only its own ancestor context, every branch survives reload, and
Quarry's shell/theme/thread-list/compact chat continue to work on web and desktop.

## 9. Architecture and rollout

During implementation, update `docs/ARCHITECTURE.md` sections on Assistant state ownership,
feature maturity, UI composition, persistent runs/context selection, and verification. Remove the
current statement that branching controls are unsupported only when they actually ship. Document
session-only layout state and the unchanged stale-stream/authentication limitations. SQLite stays
canonical, Axum owns AI/context, and Tauri stays a transport boundary; no Helix chat graph is needed.

Ship the slices independently with the existing Thread view working throughout. Keep Canvas opt-in
until parent validation/context tests and runtime/UI gates pass. The view switch offers a direct
fallback without deleting branch records. Deferred durable positions/bookmarks would require a
separate schema/API decision, additive migration, and coordinated contract verification.

Planning-task architecture impact: none — this document proposes future behavior and changes no
implemented runtime, API, schema, or theme. The canonical architecture remains descriptive of the
current product.
