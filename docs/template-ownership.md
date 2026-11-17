# Private template ownership

Current storage is fresh PostgreSQL schema 5 with pgvector as the only canonical and search store;
All three domains use the same PostgreSQL pool. See
[PostgreSQL storage](postgresql-storage.md) for setup, tests and recovery; former
SQLite marker upgrades are historical and are not executed by this runtime.

As of 2026-11-05, embedded templates belong to the authenticated importing user. An account starts
with zero templates and can list, read, retrieve previews/assets/metadata, retry processing, update,
delete, or hydrate export assets only for its own records. This applies to the stable facade and
native slide-template routes under both `/api/v1` and `/api`. Missing and foreign IDs return the
same not-found behavior. Identity comes from the verified bearer session; `X-User-Id` remains a
consistency check, never an ownership selector. Client `appId` remains fixed to Quarry and cannot
replace the owner filter.

Request middleware creates immutable repository scopes. Publication records the owner in the same
PostgreSQL transaction as JSON, assets, preview, metadata and any classification jobs. Reads and writes
check both owner and app membership. Export/insertion resolves stored image references through the
same scope. HTTP URLs and response shapes are unchanged. Web and desktop use the same server
checks; the React workspace already remounts when the authenticated session generation changes.

Shared workers process durable jobs for all owners using the trusted system repository, retaining
existing concurrency, leases, shutdown and retry budgets. Request scopes share that worker and
admission limits. PostgreSQL is the ownership authority and the only template store: vector ranking
applies owner/app/profile predicates in SQL, and no public endpoint exposes unfiltered retrieval.
Deleting a missing or inaccessible template changes nothing; canonical deletion cascades to stage
rows, lexical postings and vectors in the same transaction, with no graph cleanup step.

<a id="schema-17-rollout-and-recovery"></a>
## PostgreSQL setup and recovery

Fresh PostgreSQL schema 5 includes nullable `templates.owner_user_id`, its restricted user foreign
key and owner/template index. Every authenticated import assigns the verified actor inside the
publication transaction. Unassigned records stay hidden; no automatic ownership claim is allowed.
This deployment does not import older shared catalogs or run former SQLite upgrades.

Use a fresh `Quarry` database. Back up PostgreSQL and retain the matching configuration/binary
before future changes. Drain writers for recovery. Do not remove ownership checks, modify schema
markers or delete canonical rows to conceal a failure.
See [current storage setup and recovery](postgresql-storage.md).

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
The nine opt-in tests, live application startup, live legacy storage/provider calls and desktop packaging
were not run. Frontend source and transport signatures did not change, so frontend production
builds and native Rust checks were not rerun for this backend-only ownership change.

Schema 2 templates expose bounded content revisions in catalog/previews and document envelopes
`{revision, document}`. DELETE requires the displayed `expectedRevision` query value. Repository
replacement uses the stored revision as a conditional guard. Classification SQL mutations, refresh
and deletion coordinate on the canonical parent; provider and graph work remain outside SQL.
