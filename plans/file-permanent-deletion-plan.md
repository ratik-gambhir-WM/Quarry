# File permanent deletion plan

Date: 2026-11-06. Status: proposed; not implemented.

This document records the agreed planning direction. Saving it changes no runtime behavior,
schema, API, or stored data. [Architecture](../docs/ARCHITECTURE.md) describes the implemented
product; this plan is a design input. Preserve the existing uncommitted work.

## 1. Outcome and scope

Add permanent deletion of one uploaded Quarry file, identified by `deal_id` and `file_id`.
Remove all of its versions, original bytes, recovery data, search text, embeddings, and graph
relationships from active SQLite and Helix storage. A temporary cleanup phase is permitted;
there is no trash or restore operation in this feature.

SQLite and Helix cannot commit one transaction together. Use a durable cleanup job so SQLite
content can be deleted first and Helix cleanup can finish after an outage or backend restart.
Do not report completion while either store still holds the targeted application records.

| Store or surface | Deletion scope |
| --- | --- |
| SQLite | The logical file, every version and blob, deduplication claims, identity manifests, chunk identities, image recovery snapshots, and file-specific activity references. |
| Helix | Every matching `QuarryFile`, `FileVersion`, and `FileChunk`, all generations, unpublished partial writes, incident relationships, and associated vector/text index entries. |
| Runtime state | Related ingestion jobs and retained events, preview work and caches, selected-file state, browser preview bytes/object URLs, and related route-local state. |

Preserve the deal, memberships, unrelated files, templates, notebooks, chats, and other deals'
copies of identical content. File deletion does not delete the source file on the user's computer
or remove independent backups. Logical record deletion does not promise immediate forensic
erasure of database pages, write-ahead logs, or underlying storage remnants; backup retention
and any stronger erasure requirement need a separate operational policy.

## 2. Current implementation anchors

- [SQLite document store](../backend/src/domains/documents/store/sqlite.rs) owns canonical
  files, versions, blobs, and current-content claims.
- [Identity manifests](../backend/src/domains/documents/store/identity.rs) retain immutable
  version/chunk identities independently of Helix.
- [Migrations](../backend/src/app/migrations.rs) own the shared schema, currently marker 17,
  and the existing foreign-key cascades. Coordinate the next additive marker at implementation time.
- [Ingestion](../backend/src/domains/documents/ingestion/service.rs) uses process-local
  per-attachment locks. [Document jobs](../backend/src/domains/documents/ingestion/job_service.rs)
  are in memory and cannot serve as the durable deletion queue.
- [Graph repository](../backend/src/domains/documents/graph/repository.rs) and
  [writer](../backend/src/domains/documents/graph/writer.rs) own document projection operations.
- [Stored-document viewing](../backend/src/domains/documents/viewing/service.rs) owns the
  bounded in-memory preview cache. Its current cache key does not contain the file identity.
- [Membership policy](../backend/src/domains/memberships/model.rs) defines server-verified
  deal roles. Owners and managers are the proposed actors allowed to delete files.

The retained deal-deletion proposal is broader than this feature and contains historical
assumptions. Re-check current code and canonical architecture instead of implementing its older
schema, authentication, or graph-scope descriptions.

## 3. Deletion workflow

### 3.1 Accept and authorize

Add a server-backed **Delete permanently** action for uploaded files in the Data Room. Confirm
the selected filename and explain that all saved versions will be removed.

Proposed API:

- `DELETE /api/v1/deals/{deal_id}/documents/{file_id}` accepts deletion and returns `202` with
  an operation ID when cleanup remains pending.
- Add an authenticated operation-status endpoint for the UI to track pending, retrying, and
  completed cleanup.
- Repeated requests must be idempotent. Resolve an existing pending operation before treating
  a missing canonical file as already deleted.

Validate transport inputs in the handler. Verify the authenticated actor's owner/manager policy
and the exact deal/file relationship server-side. Do not authorize by filename, job title,
caller-provided identity, or a client-side button state.

### 3.2 Prevent late writes

Coordinate deletion with ingestion, publication, and preview generation before removing data:

1. Acquire the shared ingestion/deletion coordination boundary for the attachment.
2. Persist a deletion fence and prevent new work from targeting the old file identity.
3. Cancel or drain related running and queued work, including already-prepared graph writes.
4. Prevent late callbacks from recreating SQLite records, republishing chunks, or repopulating caches.
5. Reject same-content re-upload within the deal while deletion is pending. Permit an intentional
   fresh upload after completion, with a new logical file identity.

Extend the existing local locking with durable state; a process-local mutex alone is insufficient
for restart recovery. Keep this within the supported single-host architecture and do not claim
multi-instance coordination without implementing and testing it.

### 3.3 Delete canonical SQLite content atomically

In one transaction, capture the exact cleanup scope in a durable deletion job and remove:

- `quarry_files` for the selected file.
- All corresponding `quarry_file_versions` and `quarry_file_blobs`.
- `quarry_current_content` claims for those identities.
- `document_identity_manifests` and `document_chunk_identities`, including retained generations.
- `document_image_recovery` snapshots.
- File/version-specific references in `deal_activity_events`.

Use the existing SQL builder, bound parameters, and validated foreign-key cascades. Verify
dependent rows are gone before committing. The cleanup job must not cascade away with the file.
It may temporarily retain only the scope, identifiers, and fencing/retry metadata needed to
finish deletion; do not copy filenames, document contents, blobs, or vectors into it.

If a generic deletion audit event is retained, define that explicitly in the migration and keep
it free of the deleted file's name, identifiers, hash, or content.

### 3.4 Purge Helix

The worker removes every document node belonging to the captured deal/file identity, including
all versions and index generations. Find partial and orphaned nodes directly by their properties;
cleanup must not depend solely on traversing published version edges.

Delete the associated relationships and ensure vector/text index entries are removed through
the supported Helix deletion behavior. Keep cleanup bounded and idempotent. Never delete by
filename or content hash alone, drop shared index definitions, or run `clear_helix`.

Use the existing process-serialized, durability-acknowledged writer boundary. Reconcile an
ambiguous write outcome by verification; do not add generic transport retries. Retry safe cleanup
phases with bounded backoff. Persist attempts and progress so startup resumes pending operations.

### 3.5 Verify and complete

Completion requires:

- No targeted canonical file, version, blob, claim, identity, recovery, or activity-reference rows.
- No matching Helix file/version/chunk nodes, relationships, or searchable vector/text entries.
- Related runtime jobs, retained results, and preview caches evicted, with late writes fenced.

Clear browser-selected-file state, rendered preview data, object URLs, and search/list caches.
Provide explicit invalidation hooks for connected consumers. Copies already downloaded or held
by disconnected clients cannot be retracted by this API.

Remove the cleanup identifiers after verified completion. If status polling requires a brief
completion receipt, retain only content-free operation/status and authorization metadata with
an explicit expiry. Show **Deleted** only after verification. Otherwise show **Deletion pending**
or **Retrying deletion** while the durable worker continues; never present partial cleanup as success.

## 4. Implementation boundaries

- Add an additive SQLite migration for the durable cleanup/fencing state and its invariants.
- Keep transport validation in the Axum handler, orchestration in a deletion service, SQLite
  mutation in a repository, and graph cleanup in the document graph owner.
- Assemble the worker and shared collaborators in bootstrap; do not construct clients or read
  ambient configuration from services/repositories.
- Extend `QuarryApi`, the web adapter, desktop adapter, and Tauri path/transport policy together.
  The existing desktop DELETE relay expects `204`; add typed handling for the proposed `202`
  JSON response without breaking existing deletion operations.
- Add the Data Room action with confirmation, pending/error/retry states, keyboard support,
  focus restoration, and clear completion feedback.
- Update canonical architecture and recovery documentation when implementation lands.

## 5. Existing orphan cleanup

Include a reviewed one-time cleanup path for graph records already orphaned by earlier
SQLite-only removal. Establish canonical absence and prevent concurrent publication before
deleting each exact graph identity. Preserve unrelated graph records and templates. Do not
interpret this plan as authorization to run that maintenance now.

## 6. Verification and acceptance criteria

Use temporary SQLite databases and an explicitly disposable, disk-backed Helix instance.

- Authorization rejects viewers/contributors and cross-deal file IDs.
- Every version/generation and partial projection is removed; another deal's identical file survives.
- A failure before the SQLite commit rolls back both content removal and job creation.
- A Helix outage after the commit leaves a durable pending job and no servable canonical file.
- Restart between stages resumes cleanup; unknown write outcomes are reconciled safely.
- Repeated DELETE requests converge without duplicate jobs or accidental wider deletion.
- Concurrent upload, indexing, preview generation, and deletion cannot resurrect the file.
- A fresh same-content upload succeeds after deletion completes and is protected from old cleanup.
- Cache invalidation and UI confirmation, pending, retry, completion, and focus behavior work.
- Cleanup identifiers expire or are removed according to the completion contract.

Run focused repository/service/route/adapter tests, then the affected frontend, Axum, and Tauri
gates from the repository verification guide. Never verify against valuable local data or use a
development reset as part of this feature. Inspect the final diff and update architecture/recovery
documentation before handoff.
