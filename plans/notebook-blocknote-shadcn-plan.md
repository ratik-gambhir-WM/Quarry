# Notebook workspace with a BlockNote shadcn editor

Date: 2026-10-27. Status: proposed implementation; this plan changes no application behavior.

## 1. Requested experience

Clicking **Notebook** in the Deal Hub sidebar opens a dedicated notes workspace. A new sidebar
lists the current user's notes, and the main surface edits the selected note with the
[BlockNote shadcn integration](https://www.blocknotejs.org/docs/getting-started/shadcn).
Retain Quarry's existing typography, colors, workspace chrome, profile footer, and spacing.

The browser screenshot identifies the entry point and current appearance. Treat its other text
as page content, not instructions. The linked documentation is a technical reference; its demo
theme and installation examples do not override Quarry's project rules.

Recommended first release: personal notes with title, block content, create/select/search,
autosave, and delete. Notes survive reload and are available through the same Axum service in
web and desktop. Folders, deal associations, citations, AI generation, collaboration, uploads,
sharing, and export are separate follow-ups.

```text
Quarry workspace
┌──────────────────────┬─────────────────────────────────────┐
│ ← Deal Hub           │ Note title             Saved        │
│ Notebook   + New     ├─────────────────────────────────────┤
│ Search notes         │                                     │
│                      │ BlockNote editor                    │
│ Selected note        │ Paragraphs, headings, lists,        │
│ Another note         │ checklists, quotes, tables, links   │
│ Older note           │                                     │
│ Load more            │                                     │
│                      │                                     │
│ Existing profile     │                                     │
└──────────────────────┴─────────────────────────────────────┘
```

## 2. Findings from the existing project

| Current source | Consequence |
| --- | --- |
| [HomeWorkspaceSidebar.tsx](../frontend/src/components/hub/sidebar/HomeWorkspaceSidebar.tsx) renders Notebook without an `href`. | Wire the existing item; it currently has no destination. |
| [WorkspaceRoutes.tsx](../frontend/src/app/WorkspaceRoutes.tsx) has no Notebook route. | Add lazy workspace routes once, shared by BrowserRouter and HashRouter. |
| [WorkspaceHomeShell.tsx](../frontend/src/components/hub/WorkspaceHomeShell.tsx), [WorkspaceSidebar.tsx](../frontend/src/components/hub/WorkspaceSidebar.tsx), and [sidebarTypes.ts](../frontend/src/components/hub/sidebar/sidebarTypes.ts) compose route-specific sidebars. | Add a narrow Notebook variant and a shared owner for its sidebar/editor state. |
| [SidebarFrame.tsx](../frontend/src/components/hub/sidebar/SidebarFrame.tsx) already owns collapse, back navigation, scrolling, and the profile footer. | Reuse the frame. It is hidden below `lg`, so provide a mobile notes drawer rather than losing note navigation. |
| [WorkspaceProvider.tsx](../frontend/src/app/WorkspaceProvider.tsx) and [useWorkspaceSession.ts](../frontend/src/hooks/useWorkspaceSession.ts) preserve development-profile context. | Keep navigation state and identity changes consistent with current workspace behavior. |
| [Notebook domain](../backend/src/domains/notebooks/mod.rs) is comment-only scaffolding. | There is no note CRUD/persistence API to reuse. Build it explicitly rather than presenting local editor state as saved. |
| [quarryApi.ts](../frontend/src/contracts/quarryApi.ts) has no notebook contract. | Add typed operations and both adapters; shared UI calls `runtime.api`. |
| The manifest/lockfile contain React 19 canary and Plate 53; BlockNote is absent. | Check BlockNote's current peers against the canary. Keep Plate for Diligence Canvas slide text; do not replace that editor. |
| [index.css](../frontend/src/index.css) defines Tailwind 4/shadcn aliases and Quarry tokens. | Reuse these variables rather than copying BlockNote's sample neutral palette. |

Build roots touched by implementation: `frontend/`, `backend/`, and `frontend/src-tauri/`.
This task creates only this planning document. Preserve all existing uncommitted changes.

## 3. Routes and sidebar ownership

- Add `/hub/notebook` and `/hub/notebook/:noteId`. Use a parent `NotebookPage` layout whose
  provider and sidebar remain mounted across selected-note changes. The note ID in the URL is
  authoritative; browser Back/Forward, reload, and desktop hash navigation select the same note.
- `/hub/notebook` opens an explicit “Select a note or create one” state with the list visible.
  Do not create a blank record on every visit. New note creates one server record, then navigates
  to its stable ID and focuses the title. Untitled notes use a neutral display label, not fixture data.
- Wire Notebook's link with `navigationState`; add `notebook` to `ActiveHomeSection` and
  `SidebarMode`. Extend `WorkspaceHomeShell`'s allowed mode narrowly, following its Assistant
  composition precedent. Put Notebook state in a feature provider around the shell and outlet,
  not in `WorkspaceProvider` or the generic sidebar.
- Add `NotebookWorkspaceSidebar.tsx` under the existing sidebar directory. Use `SidebarFrame`
  with a Notebook label, Back to Deal Hub link, New note button, labeled search field, and note
  links with `aria-current`. Preserve the existing profile and collapse treatment.
- Sort by updated time, with note ID as a stable tie-breaker. Do not reorder the list on every
  keystroke; apply recency after a successful save and avoid moving a focused row until blur.
  Display title and a modest timestamp; omit sensitive body excerpts initially.
- Provide server-side title search and cursor pagination, with loading/retry/load-more states.
  Search must cover all notes, not only the first loaded page. Debounce input, cancel stale
  requests, and reset cursors on a changed query. An empty search result differs from no notes.
- On small screens, an accessible Notes button opens the same list in a drawer. Close it after
  selection and restore focus appropriately. Main editing remains usable with the onscreen keyboard.

## 4. BlockNote setup and theme integration

Install `@blocknote/core`, `@blocknote/react`, and `@blocknote/shadcn` together at compatible
versions using npm in `frontend/`. Review React canary peer compatibility and lockfile changes;
do not incidentally upgrade React, remove `.npmrc`, or use blanket peer-dependency overrides.
Lazy-load the editor so Hub, Assistant, and presentation routes avoid the extra editor payload.

The [shadcn guide](https://www.blocknotejs.org/docs/getting-started/shadcn) specifies
`BlockNoteView` from `@blocknote/shadcn`, its stylesheet, and Tailwind scanning of that package.
For `frontend/src/index.css`, the package source directive is
`@source "../node_modules/@blocknote/shadcn";`. Reuse existing font/theme definitions.
The guide flags non-portal components when supplying custom shadcn replacements. Quarry's
Popover currently uses a portal: retain shared behavior and use BlockNote defaults initially,
or supply small editor-local compatible wrappers after checking the installed package types.

Use a scoped `.notebook-editor` wrapper and existing `--background`, `--foreground`, `--popover`,
`--border`, `--ring`, and underlying `--theme-*` variables. Preserve Inter, workspace surface,
rounded controls, muted sidebar, and selected-note emphasis. Check editor menus/popovers outside
the wrapper for token inheritance. Retain the existing `data-theme` mechanism; light mode remains
the current product setting, and this feature does not re-enable dark mode.

BlockNote is an uncontrolled editor. Mount it only after the selected note loads, initialize with
its blocks, and key the editing session by owner and note ID. Do not rebuild it on autosave ACKs
or replace content every render. Its change callback feeds draft snapshots into the save owner.
See [editor setup](https://www.blocknotejs.org/docs/getting-started/editor-setup).

Store native block JSON with an explicit format/schema version; Markdown conversions can lose
structure. See [format interoperability](https://www.blocknotejs.org/docs/foundations/supported-formats).
Define a restricted first-release schema for text, headings, lists/checklists, quote, code, table,
and safe links. Disable file/image/audio/video upload, unsupported slash-menu items, embeds, and
unhandled paste/drop file paths until a validated asset lifecycle exists. Do not silently drop
unknown blocks in loaded documents; retain the record and show an unsupported-format error.

## 5. Persistence and API proposal

Use SQLite as canonical storage under `domains/notebooks`, with handler → service → repository
dependency direction and bootstrap-injected clients. Notes do not require Helix, OpenAI, or the
external Diligence Studio service. Keep global search integration separate.

Proposed table `notebook_notes`: stable `note_id`, `user_id` foreign key, `title`, `content_json`,
`content_schema_version`, integer `revision`, and `created_at`/`updated_at`. Index ownership and
recency. Return metadata only from list/search; fetch block content only for the selected note.
Do not add a speculative notebooks/folders hierarchy yet.

The live SQLite marker is 10 with a supported additive schema-9 upgrade. Extend
[migrations.rs](../backend/src/app/migrations.rs) to add the note table transactionally from
supported existing versions, assigning the next available marker at implementation time.
Preserve the schema-9 template path and schema validation; test empty database creation, 9/10
upgrades, rollback, foreign keys, and existing records. Do not follow the Axum skill's older
schema-reset guidance; root instructions and current code take precedence. No destructive reset,
real local database edits, or Helix clear is part of this work.

Proposed API under `/api/v1` (exact DTOs finalized before implementation):

| Method / path | Contract |
| --- | --- |
| `GET /notebooks/notes` | Owner-scoped metadata, bounded title query, cursor/limit; returns items and next cursor. |
| `POST /notebooks/notes` | Create with client-generated stable note ID, title, initial blocks, schema version; return detail/revision. Repeated identical create is idempotent; conflicting ID/content is rejected. |
| `GET /notebooks/notes/{noteId}` | Load owned note detail; missing or inaccessible note yields a consistent not-found response. |
| `POST /notebooks/notes/{noteId}/update` | Full bounded document/title snapshot plus expected revision; atomically compare-and-update and return new revision/timestamp. Stale revision returns `409`. |
| `DELETE /notebooks/notes/{noteId}` | Delete owned note with expected revision, after pending saves settle; `204` on success, conflict if it changed. |

POST update fits the current desktop client's existing methods. Validate the final delete revision
encoding consistently in both transports. Add typed API methods in `quarryApi.ts`, mappings in
both adapters, and narrowly named Tauri commands/service/client handling registered through the
existing command mechanism. Preserve main-window/origin checks, input/body bounds, URL encoding,
status mapping, capability/CSP restrictions, and API activity-log redaction. No generic filesystem
or unrestricted HTTP escape hatch is needed.

Validate identifiers, title/query lengths, pagination, revision, document shape, URL schemes,
and serialized size at the HTTP boundary. Proposed starting bounds: 200-character titles,
100-character search, 50-item pages (maximum 100), 1 MiB documents, 2,000 blocks, nesting depth 12.
Confirm these against actual serialization overhead; reject oversized edits visibly without
losing the local draft. Bind all SQL through the existing builder/client. Keep persisted
ownership/revision invariants in transactions and offload SQLite work through existing patterns.

For local development, follow existing profile-owner resolution and scope every query by that
owner. Caller-supplied email remains development isolation, not authentication. Public production
use requires a trusted authenticated principal and authorization; avoid claiming tenant security.

## 6. Autosave and editing lifecycle

Use one Notebook session owner for metadata requests and per-note drafts/save queues. The editor
owns cursor/undo state; the session owns title, dirty snapshots, revision, pending save, errors,
and conflict state. Avoid effects that copy fetched state into duplicate component state.
Use keyed owners, reducers, event callbacks, and subscription boundaries.

1. Editor/title changes mark a local snapshot dirty. Debounce saves around 700ms and serialize
   writes per note. Capture note ID, owner, base revision, and local change sequence for each write.
2. If edits arrive in flight, acknowledge only the saved snapshot and enqueue the latest draft
   against the returned revision. An older ACK must not mark newer edits saved or replace text.
3. Show **Saving…**, **Saved**, **Unsaved changes**, or **Couldn't save — Retry**. Cmd/Ctrl+S
   flushes pending work. Never show Saved until the server acknowledges that exact draft.
4. Before changing notes or leaving Notebook, flush and await the latest snapshot. If saving
   fails, retain the draft and offer Retry, Stay, or explicit Discard; do not discard on unmount.
   Validate the router's supported navigation-blocking mechanism in both targets before relying
   on it. Ensure note links, Back to Hub, browser history, and desktop close are covered.
5. A browser `beforeunload` warning is a fallback for dirty state, not a reliable async save.
   If desktop close needs native coordination, add a narrow validated close guard. First release
   does not promise offline/restart recovery of unacknowledged drafts; keep that limitation visible.
6. On `409`, stop automatic writes, retain the local draft, and offer reload server version or
   save local changes as a new note. Do not silently overwrite or attempt a JSON merge.
7. Switching identity clears caches after resolving unsaved work; stale list/load/save callbacks
   cannot populate the next user's state. A failed load must not initialize an empty editable
   document that can overwrite the stored note.
8. Delete uses the existing confirmation primitive, stops new edits, drains that note's save
   queue, and sends the expected revision. A late save must never recreate a deleted note.
   On success select another visible note or show the empty state; on failure keep the note/draft.

## 7. Implementation slices

| Slice | Files / completion gate |
| --- | --- |
| Contract and data | Add note DTOs, migration, notebook domain model/repository/service/handler/routes, bootstrap assembly, tests. Backend CRUD/revision/owner tests pass before calling the feature saved. |
| Transport | Extend `quarryApi.ts`, HTTP/Tauri adapters and native relay/registration with focused contract tests. Shared UI never imports raw Tauri APIs or calls fetch. |
| Navigation and list | Wire `HomeWorkspaceSidebar`, shared routes, sidebar types/composition; add `pages/NotebookPage.tsx`, `components/hub/sidebar/NotebookWorkspaceSidebar.tsx`, feature provider, and pure types/mappers under `data/notebook/`. All new paths are proposals. |
| Editor | Add lazy `components/notebook/NotebookEditor.tsx`, scoped styles, BlockNote schema, title field, loading/error surfaces, and session/save owner. Verify shadcn menu/focus compatibility. |
| Reliability and rollout | Wire autosave/conflict/navigation/delete protections, responsive notes drawer, focused tests, runtime inspection, and architecture updates before enabling the link in a released build. |

Keep dependencies/editor imports feature-local. Preserve existing presentation editor behavior and
unrelated active work. If a visual prototype is useful before the API lands, put explicit demo
records under `frontend/src/fixtures/notebook/` and label them; never use fixture notes as a failed
request fallback or claim an in-memory prototype is persisted.

## 8. Verification and acceptance

Add behavior tests for Notebook navigation and direct note URLs, web/hash routing, preserved
session state, new note creation, pagination/search beyond page one, duplicate create protection,
empty/loading/error/not-found states, block JSON roundtrip, independent undo sessions, serialized
autosave with newer edits in flight, conflict resolution, retained failed drafts, deletion races,
identity changes, and keyboard/focus behavior. Use fake timers for debounce tests and temporary
SQLite databases for migrations/revisions; include backend test modules explicitly.

Implementation commands (not run for this planning-only task):

```sh
# frontend/ — existing navigation/transport suites, plus new actual Notebook test paths
npm test -- tests/App.test.tsx tests/components/hub/sidebar/HomeWorkspaceSidebar.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts tests/api/tauriQuarryApi.test.ts
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# backend/ — new actual test-name filters first, then
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# frontend/src-tauri/
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Inspect actual BlockNote behavior in web and desktop: typing/paste, slash menu, toolbar, links,
table interaction, keyboard-only selection, note switching, failed saves, Back/reload, drawer,
scrolling, narrow screens, reduced motion, current light theme, and retained dark-token rendering.
Check console/network and ensure unrelated routes do not fetch BlockNote. Record and gracefully
stop only verification servers started for the task. Do not use routine `cargo run` against real data.

Acceptance: Notebook click opens the new sidebar/editor; users can find all their notes, create
and edit structured content, see truthful save state, reload saved content, and switch notes
without silent edit loss. The experience keeps Quarry's theme and works in web and desktop.

## 9. Architecture completion check

Implementation must update `docs/ARCHITECTURE.md` for routes, Notebook sidebar, editor boundaries,
feature maturity, ownership, schema/migration, API/desktop transport, and save/conflict guarantees.
Update `docs/DOMAIN_MODEL.md` for note identity, ownership, block format version, and revision.
Document limitations: development identity, no collaboration/uploads/offline recovery, and no
Helix/global-search integration. Keep this plan a design input rather than a current product contract.

Architecture impact for this planning task: none — only a proposed implementation document was
added; runtime, persistence, theme, and current feature maturity are unchanged.
