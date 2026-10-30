# Notebook API implementation verification

This record covers the initial API/transport implementation before the subsequent explicit-Save
editor integration. See [Save UI verification](notebook-save-ui-verification.md) for the current
UI behavior and later checks.

Date: 2026-10-28. Scope: personal notebook/folder/note resources, durable save jobs, schema 13,
Markdown/chunk/embedding/Helix publication, authenticated SSE, and web/desktop transport parity.
The initial working tree was clean. No dependency/lockfile changes, real database resets, ignored
product-data edits, `.env` reads, live provider calls, or live Helix writes were used.

## Results

Commands run from the indicated build roots, with their final outcomes:

| Root | Command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo test --locked notebook_` | Initial focused run passed 26 tests; expanded focused run passed 33, with the interactive fixture ignored. The final broad run also includes the subsequently added forged-schema regression. |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `backend/` | `cargo test --locked --all-targets -- --test-threads=2` | **412 passed, 0 failed, 8 ignored** |
| `frontend/` | `npm test -- tests/api/notebooks.test.ts` | **42 passed** |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test -- --maxWorkers=2` | **368 passed across 74 files** |
| `frontend/` | `npm run build:web` | Passed; existing large-chunk advisory remains |
| `frontend/` | `npm run check:web-bundle` | Passed after web build |
| `frontend/` | `npm run build:desktop-ui` | Passed; existing large-chunk advisory remains |
| `frontend/src-tauri/` | `cargo fmt --all -- --check` | Passed |
| `frontend/src-tauri/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `frontend/src-tauri/` | `cargo test --locked --all-targets` | **19 passed** |
| repository | `git diff --check` and final `git status --short` | Passed/reviewed; only feature-related source/tests/docs changes; no lockfile/generated-file churn |

The first sandboxed broad Rust runs could not bind disposable loopback HTTP fixtures. Those runs
were not treated as passing; reruns with loopback permission exercised the real local fake servers.
The initial unrestricted default-concurrency backend run had two unchanged QuickLook subprocess
fixture failures; their four-test subset passed against an archived HEAD baseline, and the complete
final suite passed with two test threads. The initial default-concurrency frontend run had timing
failures in existing App/AddDealModal navigation tests; all tests passed with two workers. No
unrelated UI/QuickLook implementation or test assertions were changed to conceal these failures.

Eight default-ignored backend tests comprise the interactive notebook fixture (run separately here),
a template validation server, disposable live Helix compatibility/projection tests, and installed
Chromium preview tests. Live OpenAI/Helix and packaged Tauri execution remain unverified. No release
packaging, normal product `cargo run`, reset utility, graph clearing, or Diligence Studio run was
needed or performed.

## Coverage and review

Backend tests cover notebook/child ownership, reference checks, null/omission semantics, labels and
strict pagination/query parsing; full native documents from the installed editor (including merged
table cells), unsafe content and structure/size limits; admission/idempotency, one active save,
revision races, queue capacity and Retry-After; durable canonical snapshots, failure preservation,
explicit retry without revision duplication, pre/post-commit crash recovery, lease claims, corrupt
checkpoint isolation, stale folder/delete fences, and empty projections. Repository tests verify
literal wildcard search, summary reads without blocks, overflow, rollback after a typed domain
failure, folder relocation and notebook cascades. Migration tests verify existing records survive
12 → 13, repeat initialization, forged/missing indexes, rollback of partial DDL/marker changes,
composite FKs, direct nonempty-folder rejection and deferred cascade ordering. Existing full-suite
migration/concurrency/architecture checks also passed.

Graph preparation tests verify deterministic ordered owner/notebook/note/revision/chunk identities,
bounded queries, dimension/finite-vector validation, publication after all staged chunks, and empty
replacement. The save pipeline uses a paused/failing fake to prove admission returns before indexing
and completion waits for both stores. Graph protocol requests compile against the pinned Helix 3
SDK; no live graph compatibility claim is inferred from those fake/pure tests.

All REST methods have web/desktop adapter parity tests, including encoded query values, omitted vs
null payloads, DELETE/204 behavior and 409/413/503 status preservation. SSE tests cover immediate
terminal snapshots, sequence deduplication, wrong-owner/scope rejection, revoked-session closure,
framing/UTF-8 bounds, cancellation and late subscription cleanup. Native tests verify PATCH path/
UUID/body restrictions and bounded incremental event framing. Client diagnostics tests exclude note
payloads, returned content and title-search values. Notebook embeddings use the existing provider
client through a private-diagnostics entrypoint that omits provider error bodies.

Self-review covered tests first, then correctness, authorization, bounded work, ownership/layering,
recoverability and transport parity. Required review findings were fixed before the final gates.
Verdict: **Approve** for the implemented API/transport scope. No newly unused production code was
identified. The unused conceptual notebook scaffold was replaced by registered implementation.

## Computer visual regression

The explicit ignored `notebook_visual_fixture` test was started through
`cargo test --locked notebook_visual_fixture -- --ignored --nocapture` on loopback port 39001.
It uses in-memory SQLite, real authentication and fake embedding/Helix capabilities. The frontend
ran with `VITE_API_BASE_URL=http://127.0.0.1:39001 npm run dev:web -- --host 127.0.0.1` on 1420.
Both ports were checked before startup; only the task-owned listeners were stopped afterward, and
both ports were verified free. The fixture uses synthetic test credentials; no real user records
were read or edited.

Computer confirmed:

- Authentication and direct Notebook navigation, plus the explicit session-only empty state.
- New draft creation and initial title focus; title and native body editing using the keyboard.
- Search finds the edited draft and the document remains usable.
- Desktop 1440×1000 and mobile 390×844 layouts, editor, sidebar/drawer and session-lifetime text.
- Mobile drawer keyboard dismissal restores focus to its Notes trigger.
- Browser warning/error log was empty for this flow.

Desktop and mobile screenshots were saved outside the repository as
`/private/tmp/quarry-notebook-desktop.jpg` and `/private/tmp/quarry-notebook-mobile.jpg`.
The browser viewport override was reset. Dark mode was not forced: the current product explicitly
disables it. The packaged desktop app was not launched. Visual regression confirms the existing
session editor remains intact; server notebook/folder selection and saved/progress UI are explicitly
outside this plan's API/transport scope, so no visual server-save integration is claimed.

## Architecture impact

Updated `ARCHITECTURE.md`, `DOMAIN_MODEL.md`, authentication/recovery references, the current rollout
notes and root agent schema guidance. [Notebook API and recovery](notebook-api.md) records exact
routes, bounds, durable checkpoints/leases/retention, completion semantics, independent deletion
cleanup and additive schema 12 → 13 rollback. This distinguishes durable notebook jobs from existing
in-memory document jobs and durable APIs from the session-only editor.
