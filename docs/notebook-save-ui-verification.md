# Notebook Save UI verification

Date: 2026-10-28. This follow-up integrates the previously implemented notebook API into the editor.
Earlier backend, transport, migration and documentation changes were already present and were
preserved. This change adds frontend behavior and documentation; it does not alter backend/native
contracts, dependencies, lockfiles, environment files or product databases.

## Behavior

- A filled Save button replaces Session drafts immediately before Export. It uses the same
  `bg-action`/`text-on-action` button variant as New note, with compact spacing on mobile.
- First save explicitly chooses an existing notebook or creates a named one. Subsequent new notes
  use that session's chosen destination. Reads never create a default notebook.
- New notes POST; existing notes PATCH the same server note with expectedRevision. Native blocks
  and contentSchemaVersion travel together. Folder assignment is preserved on existing notes.
- Save admits an immutable snapshot and tracks its durable job through SSE and status polling.
  Only completion displays Saved. Later edits remain Unsaved changes, without resetting the editor
  or undo session when the URL changes from draft ID to server ID.
- Lost admission responses reuse the same operation ID/input. Indexing failure retries the same
  job, preserving canonical content/revision. Conflicts retain local edits until confirmed reload.
  A failed validation permits a corrected new submission rather than retrying invalid content.
- Saved summaries load in bounded pages after sign-in; full native documents load on selection.
  A fresh canonical GET is labelled Stored on server, rather than implying indexing completion.
- Deleting a saved note calls the server and preserves the local buffer on failure. Unresolved
  admissions/saves cannot be discarded while accepted work may still be running. Authentication
  changes dispose monitors and clear local edit buffers; saved content remains in SQLite.

## Automated checks

From `frontend/`:

| Command | Result |
| --- | --- |
| `npm test -- tests/components/notebook/NotebookPage.test.tsx tests/components/notebook/notebookPersistence.test.ts tests/components/notebook/notebookSession.test.ts` | Initial focused run exposed a selection/focus regression and test styling assumption; both corrected |
| `npm test -- tests/components/notebook tests/App.test.tsx --maxWorkers=2` | **39 passed across 6 files** |
| `npm test -- --maxWorkers=2` | **382 passed across 75 files** |
| `npm run typecheck` | Passed for web and desktop |
| `npm run check:boundaries` | Passed |
| `npm run build:web` | Passed |
| `npm run check:web-bundle` | Passed after web build |
| `npm run build:desktop-ui` | Passed |

Builds retain the existing large-chunk advisory. No generated output is included in the diff.
The first broad run contained a stale App test expecting Session drafts; that assertion now checks
Save, matching the explicitly requested behavior. No unresolved failure remains.

New controller/UI tests cover create → completion → PATCH, revision reuse, saved-note hydration,
unchanged editor identity, newer edits after an older completion, duplicate admission prevention,
immutable response-loss reconciliation, partial indexing failure/retry, missed-SSE status recovery,
conflict/reload, definitive validation correction, unsafe discard prevention, cancellation of the
notebook chooser, deletion failure and disposal of late callbacks. Existing hash routing, history,
search, session isolation, mobile drawer, keyboard focus and export regressions also pass.

Review identified and fixed a missing-route-ID match against an undefined remote ID, which had
mounted a new editor before its focus request arrived. It also kept canonical reads distinct from
completed indexing, blocked deletion of uncertain accepted work, and cleared private persistence
snapshots when disposing an authentication session. Final self-review across
correctness, accessibility, security, architecture and bounded loading found no required issue.
No unused production code or unrelated refactor was introduced.

## Computer visual/integration check

The existing opt-in fixture ran from `backend/`:

`cargo test --locked notebook_visual_fixture -- --ignored --nocapture`

The web UI ran from `frontend/`:

`VITE_API_BASE_URL=http://127.0.0.1:39001 npm run dev:web -- --host 127.0.0.1`

The API fixture uses in-memory SQLite, real authentication and fake embedding/Helix capabilities.
Computer verified first-save notebook creation, native body/title save, completed Saved feedback,
subsequent save at the same server-ID URL, reload/sign-in restoration of the updated content, and
another PATCH after reload. The new-note title received focus. The Save button matches New note's
filled color and precedes Export on desktop and at 390×844. Browser warning/error logs were empty.
Dark mode remains disabled by the product; packaged Tauri execution and live OpenAI/Helix were
not exercised. Shared desktop typechecking and production UI build passed.

Screenshots are outside the repository:

- `/private/tmp/quarry-save-button-desktop.jpg`
- `/private/tmp/quarry-save-button-mobile.jpg`

Both ports were free before startup. The task-owned listeners (1420 and 39001) were stopped and
both ports were verified free afterward; viewport overrides were reset. No real user note was
read, overwritten or deleted during verification.

## Architecture impact

Updated the canonical architecture, domain model and notebook API documentation for explicit Save,
session-scoped edit buffers/job tracking, saved-note loading, canonical URL identity, POST/PATCH,
retry/conflict/deletion semantics and the remaining folder/autosave/offline limitations. The earlier
API-only verification record now links to this follow-up. `git diff --check` and final status/diff
review passed, with earlier uncommitted work retained.
