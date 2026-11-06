# Private template ownership

As of 2026-11-05, embedded templates belong to the authenticated importing user. An account starts
with zero templates and can list, read, retrieve previews/assets/metadata, retry processing, update,
delete, or hydrate export assets only for its own records. This applies to the stable facade and
native slide-template routes under both `/api/v1` and `/api`. Missing and foreign IDs return the
same not-found behavior. Identity comes from the verified bearer session; `X-User-Id` remains a
consistency check, never an ownership selector. Client `appId` remains fixed to Quarry and cannot
replace the owner filter.

Request middleware creates immutable repository scopes. Publication records the owner in the same
SQLite transaction as JSON, assets, preview, metadata and any classification jobs. Reads and writes
check both owner and app membership. Export/insertion resolves stored image references through the
same scope. HTTP URLs and response shapes are unchanged. Web and desktop use the same server
checks; the React workspace already remounts when the authenticated session generation changes.

Shared workers process durable jobs for all owners using the trusted system repository, retaining
existing concurrency, leases, shutdown and retry budgets. Request scopes share that worker and
admission/mutation limits. Helix keys and projection shape are unchanged; SQLite remains the
ownership authority. There is no public endpoint exposing unfiltered Helix template retrieval.
Deleting a missing or inaccessible template never deletes its graph projection. If a graph deletion
fails after canonical deletion, startup reconciliation removes the stale projection.

## Schema 17 rollout and recovery

Drain HTTP and background writers, stop the previous backend, and take a consistent SQLite backup
including committed WAL data and a matching Helix backup before deploying. Startup validates the
complete preceding schema and transactionally adds nullable `templates.owner_user_id`, referencing
`users(id)` with `ON DELETE RESTRICT`, plus `templates_owner_user_id_idx`. Supported schema markers
12 through 16 upgrade through their existing additive steps before ownership is added. Incompatible
or damaged layouts fail without advancing the marker or partially applying ownership changes.

Existing shared built-ins/imports have no trustworthy importer attribution. They remain stored with
a null owner and become invisible to every user's template API; they are never deleted or assigned
to the first person signing in. Reimporting an original PowerPoint creates a new owned copy. There
is no automatic claim, transfer or recovery endpoint for old shared records. A deliberate ownership
assignment would require separately reviewed maintenance using verified importer evidence.

Rollback requires draining and stopping the new backend, then restoring the matching pre-upgrade
SQLite/Helix backups and binary. Older binaries reject schema 17. Do not decrement the marker or
remove the ownership column to run an older binary: it would expose shared data again. Restoring
backups loses intervening writes and requires an explicit operational decision. No live database
or graph is changed as part of implementation verification; migration tests use disposable stores.

## Legacy service

The separately configured Diligence Studio service has an app-wide catalog without authenticated
user ownership. Quarry therefore returns 503 for its template facade operations instead of
forwarding requests into that shared catalog. Use embedded mode for private templates. This does
not change the independent service's files, data, routes or fixed `X-App-Id` adapter contract, and
switching modes never copies or assigns legacy records.

## Verification

Checks run on 2026-11-05:

| Build root | Command | Outcome |
| --- | --- | --- |
| `backend/` | `cargo fmt --all -- --check` | Passed |
| `backend/` | `cargo check --locked --all-targets` | Passed |
| `backend/` | `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `backend/` | `cargo test --locked --all-targets -- --test-threads=4` | 471 passed, 0 failed, 9 ignored |
| `backend/` | `cargo test --locked quicklook_` | 4 passed |
| `frontend/` | `npm run typecheck` | Web and desktop passed |
| `frontend/` | `npm run check:boundaries` | Passed |
| repository root | `git diff --check` | Passed |

The focused frontend command below passed all 109 tests:

```sh
npm test -- tests/api/httpQuarryApi.test.ts tests/api/tauriQuarryApi.test.ts tests/auth/authentication.test.tsx tests/App.test.tsx tests/components/deal-room/DeliverableTemplatesView.test.tsx tests/components/deal-room/TemplateDocumentStore.test.tsx
```

The default-parallel backend run hit two existing QuickLook fixture failures; both passed in
isolation, and the complete suite passed with four test threads. No tests were newly ignored.
The nine opt-in tests, live application startup, live Helix/provider calls and desktop packaging
were not run. Frontend source and transport signatures did not change, so frontend production
builds and native Rust checks were not rerun for this backend-only ownership change.
