# Authentication implementation verification

Date: 2026-10-28. Scope: schema 11, password/session API, resource ownership, HTTP/Tauri
transports, global user provider, login/register/account UI, and the wider West Monroe form.

## Automated gates

All commands below passed against the final implementation from the stated build roots.

| Root | Command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `backend/` | `cargo test --locked --all-targets` | 373 passed, 7 ignored |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| `frontend/` | `npm test` | 278 passed in 69 files |
| `frontend/` | `npm run build:web` | Passed |
| `frontend/` | `npm run check:web-bundle` | Passed after the web build |
| `frontend/` | `npm run build:desktop-ui` | Passed |
| `frontend/src-tauri/` | `cargo fmt --all -- --check` | Passed |
| `frontend/src-tauri/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `frontend/src-tauri/` | `cargo test --locked --all-targets` | 16 passed |
| Repository root | `git diff --check` | Passed |
| Repository root | `git status --short` | Reviewed; original untracked plan preserved and updated |

Focused authentication tests cover registration/login/me/logout, password salting and limits,
token digests and expiry, revocation and stream closure, header consistency/CORS, exact West
Monroe domain enforcement, two-user resource isolation, removed public profile bypasses,
form states, logging exclusions, stale 401s, memory-only expiry/cancellation and fragmented
UTF-8 authenticated document GET-SSE. The native test uses a disposable loopback server to
verify credential validation, scoped headers and structured 401 status without global defaults.

An existing QuickLook stdout/stderr buffer timing assertion failed once under parallel load;
its focused rerun and the subsequent full default backend suite passed. A pre-existing CMYK
iterator lint from the newer Clippy toolchain was fixed with equivalent `as_chunks` iteration.
Vite builds retain warnings for existing large dependency chunks.

The seven ignored backend tests require explicitly isolated Helix validation instances or
full Chromium/OS integration conditions. They were not enabled for this authentication task.
The skill-creator Python validator could not run because the default Python lacks PyYAML;
the changed skill's unchanged frontmatter was successfully parsed and name-checked with Ruby
Psych instead.

## Runtime and development reset

The user selected `helix-quarry-v3-dev` on port 6969. Its image matched the repository pin and
it had no mounts. No backend was listening on port 3001 before the reset. The selected container
was stopped, removed, recreated with an empty store from the same pinned image and restarted.
The explicit offline command was run from `backend/`:

```sh
target/debug/reset_development --discard /Users/rgambhir/Quarry/backend/data/quarry.sqlite3
```

The shared canonical SQLite schema is now 11. There is one maintained development database
location and one launcher-managed Helix instance for normal application use. The test-created
smoke account was removed afterward while the backend was stopped; other newly created records
were retained. No `.env` contents, provider secrets, password hashes, or live session tokens were
printed or committed.

A backend started with the explicit canonical database path and selected Helix URL passed live
registration, login, current-user retrieval, authenticated deal listing, logout and revoked-token
rejection. After rebuilding/restarting, login still succeeded and non-West Monroe registration
was rejected. These smoke requests made no AI/provider calls.

`npm run dev:web -- --host 127.0.0.1` was inspected in the browser. Login reached `/hub` with the
disposable account; a page reload returned to sign-in as designed. Registration's complete wider
layout, fixed `@westmonroe.com` suffix, labels and visible controls were checked. A narrow
viewport stacked the form without horizontal overflow. The currently supported light theme
was inspected; the existing dark-mode feature flag remains unchanged.

`npm run dev:desktop` compiled and launched the native executable. The UI automation tool could
not attach to the unbundled development executable, so native visual/interactive login is not
claimed as verified. The shared desktop UI build, native gateway tests and full native gates
passed. Packaging/release validation was not run.

All task-owned Vite, Tauri and backend processes were stopped; ports 1420 and 3001 were checked
free. The canonical Helix service remains running for normal development.

## Review and architecture impact

Reviewed correctness, credential handling/logging, ownership and compatibility mounts, runtime
boundaries, blocking/concurrency limits, cancellation/stale-session behavior, UI semantics and
verification evidence. No unresolved required finding remains in that review. Removed legacy
profile/demo/diagnostic and arbitrary-path summary handlers are intentional; uploaded-file
summarization remains available behind authentication.

Architecture impact: updated `ARCHITECTURE.md`, the authentication design, migration/recovery
notes, repository guide and backend skill for the new schema, API/session boundary, transports,
per-user checks, shared catalog policy, responsive form and remaining limits. The plan is marked
implemented and remains design history. Deploy the backend and updated web/desktop clients
together; older profile-only clients cannot authenticate against the new contract.
