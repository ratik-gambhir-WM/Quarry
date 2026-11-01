# Shared deal projects: policy and recovery

The existing deal is the shared project. SQLite owns access, files, attribution and activity;
Helix owns a recoverable document projection. Teams organize active deal members and never grant
access on their own. Organization tenancy, email invitations/guests, shared notebooks/chats,
persistent PDF comments/findings and live co-editing remain deferred.

## Access policy

| Operation | Owner | Manager | Contributor | Viewer |
| --- | --- | --- | --- | --- |
| Read shared deal, uploaded documents, roster, activity, job progress and search | Yes | Yes | Yes | Yes |
| Upload documents | Yes | Yes | Yes | No |
| Update shared deal fields/links or extract metadata | Yes | Yes | No | No |
| Manage teams and assign existing members | Yes | Yes | No | No |
| Add/remove members or change roles | Yes | No | No | No |
| Transfer ownership or archive | Yes | No | No | No |
| Resolve device/server local sources | Only if also the original creator | No | No | No |

Non-owners can leave. Exactly one owner must remain; transferring to an existing member demotes
the prior owner to manager in the same transaction. `deals.user_id` is immutable creator provenance,
not authorization. Removing the creator removes their access. Archived deals retain reads and
block ordinary writes; owner revocation, non-owner leave and ownership transfer remain available.

An owner adds an existing registered user by exact normalized username. There is no public user
directory or email invitation flow. Minimal roster/actor identities exclude email and credentials.
Notebooks, assistant threads and company directories remain personal; a deal link never shares
those contents. Managers can retain attached company references and add their own company records.
Shared responses redact local roots. No collaborator acquires another device's native authorization.

Authentication still requires bearer credentials and a matching `X-User-Id`. Foreign/unknown
resources return 404; a member missing an operation capability receives 403; stale membership,
team or access revisions receive 409. A 403 does not invalidate the session. Admission checks
are repeated inside canonical write transactions. Read responses are rechecked before delivery.

## API and clients

The following paths are relative to `/api/v1`; the compatibility `/api` mount has the same policy.

| Method | Path | Contract |
| --- | --- | --- |
| GET | `/deals`, `/deals/{deal_id}` | Accessible aggregates; actor role, capabilities, access revision and creator ID |
| GET, POST | `/deals/{deal_id}/members` | Cursor roster; add registered username and non-owner role |
| PATCH, DELETE | `/deals/{deal_id}/members/{user_id}` | Expected member revision for role change; owner removal or self-leave |
| POST | `/deals/{deal_id}/ownership-transfer` | Active target member and expected access revision |
| GET, POST | `/deals/{deal_id}/teams` | Cursor roster; create named team |
| PATCH, DELETE | `/deals/{deal_id}/teams/{team_id}` | Rename with expected revision; delete team only |
| POST | `/deals/{deal_id}/teams/{team_id}/members` | Idempotent assignment of an existing deal member |
| DELETE | `/deals/{deal_id}/teams/{team_id}/members/{user_id}` | Remove team assignment only |
| GET | `/deals/{deal_id}/activity` | Stable cursor history; default 30/max 100 events |
| POST | `/deals/{deal_id}/documents/search/keyword` or `/vector` | Search one authorized deal |
| POST | `/documents/search/keyword` or `/vector` | Search every accessible deal, derived server-side |

Roster pages also default to 30/max 100; a deal has at most 500 members and 100 teams. Team names
contain 1–100 characters, collapse whitespace and use Unicode lowercase for uniqueness. Member
and team revisions, and the monotonic deal access revision, are bounded to JavaScript-safe integers.
DELETE and management PATCH operations return 204. Creates return 201 with JSON null.

Web and desktop share payload/path mapping. Native PATCH accepts only the existing notebook
paths and the two explicit member/team paths, with bounded JSON. Native origin, credentials,
file/PDF/MIME, URL and path-containment checks remain in place.

Settings contain member/team controls; Deal activity uses persisted events; Search documents has
“This deal” and “All accessible deals” choices. Stored documents identify the authenticated uploader
or display Unknown for legacy attribution. Read-only roles do not receive upload/edit affordances.
Request stores own subscriptions, retries, focus refresh, cancellation generations and stale
response suppression; APIs are not started during React render. The access boundary initiates
revalidation on navigation, and also refreshes on focus and every five seconds. An already checked
workspace remains mounted during revalidation. Access loss unmounts previews,
search results, upload controls and session annotations and removes the deal from the cache.
Previously downloaded/offline copies cannot be retracted.

## Documents, jobs and search

`quarry_files.deal_id` is the storage scope. Document models, parsers, graph properties and indexes
use deal scope. The authenticated uploader is stored separately on each accepted immutable version.
Exact-byte retries retain original attribution and write no second upload event. `quarry_current_content`
serializes current-content claims per deal, preventing concurrent duplicate logical attachments.
Identical bytes in distinct deals remain isolated.

`document_identity_manifests` records derivation version, original content ID, active generation
and exact chunk count; `document_chunk_identities` preserves IDs, generation, order and hashes,
including historical generations. New derivation is version 2 and uses deal scope. Legacy manifests
use version 1 and preserve the original IDs. Manifest validation, bytes, attribution and accepted
upload activity commit together before the immutable graph write. Partial indexing leaves canonical
bytes committed. Upload failures distinguish that state and permit explicit exact-byte retry.

Document jobs retain deal ID, actor ID and watch state in this process. Every current member may
subscribe, including viewers; session and membership are revalidated at least once per second
and before event delivery. Revocation before canonical commit rejects the upload; trusted indexing
of an already accepted version may finish. Restart loses document jobs; refresh the stored list
and explicitly retry the same bytes when needed. Shared failures contain sanitized recovery text.

Search first selects current, live, authorized chunk identities from SQLite, verifies complete graph
projection headers, ranks only those candidates, then checks canonical identities again before
exposing text. Vector search uses the pinned SDK's within-stream cosine ranking; distances from
the same configured embedding model are comparable across deals. The pinned SDK lacks a
within-stream text ranking operation. Keyword search therefore uses bounded lexical term-frequency
ranking, length normalization and deterministic ID tie breaks, with reciprocal-rank fusion across
deals (`1 / (60 + rank + 1)`). This is not a BM25 score comparison across partitions.

Search is explicitly bounded to 200 accessible deals globally, 5,000 current files and 10,000
canonical chunk identities per deal (including retained generations). Exceeding a bound returns an
explicit error; a missing/partial graph projection never masquerades as complete search results.
PDF-page fixture search remains a separate selected-document tool; API search links to the target
data room. Exact cross-document page activation and persistent PDF annotations remain deferred.

## Schema 14 rollout and recovery

Schema 14 preserves validated schemas 12 and 13, including accounts, sessions, notebooks, chats,
company references, templates, blobs and file/version IDs. Schema 12 receives notebook schema 13
first. One immediate transaction creates membership/team/activity/revision and identity tables,
backfills exactly one owner per existing deal (including archived deals), adds nullable uploader
attribution, rebuilds files without the obsolete user-valued workspace column/index, validates
foreign keys/layout/owners and advances the marker. Legacy versions retain null uploader attribution;
one system membership-backfill event per deal does not invent historical upload events.

The one-time Helix scope migration was executed for the selected local store and its source and
CLI were removed after verification, as requested. It is not a maintained product command.
The matching cold graph export, consistent pre-upgrade SQLite backup, maintenance executable and
source checkpoint remain together in ignored local backup storage for controlled recovery.
SQLite schema construction/validation and immutable identity manifests remain required runtime
code; removing the one-time utility does not remove these invariants.

For another unconverted store, keep old writers/search drained and take matching consistent
SQLite and cold graph backups before any offline maintenance. Use the matching reviewed maintenance
artifact rather than running a nonexistent repository command or starting mixed binaries. The
completed cutover requires exact identity manifests, canonical deal scope on all existing graph
file/version/chunk nodes, no legacy workspace properties, usable deal-partitioned indexes and the
`document_deal_scope_indexes=ready` checkpoint. Startup rejects missing manifests and unfinished
checkpoints; it never guesses legacy identities or silently repairs a graph.

The verified maintenance operation reconstructs immutable chunk IDs from canonical bytes and the
drained legacy scope, checkpoints them before graph writes, updates properties in place, compares
stored text/IDs/vector bit patterns, and replaces only affected index definitions. Missing index
definitions are accepted only through the pinned server's structured `index_not_found` response;
unknown write outcomes are not replayed. Ambiguous parser/graph mappings stop for recovery.
Images require their original stored description; the migration calls no provider. A completely
missing non-image projection can retain its reconstructed manifest, but migration cannot recreate
missing embeddings. Search fails explicitly until an authorized exact-byte upload repairs it.
Never mark an empty or partial projection as complete search results.

Before enabling normal sharing, verify owner/foreign-key/manifest invariants, scope properties,
index readiness and current-candidate search, and exercise two-user uploads, retry and revocation
against a disposable pinned runtime. Reload/rebuild matching web and desktop clients. Do not mix
old creator-scoped writers with deal-scoped readers.

Before new writes, rollback restores the matching drained SQLite backup/binary **and** compatible
graph/index state. After shared writes, preserve/export new records before choosing recovery.
A SQLite-only rollback cannot restore removed graph properties or index definitions. Never use
`clear_helix`, automatic reset or automatic downgrade for this rollout. A general graph rebuild
and collaboration outbox remain unimplemented.
