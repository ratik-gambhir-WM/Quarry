---
name: code-verification
description: Select and run Quarry's repository-native verification for code, configuration, documentation, and cross-runtime changes. Use before handing off any implementation, refactor, fix, feature, dependency update, or architecture/documentation change; routes checks across the npm frontend, Tauri crate, and Axum crate without treating destructive startup as a test.
---

# Quarry code verification

Produce evidence proportional to the change while preserving user work, local data, secrets, and
external systems.

## Preflight

1. Read the root `AGENTS.md` and relevant sections of `docs/ARCHITECTURE.md`.
2. Run `git status --short` and record the pre-existing dirty state.
3. Read the real scripts/manifests in every affected build root. Do not invent commands.
4. Identify changed contracts and consumers, including generated/ignored outputs that checks may
   create.
5. Start with the narrowest meaningful test; broaden only after it passes or yields useful evidence.

Never clean the worktree, revert unrelated files, broadly format user changes, mix package
managers, or update dependencies merely to make a check run.

## Scope routing

| Changed area | Required verification family |
| --- | --- |
| `frontend/src/**`, Vite, TS config, package manifest | frontend tests/typechecks/boundaries/build as relevant |
| shared API contract or transport | frontend web + desktop checks; backend checks; Tauri checks if relay/native code changed |
| `frontend/src-tauri/**` | Tauri Rust checks plus frontend desktop typecheck/boundary checks |
| `backend/**` | backend Rust checks; runtime smoke only with explicit disposable dependencies |
| schema/Helix/migration | focused invariant tests plus full backend gates; no real data or destructive utility |
| docs/agent skills only | structural/content checks, links/paths, skill validation, `git diff --check` |

There is no root test command and no CI-equivalent wrapper. Verify each touched build root.

## Frontend ladder

From `frontend/`:

```sh
# Focused test; use the actual affected path.
npm test -- src/path/to/file.test.ts

# Target checks.
npm run typecheck:web
npm run typecheck:desktop
npm run check:boundaries

# Broad checks.
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Rules:

- Use `npm run typecheck` when shared code is affected.
- Run `check:web-bundle` only after `build:web`; it inspects `frontend/dist`.
- There is no lint/format script. Do not report one as skipped or invent one.
- Test mode selects the web runtime. Direct Tauri adapter tests and desktop typechecking are needed
  for desktop contract confidence.
- A Vite build is not a substitute for TypeScript checks or runtime behavior.

For meaningful UI changes, run the appropriate development target and inspect:

- the changed route and primary user path;
- loading, empty, error, retry, disabled, and success states;
- browser console and relevant network/IPC activity;
- keyboard operation, focus visibility/restoration, and accessible names;
- light/dark themes, reduced motion, and relevant viewport sizes;
- both web and desktop when platform behavior or shared contracts changed.

## Backend ladder

From `backend/`:

```sh
# Focused behavior; use an actual test-name filter.
cargo test <filter>

# Broad checks.
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Add `cargo build --locked --release` when release compilation, feature flags, or packaging changed.

`autotests = false`: `backend/tests/**` are manual modules, so do not use
`cargo test --test <filename>`. Confirm a new test file is actually included.

Do not use `cargo run` as a build check. It opens and may migrate SQLite, requires live Helix, and
initializes indexes. Runtime smoke testing is authorized only when the task needs it and the
database/configuration is explicitly disposable. Never run `clear_helix` as verification.

## Tauri ladder

From `frontend/src-tauri/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

For any native command, IPC payload, desktop API relay, capability, CSP, or routing change, also
run from `frontend/`:

```sh
npm run typecheck:desktop
npm run check:boundaries
npm test
npm run build:desktop-ui
```

Reserve `npm run build:desktop` for actual packaging/release validation. It is broader and slower
than normal implementation verification.

## API and cross-runtime contract checks

When an API method, route, DTO, multipart field, bytes/PDF response, error, or SSE event changes:

1. Verify the `QuarryApi` TypeScript contract.
2. Verify web adapter mapping and error behavior.
3. Verify desktop TypeScript mapping.
4. Verify Tauri command/service/client behavior if it participates.
5. Verify Axum route, extractor, handler, service, and response.
6. Run both frontend type targets, adapter tests, relevant Tauri tests, and backend route/service
   tests.
7. Confirm `/api/v1` remains the client target and compatibility behavior is intentional.

Do not claim contract compatibility from compilation alone.

## Data and external-system safety

- Use temporary SQLite databases and fake/test clients.
- Do not read, copy, mutate, or delete `backend/data/` as verification.
- Do not start migrations against valuable data.
- Do not call live OpenAI, Helix, WM AI, Microsoft Graph, or LibreOffice unless the task explicitly
  requires an integration test and the environment/side effects are authorized.
- Do not print `.env`, secrets, document contents, user email, or absolute sensitive paths.
- If an external integration is unavailable, verify construction/mapping with unit tests and state
  the remaining uncertainty.

## Documentation and skill verification

For Markdown/agent-skill changes:

- confirm every documented path and command exists in the live tree;
- search for stale references to removed/renamed files;
- validate YAML frontmatter and skill naming with the skill-creator validator;
- inspect rendered Markdown structure when complex tables/diagrams changed;
- run `git diff --check` and inspect the final diff/status.

Documentation-only work does not require compiling unrelated code, but commands claimed as known
working should be supported by recent evidence or explicitly described as not run.

## Mandatory architecture-impact check

Every feature, refactor, fix, contract change, configuration change, and infrastructure change
must finish by re-reading the relevant sections of `docs/ARCHITECTURE.md` and comparing them with
the final diff.

Update `docs/ARCHITECTURE.md` in the same change when the diff alters:

- routes, pages, feature maturity, or visible product behavior;
- runtime/platform selection, module ownership, or dependency direction;
- API methods, DTOs, transport hops, errors, multipart/binary/SSE behavior;
- schemas, graph shape, data ownership, migrations, jobs, caches, or persistence guarantees;
- configuration/environment variables, integrations, ports, build/deploy behavior;
- authentication, authorization, data exposure, trust boundaries, or security controls;
- repository structure, standard verification commands, or known gaps.

If none applies, the final report must include `Architecture impact: none — <specific reason>`.
This is a required verification result, not an optional documentation suggestion.

## Final evidence report

Before handoff:

1. Run `git diff --check`.
2. Inspect the final diff for scope, generated output, secrets, debug code, and accidental lockfile
   churn.
3. Re-run `git status --short` and distinguish new changes from pre-existing user work.
4. Report exact commands and pass/fail outcomes.
5. Report skipped checks and why.
6. Report runtime/manual observations and remaining uncertainty.
7. Report the architecture update made, or the explicit no-impact reason.

Never claim a check passed when it was skipped, unavailable, timed out, or only partially ran.
