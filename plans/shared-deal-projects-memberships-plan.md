# Shared deal projects, teams, and memberships implementation plan

Date: 2026-10-30. Status: proposed; no runtime changes implemented by this document.

## 1. Outcome and scope

Let multiple authenticated users work in one existing deal project. Deal members can see the
same stored documents; authorized contributors can upload documents; members can see who
uploaded them and a durable activity history. Owners manage access and organize members into
deal-specific teams. Support the same workflows in the web and Tauri distributions.

This implements the shared-access foundation and shared-document workflow discussed in the
assessment. It does not implement persistent PDF annotations/comments, findings, organization
tenancy, email invitations, shared notebooks/chats, live co-editing, or permanent deletion.
Those features can subsequently use the membership policy and stable deal/file/version IDs.

[Architecture](../docs/ARCHITECTURE.md) and [authentication](../docs/authentication.md) describe
current behavior. This plan is a design input, not an implemented contract. Recheck the live
tree before implementation, including schema markers and any intervening deletion/search work.
Preserve the pre-existing deletion of `plans/notebook-management-backend-api-plan.md`.

## 2. Current implementation and affected boundaries

| Current source | Consequence for this work |
| --- | --- |
| [Identity resource access](../backend/src/domains/identity/access.rs) and [HTTP authentication](../backend/src/app/http/auth.rs) enforce `deals.user_id` on deal paths. | Authentication remains intact; replace ownership-only admission with operation-specific membership policy. Merely widening the middleware check would give viewers write access. |
| [Deal repository](../backend/src/domains/deals/repository.rs) lists owner-scoped aggregates and validates same-owner company assignments. | List accessible deals through membership without duplicates. Preserve company-directory isolation explicitly. |
| [Document store](../backend/src/domains/documents/store/sqlite.rs) requires the file workspace to equal the deal creator's numeric user ID. | Replace creator-based scope with canonical `deal_id`, retire `workspace_id`, and check membership again in write transactions. This is a data migration, not a field rename: existing workspace values contain user IDs. |
| [Ingestion](../backend/src/domains/documents/ingestion/service.rs) uses that identity for parsing, hashing, deduplication, and graph lookup. | Use deal scope consistently for all collaborators, with separate authenticated actor attribution and an explicit identity migration. |
| [Document jobs](../backend/src/domains/documents/ingestion/job_service.rs) retain uploader identity in memory. | Add deal identity and membership-aware subscriptions; preserve their documented process-local lifetime. |
| [Search handlers](../backend/src/domains/documents/search/handler.rs) force actor workspace; [search service](../backend/src/domains/documents/search/service.rs) checks canonical live ownership after retrieval. | Add authorized deal-scoped search and update existing global search to include shared deals safely. |
| [Graph models](../backend/src/domains/documents/graph/model.rs) lack `deal_id`. | Replace `workspace_id` with `deal_id` across graph models, projections, queries, and search indexes. Backfill from SQLite's actual file-to-deal relationship, preserving existing identities and embeddings. |
| [Shared API](../frontend/src/contracts/quarryApi.ts), [web adapter](../frontend/src/api/httpQuarryApi.ts), [desktop adapter](../frontend/src/api/tauriQuarryApi.ts). | Carry capabilities, membership operations, attribution, and pagination through both adapters. |
| [Native relay](../frontend/src-tauri/src/quarry_api/service.rs) and [notebook PATCH](../frontend/src-tauri/src/quarry_api/notebooks.rs). | PATCH currently accepts notebook paths only. Extend narrowly if the proposed management PATCH routes are adopted. |
| [Schema](../backend/src/app/migrations.rs) is marker 13. | Add a validated, data-preserving upgrade; no development reset. |

Build roots: `backend/`, `frontend/`, and `frontend/src-tauri/`. No changes to the independent
Diligence Studio server are required. Shared React still calls Axum through `@quarry/runtime`;
Tauri remains a validated transport/native boundary. SQLite remains canonical; Helix remains
a recoverable graph/search projection. No organization/workspace hierarchy is introduced.

Implementation uses the repository's Axum/React/verification/review skills plus the requested
Rust and React/Vite engineering guidance. Keep Rust edition 2021 and the manifest MSRV; do not
copy newer-edition syntax or optimization/dependency suggestions merely because a skill lists them.

## 3. Product policy and stable contracts

Use the existing deal as the project; do not create a duplicate project entity. A team belongs
to exactly one deal and organizes work. Users may join multiple teams in that deal. Team
membership never grants access independently of active deal membership. There are no team-only
documents or inherited organization roles in this release.

| Capability | Owner | Manager | Contributor | Viewer |
| --- | --- | --- | --- | --- |
| Read shared deal summary, stored documents, search, upload progress, activity, member/team roster | Yes | Yes | Yes | Yes |
| Upload documents | Yes | Yes | Yes | No |
| Edit shared deal fields/links and run deal metadata extraction | Yes | Yes | No | No |
| Create/rename/delete teams and assign existing deal members | Yes | Yes | No | No |
| Add/remove deal members or change access roles | Yes | No | No | No |
| Transfer ownership or archive deal | Yes | No | No | No |
| Select/configure local source roots | Only when also the creator, with native authorization | No | No | No |

Exactly one active owner is required. Creation inserts the deal and owner membership atomically.
Ownership transfer atomically promotes an existing active member and demotes the prior owner to
manager. Owner removal, self-demotion, and self-leave require transfer first. Other members may
leave; leaving removes team assignments and access, while retaining authored history. Archive
retains membership and reads but blocks content/access/team mutations; owner-initiated member
removal and non-owner self-leave remain available for revocation, and ownership transfer remains
available for administration.
Do not infer permissions from job title, email suffix, UI controls, or caller-provided actor IDs.

Keep bearer-session handling, `/api/v1`, camelCase DTOs, multipart limits/names, PDF checks,
native origin/path validation, and document-job event names stable. Unknown/foreign resources
return 404; authenticated members lacking an operation capability receive 403; revision conflicts
receive 409. A 403 must not log the user out.

## 4. Canonical data design

Names below are proposed. Use bound queries through the current SQL builder and SQLite client;
extend their supported operations if necessary rather than scattering raw SQL in repositories.
Migration DDL stays in the migration owner.

| Table/change | Fields and invariants |
| --- | --- |
| `deal_memberships` | Composite key `(deal_id, user_id)`; role enum owner/manager/contributor/viewer; added-by actor, joined/updated timestamps, optimistic revision. Foreign keys to deals/users. Physical membership removal is allowed; durable activity retains attribution separately. Partial unique owner index limits owners to one; transactional service/repository checks ensure at least one. Index `(user_id, deal_id)` supports accessible deal lists. |
| `deal_teams` | Stable team ID, deal ID, name, normalized name key, creator, timestamps, revision. Unique `(deal_id, normalized_name)` and `(deal_id, team_id)`. Bound name length, team count, and normalization explicitly in DTO tests. |
| `deal_team_memberships` | Key `(deal_id, team_id, user_id)`; composite foreign keys to same-deal team and deal membership. Removal of a deal member cascades its team assignments; removal of a team never removes deal membership. |
| `quarry_file_versions.uploaded_by_user_id` | Actor FK for each newly accepted immutable version, distinct from storage scope. Existing versions remain null with attribution origin `legacy_unknown`; do not invent an uploader by assuming the creator uploaded everything. |
| Document identity manifests | Version-level derivation version and exact per-chunk ID/order/hash/generation mapping, linked to canonical file versions. Unique chunk IDs and per-version generation/order entries. Preserve legacy IDs through scope migration and exact-byte retry; persist new manifests before graph writes. |
| `quarry_files` scope cleanup | Retain its actual `deal_id` FK, remove redundant `workspace_id`, and replace workspace-based lookup indexes with deal-scoped indexes. Preserve rows, file/version/blob foreign keys, and current-version invariants during any table rebuild. |
| `deal_activity_events` | Stable event ID, deal ID, authenticated actor, typed event kind, target IDs, server timestamp, bounded allowlisted detail fields. Append-only through application APIs. Index `(deal_id, created_at, event_id)` for stable cursor pagination. No paths, credentials, document text, provider error bodies, or email snapshots. |
| Deal access revision | Monotonic deal-level revision incremented for membership/role changes, allowing cache invalidation and bounded stream revalidation. |

Keep `deals.user_id` immutable as creator provenance only, including after ownership transfer;
it must no longer determine document storage or search scope. Document its meaning; add explicit
`createdByUserId`, current role, and
capabilities to client DTOs rather than interpreting that legacy field as the current owner.
The current owner comes from membership. No implicit creator access survives membership removal.

Activity covers project creation, membership/role/ownership changes, team changes, archive,
shared metadata changes, and accepted uploads. Write the event in the same SQLite transaction
as its canonical mutation, using narrow activity persistence helpers, not nested transactions.
Exact-byte retry/deduplication does not reassign original attribution or create a second
`document_uploaded` event. Indexing completion is not a second upload. Migration does not
fabricate historical events; record one system-origin membership-backfill event per deal.
History survives member removal; user hard-deletion/retention remains a separately defined policy.

## 5. Authorization and data-boundary changes

Implement a typed `DealAccessService` in the existing `memberships` domain with narrow repository
dependencies. `identity` continues authenticating sessions; resource access delegates deal policy
to membership. Bootstrap wires services; handlers never acquire repositories through AppState.
Use typed IDs/roles/capabilities, `ServiceError`/`RepositoryError`, and sanitized HTTP errors.
Transport validation remains in handlers; trusted use-case policy stays in services/repositories.
Run blocking SQLite work through the existing client offloading; never hold a lock across await.

Audit every current route and service entry, including `/api` compatibility aliases, nested
file/version/job identifiers, source preview, metadata extraction, archive, and search. Require
the relevant capability, verify child-to-deal relationships, and exclude owner-only convenience
methods from production call paths. Recheck membership/archive/revision inside mutations so a
revocation racing an upload or role update cannot be bypassed by earlier HTTP admission.

Do not broaden unrelated ownership:

- Personal notebooks and assistant threads remain private. A deal link never grants access to
  their content; shared deal ownership does not authorize another user's notebook/chat endpoints.
- Read deal-associated company information already included in the aggregate, but do not expose
  the creator's directory. Managers may retain existing party references or add companies they
  themselves own; validate this within the deal update transaction. Editing company entities
  remains owner-scoped. Avoid opening another person's company editor from the shared deal UI.
- Local desktop folders remain device-local. Source configuration requires both current owner
  and original creator identity, preserving native authorization; after ownership transfer no
  member gains a new local-source capability. Shared members use canonical uploaded documents; redact local paths and
  `source_uri` from shared DTOs/activity and never fall back to a server root for unauthorized callers.
- Existing shared templates/research endpoints retain their separate policies; membership does
  not silently make them deal-owned or fix their wider production-readiness limitations.

First release adds registered users by exact normalized username, initiated by the owner.
Resolve server-side through a bounded management operation; return only the selected user's
minimal display identity, not a global directory or email-based account list. Existing registration
does not verify mailboxes. Email invitations, acceptance tokens, external guests, and SSO are deferred.

## 6. Shared ingestion, jobs, and search

Introduce an internal upload context containing authenticated actor ID and authorized deal ID.
`deal_id` is the document storage/search scope for both existing and new deals. Retire the document
`workspace_id` field rather than retaining a permanent creator-based namespace or simply renaming
its existing user-ID values. SQLite already has the authoritative `quarry_files.deal_id`; use it
to populate Helix file/version/chunk properties and deal-partitioned indexes.

Separate actor attribution from scope throughout parser inputs, internal models, persistence,
graph request construction, API DTOs, and adapters. Rename misleading parser `user_id` inputs
where they actually mean document scope. Deduplicate by `(deal_id, content_sha256)` with the
existing current/live-version semantics. Identical bytes in different deals remain isolated.

Preserve existing file/version/chunk IDs during migration. New document content IDs and chunk
identity derivation use explicit deal scope; file-version IDs keep their existing file/hash rule.
Before switching derivation, persist a versioned identity manifest for existing versions, including
their exact chunk IDs, order, hashes, and generation. Retries of migrated versions reuse that
manifest and the canonical file/version IDs instead of generating a second set under deal scope.
New versions persist the same manifest shape with the new derivation version. Add the manifest
and its uniqueness/foreign-key constraints to the schema migration and recovery contract. Legacy
scope may be read by the controlled migration to reconstruct old identities, but is not retained
as a runtime authorization/search namespace. If a partially indexed version cannot be mapped
unambiguously, stop that migration item with a resumable error; do not invent IDs or duplicate
chunks. Verify recovery of these cases before cutover.

Carry the actor separately into the version write and activity transaction. Preserve immutable
preflight-before-SQLite requests, canonical identity comparisons, upload bounds, and the existing
SQLite-before-Helix failure semantics. UI must distinguish durable upload from failed indexing
and let an authorized contributor retry the same bytes without inventing success.

Store `(job_id, deal_id, actor_id, sender)` in process-local job state. Any current member with
read permission can subscribe to that deal's progress, but cannot access a job in another deal.
Sanitize shared failure events. Revalidate membership as well as session validity at stream
admission and at most every second; stop delivery when access is removed. An upload revoked
before canonical commit fails; indexing for a previously committed accepted version may finish
as trusted system work. Removal does not delete the accepted version. No restart durability is
promised for document jobs; surface lost jobs through document-list refresh and explicit retry.

Replace graph workspace scope with deal identity using canonical SQLite mappings. Do not
store memberships as the authority in Helix or introduce a full collaboration knowledge graph here.
For deal search, authorize the deal, select candidates for that deal and current live versions
before ranking, then batch-check returned identities against SQLite before exposing text.
Existing global search must cover all accessible deals: derive the authorized set server-side,
perform bounded scoped searches, and merge ranked candidates. Define the merge/dedup policy and
test it; BM25 scores from separate partitions are not automatically comparable. Prefer a supported
single scoped ranking query; if unavailable, use explicit rank fusion and bounded pagination.
Never substitute one creator's workspace partition for the authorized deal set.

Migrate graph properties and search partitioning with an idempotent, checkpointed maintenance
operation over stable IDs, covering partial and historical generations. Preserve stored embeddings;
rebuild affected indexes from existing vectors/text without silently recomputing embeddings. Verify
index replacement and property removal against the pinned SDK/server in a disposable instance;
current online docs alone are insufficient. Keep document writes and affected search drained during
cutover until identity manifests, graph migration, and scoped-query verification complete. Do not
mix creator-scoped writers with deal-scoped readers. Remove obsolete workspace fields/indexes
after verifying the replacement; temporary migration metadata is not a permanent namespace.
Document counts, failures, retries, and readiness checks. Do not run
`clear_helix`, reset SQLite, or silently report partial results as complete. A general graph rebuild
or collaboration outbox can follow separately; this release must at least support resumable
deal-scope migration, identity-manifest recovery, and exact-byte document-index retry.

## 7. Proposed API and shared UI

All paths below are relative to `/api/v1`; new contracts are proposed, not current endpoints.

| Method/path | Behavior |
| --- | --- |
| `GET /deals` and `GET /deals/{deal_id}` | Return accessible aggregates with actor role/capabilities and safe creator metadata; list retains existing archive behavior. |
| `GET /deals/{deal_id}/members` | Paginated minimal member profiles, roles, revisions, team IDs. |
| `POST /deals/{deal_id}/members` | Owner adds existing normalized username and non-owner role; duplicate membership returns 409. |
| `PATCH /deals/{deal_id}/members/{user_id}` | Owner changes non-owner role with expected revision. |
| `DELETE /deals/{deal_id}/members/{user_id}` | Owner removes a non-owner, or non-owner leaves; 204. |
| `POST /deals/{deal_id}/ownership-transfer` | Owner transfers to an active member with expected access revision. |
| `GET/POST /deals/{deal_id}/teams` | Paginated roster/create. |
| `PATCH/DELETE /deals/{deal_id}/teams/{team_id}` | Rename with expected revision/delete; DELETE returns 204. |
| `POST /deals/{deal_id}/teams/{team_id}/members` | Assign one existing deal member; duplicates are idempotent. |
| `DELETE /deals/{deal_id}/teams/{team_id}/members/{user_id}` | Remove assignment; 204. |
| `GET /deals/{deal_id}/activity` | Stable cursor page, default 30/max 100; server-generated event data. |
| `POST /deals/{deal_id}/documents/search/{keyword\|vector}` | Typed, bounded deal-scoped search; existing global routes remain membership-aware. |

Use minimal camelCase DTOs, bounded body/query sizes, opaque cursor validation, unknown-field
rejection, and explicit role enums. Authorization and target lookup precede mutation. Include
expected revisions on access changes; DELETE can carry its expected revision in a validated
query parameter to preserve the current native no-body DELETE shape. Concurrent duplicate
creation/owner transfer/removal must resolve through constraints or 409, never partial state.

Add operations to `QuarryApi` and both adapters. Extend native PATCH validation for exactly the
member/team route shapes and bounded JSON; retain notebook behavior. Validate numeric user IDs
and canonical resource IDs appropriately rather than applying notebook UUID rules to all IDs.
Reuse 204 DELETE and existing authenticated upload/SSE transport; no shell/filesystem/CSP expansion.

Add a Members and teams panel from the shared Deal Room header, a member-role indicator, uploader
display in stored-document views, and API-backed deal activity in the existing activity route.
Owners get add/change/remove/transfer controls; managers get team controls; all others see read-only
rosters. Capabilities drive affordances but never replace server checks. Keep source controls
creator-only and personal note/chat ownership explicit. Label legacy uploader as Unknown.

Reuse existing primitives/tokens and router manifest. Keep orchestration in a narrow feature
hook/data owner; derive display state and use keyed identity/deal owners. Do not add effect-body
state setters, duplicate caches, new state libraries, or fixtures masquerading as server data.
Handle loading, empty, error/retry, pending, conflict, and revoked access. Cancel stale requests,
restore dialog focus, label controls, announce mutation outcomes, and test keyboard navigation.
Refresh access on focus/navigation and management changes. A revoked active project clears
document bytes, search results, roster, previews, and annotations from app caches and exits to
the accessible deal list; downloaded/offline copies cannot be retracted. Clear session-only
annotation state on identity/deal-access loss without claiming annotation persistence.

## 8. Delivery sequence and acceptance gates

1. **Contract and policy inventory.** Enumerate all deal read/write paths and private linked
   resources; finalize capability DTOs, membership limits, cursor format, archived behavior,
   and source redaction. Review the exact-user-add flow and company update policy.
2. **Migration and canonical membership.** Implement schema upgrade, owner backfill, atomic
   project creation/transfer, team constraints, and transactional activity helpers. Test temporary
   schema-13 fixtures, malformed layouts, and rollback. No externally enabled sharing yet.
3. **Server authorization and ingestion.** Replace owner-only operations, apply private-resource
   exclusions, separate actor/storage scope, add attribution, and make job streams revocable.
   Gate: two members upload/read the same project; viewer writes and outsider reads fail.
4. **Graph/search compatibility.** Replace workspace scope with deal scope, persist identity
   manifests, migrate graph properties/index partitions resumably, and implement authorized candidate
   selection, global shared search, and canonical batch checks. Gate: shared search retrieves
   both users' uploads and never returns another deal's chunks, including stale projections.
5. **Cross-runtime adapters and UI.** Add management APIs/DTOs, native relay validation, member/team
   panel, attribution, and persisted activity. Gate: end-to-end web/desktop parity, revision
   conflicts, failed uploads/index retries, removed-member invalidation, and accessibility checks.
6. **Integrated review and rollout.** Run the checks below, review authorization route coverage,
   re-read/update canonical docs, and exercise a drained migration/backfill on disposable data.
   Enable sharing only after all runtime gates pass. Each intermediate change must remain
   functional and fail closed; do not ship management controls ahead of authorization/search.

## 9. Migration, compatibility, and rollback

Reserve marker 14 only if marker 13 is still current when implementation begins. Keep supported
12-to-current upgrade sequencing valid. In one immediate transaction create tables/indexes, add
attribution/access fields and identity-manifest tables, retire the redundant document workspace
column/indexes using a validated table rebuild if needed, and insert one owner membership for every existing deal (including archived
deals), validate owner counts/foreign keys/layout, and advance the marker. Preserve all existing
accounts, sessions, company assignments, file bytes/IDs/versions, notebooks, chats, and templates.
Fresh initialization must produce the same final schema as upgrading. Reject malformed/unsupported
layouts without deletion; validate the new invariant on restart without repairing silently.

Drain old backend writers, take a consistent pre-upgrade SQLite backup, record the matching binary
and graph configuration, deploy the coordinated backend and web/desktop clients, then perform
the identity-manifest backfill and resumable graph/index migration before enabling sharing. Retain
the drained backup as a controlled source for legacy identity reconstruction until migration is
validated; it is not a runtime namespace. If complete manifests require a staged upgrade, define
the staging schema and final cleanup markers explicitly before coding. Follow existing documented backup procedures;
do not copy a live SQLite main file without its WAL consistency requirements. Old binaries must
reject the new marker. Old clients may lack capabilities or shared-search behavior; require reload/
desktop update rather than claiming transparent compatibility. Existing SSE names remain stable.

Rollback before new writes uses the matching drained backup and binary plus compatible graph
state. After shared writes, preserve/export new records before choosing recovery; no automatic
downgrade, global graph clear, or database reset. Rollback must restore compatible graph properties
and index definitions as well as SQLite; a SQLite-only rollback is insufficient after removing
workspace properties/indexes. Preserve original nodes/embeddings and checkpoint incomplete work.
Ownership transfer never changes deal scope or document identities. A removed creator retains
provenance only, without residual access or a storage-namespace role.

## 10. Verification and definition of done

Add focused tests in the repository's manually included backend test modules; register each new
test module. Include:

- Membership joins without duplicate deals; exact owner backfill and no legacy data loss.
- Same-deal team foreign keys; last-owner protection; concurrent transfer/removal/revision races.
- Role matrix for every read/write/compatibility route, forged actor hints, foreign child/job IDs,
  archived behavior, and removed-creator denial after transfer.
- Two actors uploading identical/different bytes; correct attribution; same-deal deduplication;
  cross-deal identical bytes remain isolated; post-commit Helix failure retains canonical data.
- User-ID workspace values migrate to their actual deal IDs, including several deals owned by
  one user. Existing file/version/chunk IDs and embedding values remain unchanged; retrying a
  migrated version uses its manifest without duplicate chunks. New identity derivation uses deal
  scope. Final models, DTOs, queries, and indexes contain no document `workspace_id` dependency.
- Removal during multipart processing, before canonical commit, during SSE, and after accepted
  upload; no unauthorized late write, event delivery, or cache repopulation.
- Search before/after backfill, scoped top-k, shared global ranking, revoked permissions,
  stale/deleted/non-current graph hits, partial index failure, and bounded batch authorization.
- Private notebooks/chats/company directories remain private; shared company edits follow policy;
  no shared DTO/activity exposes local paths or credentials.
- Activity written atomically once, pagination across equal timestamps, unknown legacy uploader,
  retry/idempotency, minimal actor profiles, and removed-member attribution retention.
- Both API adapters, native PATCH allowlists, 204 DELETE, SSE cancellation/error handling, and UI
  empty/error/conflict/revocation/focus flows. Use synthetic fixtures in the prescribed fixture trees.

Run these implementation gates from the named build roots (not run for this plan-only change):

```sh
# backend/
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# frontend/
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui

# frontend/src-tauri/
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Run focused affected tests first using actual test filters/paths. Run the existing ignored Helix
v3 compatibility suite, extended for scoped candidate ranking/backfill, only against an explicitly
selected disposable instance; report the exact filter/configuration and outcome. No live provider
calls or production/local valuable data mutations are routine checks.

Manually inspect two separate authenticated sessions in web and desktop: create project; add
member; assign teams; upload as each contributor; view/search both uploads; verify uploader and
activity; deny viewer upload; transfer ownership; remove a member during an open preview/job;
verify denial and UI cleanup. Check responsive viewports, theme policy, reduced motion, keyboard,
focus, console/network errors, reload, and backend restart behavior. If starting a dev server,
record its PID/ports, stop only that task-owned process, and verify cleanup.

Before implementation handoff, update `docs/ARCHITECTURE.md`, `docs/DOMAIN_MODEL.md`, and
`docs/authentication.md` for actual membership ownership, routes/roles, private-resource exclusions,
schema/rollout, workspace-to-deal scope migration, identity manifests, graph backfill/search,
attribution, and process-local job limits.
Add a focused recovery/policy document if these details exceed the canonical narrative. Correct
older search/identity wording from current code; do not copy stale assertions from other plans.
Keep deferred annotations/findings/organization tenancy explicitly deferred. Perform multi-axis
code review, `git diff --check`, final diff/status inspection, and report exact check outcomes,
skipped integrations, and residual limitations. Do not claim a performance improvement without
representative measurements.

Plan-only architecture impact: none — this document proposes future work and changes no current
runtime, schema, ownership policy, API, or operational behavior.
