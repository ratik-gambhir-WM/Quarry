# Notebook API implementation verification

Current storage is PostgreSQL schema 6 with pgvector for all product persistence and search.
See [current setup and recovery](postgresql-storage.md). Dated SQLite, graph-storage, reset and
verification records below describe earlier implementations, not current prerequisites or commands.

This record covers the initial API/transport implementation before the subsequent explicit-Save
editor integration. See [Save UI verification](notebook-save-ui-verification.md) for the current
UI behavior and later checks.

Date: 2026-10-28. Scope: personal notebook/folder/note resources, durable save jobs, schema 13,
Markdown/chunk/embedding/legacy storage publication, authenticated SSE, and web/desktop transport parity.
The initial working tree was clean. No dependency/lockfile changes, real database resets, ignored
product-data edits, `.env` reads, live provider calls, or live legacy storage writes were used.

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
a template validation server, disposable live legacy storage compatibility/projection tests, and installed
Chromium preview tests. Live OpenAI/legacy storage and packaged Tauri execution remain unverified. No release
packaging, normal product `cargo run`, reset utility, or graph clearing was needed or performed.

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
and completion waits for both stores. Graph protocol requests compile against the pinned legacy storage 3
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
It uses in-memory SQLite, real authentication and fake embedding/legacy storage capabilities. The frontend
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

## Schema 4 atomic SQL cutover — 2026-11-16

The notebook worker now prepares outside SQL and publishes native content, the complete active
pgvector/GIN set, terminal job revision/set/sequence and slot release in one retained transaction.
The private keyword/semantic/hybrid search API is wired through both adapters and the validated
Tauri POST relay. The sidebar remains a metadata filter. Rollout uses a fresh schema or explicit
guarded MVP reset after draining old workers; no development database reset, backfill, live provider
call or retained legacy storage mutation was performed during implementation.

Verification from the owning build roots:

| Command | Result |
| --- | --- |
| Backend `cargo fmt --all`, then `cargo fmt --all -- --check` | Passed |
| Backend `cargo check --locked --all-targets` | Passed |
| Backend `cargo clippy --locked --all-targets -- -D warnings -D clippy::redundant_clone` | Passed |
| Backend `QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets` | 539 passed, 0 failed, 8 opt-in tests ignored |
| Frontend `npm run typecheck` | Both targets passed |
| Frontend `npm run check:boundaries` | Passed |
| Frontend `npm test` | 689 passed across 102 files; focused notebook adapter/persistence tests also passed |
| Frontend `npm run build:web` and `npm run build:desktop-ui` | Passed |
| Frontend `npm run check:web-bundle` | Passed after the web build |
| Both matching `check:bundle-size:*` and `check:notebook-bundle:*` | Passed before dist changed targets |
| Tauri `cargo fmt --all`, then `cargo fmt --all -- --check` | Passed |
| Tauri `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| Tauri `cargo test --locked --all-targets` | 27 passed |

PostgreSQL checks used only random schemas in the explicitly selected `_test` database with the
already provisioned shared pgvector extension. The sandbox initially denied database/loopback
access; those suites passed with the required local-network permission. No application server,
container or volume was started for verification. Test fixtures own/clean their schemas/listeners.

Coverage includes later-batch and deferred-constraint rollback, final attempt-fence loss after
chunk insertion, new-note invisibility, zero-chunk completion, old/new repeatable-read snapshots,
exact artifact reuse after folder changes, stale revision/parent deletion, expired-token recovery,
completed replay, and 32-per-user/256-global admission across independent pools. Existing full-suite
gates cover writer-lease lifetime/loss, independent reads and bounded pool failures, resource CAS,
document complete-set recovery and captured template lexical parity. Global production write guards
remain; test-only independent pools do not establish multi-instance deployment support.

Search checks cover malformed/private requests before embedding, current-set/revision joins,
Unicode/phrase/prefix/technical tokens, exact cosine reference score, actual EXPLAIN execution and
GIN index validity, explicit provider failure, and session revocation during a paused query.
Exact cosine remains the baseline. No ANN, large-corpus relevance/recall, production latency or
load claim is made; those need representative corpus evaluation before an ANN rollout.

Review covered state/architecture, transaction/authorization placement, bounded requests, SQL
ownership, cross-runtime transports and manual ownership/clone analysis. Repeated deep claim copies
were removed and checked vectors now move into SQL parameters; one native snapshot copy remains
necessary for the owned blocking export task. Small profile/title copies belong to independent
bound rows. No required review finding remains. The removed legacy storage implementation and compatibility
tests were retired with their callers; no alternate notebook backend or unused cleanup worker remains.
Visual styles, controls, motion and focus were unchanged; no manual browser/theme walkthrough was
run. Existing editor tests cover save/retry/newer-edit/reload behavior. Architecture and recovery
documentation and repository guidance were updated for schema 4 and the new commit boundary.

One full frontend run during concurrent backend compilation timed out in the unchanged slide-editor
lazy-load fixture. That fixture passed alone, then the entire 689-test suite passed without concurrent
compilation. Its implementation and timeout were not changed.

### Follow-up ZIP security fix and external dependency

After the notebook migration, the user requested Dependabot #5/#6 remediation and asked that the
DOCX library source remain outside Quarry. The patched published 0.1.11 source first passed the
full backend gate: **541 passed, 0 failed, 8 opt-in tests ignored**, including the two new adversarial
DOCX regressions. It was then published to the separate private
[docx-rust-quarry repository](https://github.com/ratik-gambhir-WM/docx-rust-quarry), and Quarry now
pins `c63f575c792d1f1bad173dd98d8d23d80d5b4a5d` as a normal Cargo Git dependency. No vendored
library remains in Quarry. Cargo resolved only ZIP 9.6.0; ZIP 4.6.1 and its exclusive derive helper
were removed without unrelated upgrades. The package retains the MIT license and release checksum
provenance; only ZIP compatibility/header checks and one explicit lifetime differ from upstream.

After switching to the Git source:

- `cargo check --all-targets` fetched and compiled the pinned dependency successfully.
- `cargo test --locked --lib docx -- --nocapture`: **11 passed**, including parsing, ingestion,
  viewing, valid roundtrips, conflicting local/central names and the 112-byte symlink archive.
- `cargo fmt --all -- --check` and the backend Clippy gate passed.
- `cargo tree --locked -i zip`: every consumer, including DOCX, resolves to ZIP 9.6.0.
- `cargo audit --json`: **0 vulnerabilities**, with existing unmaintained/yanked warnings.
- Workflow and composite-action YAML parsed successfully. The exact CI SSH setup script loaded
  the dependency-only read-only deploy key and authenticated a strict-host-checked `git ls-remote`
  of the pinned commit; its temporary agent was stopped. The new key was stored only in the
  repository's encrypted Actions secret. No private key is committed.

The CI workflow changes are local until Quarry's changes are pushed. No Actions run was dispatched,
and no Dependabot alert was dismissed; GitHub will re-evaluate after default-branch publication.
Developers need SSH read access to the private dependency; secretless forks need an authorized CI
run. The temporary local vendor tree was removed after the Git fetch succeeded. Architecture docs
record the external dependency, auth mechanism and upstream-removal criteria.

### Private DOCX source-package release — 2026-11-16

At the user's request, the separate private DOCX repository published prerelease
[`v0.1.11-quarry.1`](https://github.com/ratik-gambhir-WM/docx-rust-quarry/releases/tag/v0.1.11-quarry.1)
at commit `b9b6a04dd6dc75a003cc13d4fb1cbd14d48ac128`. The `.crate` archive and `SHA256SUMS` are
GitHub Release assets, not a Cargo registry publication. Repository privacy and Quarry's existing
Git pin were preserved. Release changes are package version/compiler metadata and distribution
documentation; `git diff c63f575 -- src` confirmed the Rust source is unchanged.

`cargo package` successfully extracted and rebuilt the archive; `cargo test --lib` passed all
72 library tests. The final archive was rebuilt from the clean release commit, inspected for
expected paths/license/version/VCS provenance, uploaded, independently downloaded, and verified
with `shasum -a 256 -c SHA256SUMS`. GitHub's asset digest also matched. Package SHA-256:
`9e62ecd0ac566d448e2a8c66d0718b99a06ef3ae869c8efb35d8ad7458920506`.

The upstream-source `cargo fmt --check` reports existing formatting differences in two unchanged
Rust files; no formatting churn was added. Quarry's runtime gates were not repeated for this
external metadata/release operation; its dependency/source/lockfile remain unchanged. Architecture
documentation records the release/distribution boundary, and `git diff --check` passed.
