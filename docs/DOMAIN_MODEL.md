# Quarry domain and data model

# Version 1

| Field | Value |
| --- | --- |
| Status | Current-state companion to `ARCHITECTURE.md` |
| Last verified | 2026-09-25 |
| Repository snapshot | Live working tree; optional legacy Diligence Studio is in a separate checkout |
| Audience | Quarry product engineers, reviewers, and coding agents |
| Scope | Product-domain ownership, durable entities, projections, ephemeral models, and known gaps |

This document explains what Quarry's domains mean and which data models actually exist today. It
is a companion to [`ARCHITECTURE.md`](ARCHITECTURE.md), which remains the canonical source for
runtime topology, request routes, infrastructure, configuration, and verification. Code,
manifests, migrations, and tests remain authoritative when either document drifts.

Here, **model** means the product and data model, not an OpenAI or other AI model selection.
Configured AI models are infrastructure concerns described in the architecture guide.

## 1. How to read the domain tree

Quarry's Axum backend is a modular monolith. A domain is a product capability boundary, not a
deployment boundary or necessarily a database table. An implemented domain may own routes,
handlers, services, domain models, and persistence ports. Concrete SQLite, Helix, AI, Office, and
external-service mechanisms live under `backend/src/adapters/` and are injected at bootstrap.

The directory tree deliberately contains both active modules and comment-only ownership markers:

- **Implemented** means the module is registered from `backend/src/domains/mod.rs`, assembled in
  `backend/src/app/bootstrap.rs`, and has working service or route behavior.
- **Partial** means some related behavior exists, but the named domain does not yet own a complete
  durable lifecycle or production boundary.
- **Reserved** means the directory states intended ownership only. A reserved `mod.rs` is not an
  API, entity, table, service, or claim that the product capability works.

Do not activate a reserved domain by adding it to the registry alone. Define its model,
invariants, authorization policy, persistence or external capabilities, service boundary, routes,
and tests first.

## 2. Domain map

### 2.1 Active domains

| Domain | Current responsibility | Durable model | Important boundary or limitation |
| --- | --- | --- | --- |
| `users` | Development profile creation and lookup by email | `users` | Profile lookup is not authentication; the development-era `api_key` is not an AI provider key |
| `deals` | Deal lifecycle, metadata, data-source selection, and file-assisted key-question extraction | `deals`, `deal_metadata` | Archive sets `archivedAt` while retaining the stage; owner email is resolved through the public `UserDirectory` facade |
| `data_rooms` | Browse and preview a deal's configured local filesystem source | None of its own | Reads a local root from deal metadata or server configuration; does not own stored document blobs or activate SharePoint |
| `documents` | Parse, ingest, persist, version, index, search, and view deal documents | `quarry_files`, `quarry_file_versions`, `quarry_file_blobs`; Helix projection | SQLite is canonical; Helix is a search projection; jobs and locks are process-local |
| `summaries` | List summarizable server files, summarize paths/uploads, and write Markdown | None | OpenAI-backed when configured; filesystem-oriented operations have a development trust model |
| `research` | Forward WM AI file, index, status, and GraphRAG workflows | None in Quarry | Operates on upstream WM AI models; broader product research ownership is not implemented |
| `templates` | Expose and mutate the validated app-scoped slide-template catalog | None in Quarry's canonical store | The optional legacy Diligence Studio service, maintained in a separate checkout, owns its ephemeral PPTX conversion, slide splitting, previews, and catalog rows; no deal ownership or selection |
| `system` | Health/capability/database-status operations | Reads infrastructure state only | Health is shallow; database status can expose a server path and has development-only posture |
| `dev_support` | Demo greeting/login-event behavior | None | Explicitly development/demo behavior, not a product identity or event domain |

The registered backend domain list is defined by
[`backend/src/domains/mod.rs`](../backend/src/domains/mod.rs). Bootstrap decides which concrete
capabilities each domain receives and merges already-state-bound feature routers.

### 2.2 Partial and reserved domains

| Domain | Intended ownership | Current reality |
| --- | --- | --- |
| `identity` | Authenticated principals and credential verification | Reserved; no inbound authentication or principal extractor |
| `workspaces` | Tenant workspace lifecycle | Reserved; no workspace table or trusted workspace aggregate |
| `memberships` | Principal-to-workspace roles and access policy | Reserved; no server-side tenant authorization |
| `user_settings` | Durable user preferences | Reserved backend boundary; current theme/preferences are client-local |
| `connections` | External data-source connection lifecycle | Reserved; SharePoint adapter is dormant and the UI modal does not submit an integration |
| `diligence` | Questions, findings, evidence, and review state | Reserved; some frontend review/search content is fixture-derived |
| `workflow` | Durable tasks, approvals, and workflow execution | Reserved; there is no durable queue or workflow engine |
| `deliverables` | Generated/user-managed deal artifacts and provenance | Reserved; current completed/in-progress UI has no backing API |
| `vaults` | Durable knowledge collections and access rules | Reserved; current vault screens are presentation/staging experiences |
| `notebooks` | Owner-scoped notebooks, folders, native notes, durable save jobs and Helix projection | Durable API, both transports and explicit-save editor; folder UI deferred |
| `unified_search` | Authorized cross-domain search composition | Reserved; implemented search is document-chunk search only |
| `assistant` | Conversations, messages, citations, and interaction orchestration | Reserved; no assistant conversation API or durable thread model |
| `activity` | Durable product activity history | Reserved; frontend activity logs are session-local and separate |
| `notifications` | Preferences, delivery requests, attempts, and retry policy | Reserved; no notification queue or delivery adapter is active |

The separate `templates` and `deliverables` boundaries are intentional: reading or importing an
upstream template does not create a deliverable, associate one with a deal, or implement artifact
generation.

## 3. Active domain relationships

```mermaid
flowchart LR
    Users[users\nprofile directory]
    Deals[deals\ndeal + metadata lifecycle]
    DataRooms[data_rooms\nlocal source read model]
    Documents[documents\ncanonical files + index]
    Summaries[summaries\nsummary use cases]
    Research[research\nWM AI workflows]
    Templates[templates\npreview catalog]

    Users -->|UserDirectory| Deals
    Deals -->|DataRoomSourceReader| DataRooms
    Deals -->|deal_id + owner record| Documents
    Documents -->|public document capabilities only| Search[document search/viewing]

    Summaries --> OpenAI[OpenAI adapter]
    Research --> WMAI[WM AI adapters]
    Templates --> DSAdapter[Diligence Studio adapter]
    DSAdapter --> DS[Diligence Studio service\nin-memory catalog]
    DataRooms --> FS[Configured server filesystem]
    Documents --> SQLite[(SQLite)]
    Documents --> Helix[(Helix)]
```

Cross-domain dependencies use small public facades rather than another domain's private handler,
service, or repository modules:

- `deals` asks `users::UserDirectory` to resolve a profile by email.
- `data_rooms` asks `deals::DataRoomSourceReader` for the configured local root.
- `documents` owns document storage/indexing and validates its deal relationship at persistence;
  it does not make `data_rooms` the owner of uploaded document bytes.
- `summaries`, `research`, and `templates` depend on injected adapter capabilities, not on each
  other's internal models.

Handlers validate transport input and call services. Services own use-case transitions.
Repositories own persistence/index operations. Adapters implement concrete mechanisms. None of
these lower layers reads ambient configuration or imports application-wide state.

## 4. Canonical relational model

SQLite schema version 7 is the canonical durable product store. The active relationships are:

```mermaid
erDiagram
    USER ||--o{ DEAL : owns
    USER ||--o{ DEAL_METADATA : records
    DEAL ||--o| DEAL_METADATA : has
    DEAL ||--o{ QUARRY_FILE : contains
    QUARRY_FILE ||--|{ QUARRY_FILE_VERSION : versions
    QUARRY_FILE_VERSION ||--|| QUARRY_FILE_BLOB : stores

    USER {
        integer id PK
        text email UK
        text first_name
        text last_name
        text api_key
        text role
        text created_at
        text updated_at
    }
    DEAL {
        text deal_id PK
        integer user_id FK
        text deal_name
        text status
        text start_date
        text close_date
        text transaction_type
        text target_company
        text primary_buyer
        text deal_sponsor
    }
    DEAL_METADATA {
        text deal_id PK
        integer user_id FK
        json key_questions_json
        text local_path nullable
        text sharepoint_link nullable
        text sow_link nullable
        text fact_sheet_link nullable
        text rl_link nullable
    }
    QUARRY_FILE {
        text file_id PK
        text deal_id FK
        text workspace_id
        text display_name
        text source_uri nullable
        json metadata_json
        text deleted_at nullable
    }
    QUARRY_FILE_VERSION {
        text version_id PK
        text file_id FK
        integer version_number
        text original_filename
        text mime_type
        text content_sha256
        integer byte_size
        boolean is_current
    }
    QUARRY_FILE_BLOB {
        text version_id PK
        blob file_bytes
    }
```

`app_metadata` and `reminders` also exist in schema version 7, but no current registered domain
service or route consumes them. They are dormant schema, not evidence of implemented application
metadata or reminder domains.

### 4.1 Development user profile

A `User` is currently a development profile with a numeric SQLite ID and unique email. Deals use
the numeric ID as their owner reference. The profile's `api_key` field is returned by current API
responses, but Quarry's OpenAI features use server-side `OPENAI_API_KEY`; creating a profile does
not configure AI.

There is no authenticated principal attached to requests. Email, `userId`, and `workspaceId`
values supplied by clients are not trustworthy identity assertions.

### 4.2 Deal aggregate

A `Deal` is the primary current product aggregate. Its stable key is `deal_id`; creation currently
requires an ID beginning with `DEAL-`, a non-empty name, typed engagement/transaction/status fields,
and valid optional date-only values. Engagement completion cannot precede start; transaction
closing can precede diligence.

`DealMetadata` is an optional one-to-one extension keyed by `deal_id`. It owns:

- extracted key questions serialized as a JSON array;
- an optional local data-room path; or
- an optional HTTPS SharePoint link.

The local path and SharePoint link are mutually exclusive, but both may be absent. A SharePoint
link is metadata only today. Archiving sets `archivedAt` and retains the diligence stage; it does not delete the deal,
metadata, files, versions, or blobs.

The ordered diligence stages are Pipeline (`pipeline`), Prepare (`prepare`), Review & plan
discovery (`review_plan_discovery`), Interviews & site visit (SV) (`interviews_site_visit`),
Debrief & flash report (`debrief_flash_report`), Follow-ups & analysis (`follow_ups_analysis`),
QA, readout & closeout (`qa_readout_closeout`), Closed (`closed`), and Pencils Down (`pencils_down`).
Closed and Pencils Down are Historic outcomes; Pencils Down indicates stopped work. All other
stages are Current. Platform, add-on and strategic acquisitions require exactly one target in
working stages and Closed; Pipeline and Pencils Down permit incomplete parties. Users can select
stages in any order. Stage changes never assign or clear transaction/engagement dates. Archiving
independently sets `archivedAt`, retaining the stage and children while excluding normal lists.

Schema 12 enforces the stage codes. See [development reset](authentication.md#schema-13-and-development-reset).

### 4.3 Logical file aggregate

A `QuarryFile` is the stable logical attachment within a deal/workspace scope. It owns one or more
immutable `QuarryFileVersion` records, and each version owns exactly one byte blob.

Core invariants enforced by schema and repository logic include:

- a file belongs to exactly one existing deal and one stored `workspace_id`;
- a reused `file_id` cannot move between deals or workspace IDs;
- soft-deleted files are excluded from active reads and cannot be silently restored by ingestion;
- version numbers and content hashes are unique within a file;
- at most one version is current for a file;
- a version ID, byte size, content hash, and blob must agree for an idempotent write;
- child versions/blobs cascade when the owning file or version is deleted;
- an archived deal rejects new document persistence but retains existing records.

The current ingestion path uses the request's normalized `userId` value as `workspace_id` and
checks it against the deal owner's normalized email before persistence. This is a consistency
check, not authorization, because the request has no authenticated principal. The naming reflects
an incomplete tenant model and should not be generalized into a trusted Workspace entity.

## 5. Document identity and version lifecycle

The parser and persistence layers use several identifiers with different meanings:

| Identifier | Current derivation | Meaning |
| --- | --- | --- |
| `deal_id` | Client-supplied, validated `DEAL-...` string | Deal aggregate identity |
| `file_id` | Random on parse unless exact-content lookup reuses an existing file | Logical file identity |
| `content_sha256` | SHA-256 of original bytes | Exact content identity |
| `document_id` | SHA-256 of normalized request identity plus content hash | Ingestion/result identity; not the logical file key |
| `version_id` | SHA-256 of file ID plus content hash | Immutable file-version identity |
| `chunk_id` | Deterministic from file/version/index generation, order, and chunk hash | Search projection chunk identity |
| `index_generation` | Currently the version ID | Identifies the projected chunk generation |

The ordinary ingestion lifecycle is:

1. Validate the deal/request and compute the uploaded content hash.
2. Look for a current SQLite attachment with the same deal, stored workspace identity, and exact
   content hash.
3. If both SQLite and Helix already contain the same current version, report the upload as
   successfully skipped.
4. Otherwise parse PDF/DOCX, produce normalized chunks, and generate embeddings.
5. Persist the logical file, immutable version metadata, and original bytes in one SQLite
   transaction.
6. Project the committed file/version/chunks into Helix.

Parsers assign a new random `file_id`. Exact-content re-upload can reuse a committed file ID, but
changed content normally becomes a new logical file rather than version 2. The schema and
repository support later versions when a caller explicitly supplies the same file ID; ordinary
ingestion does not yet resolve changed bytes to an earlier logical attachment.

## 6. Helix search projection

Helix is not a second canonical store. It contains the query-oriented projection:

```text
QuarryFile
  ├── HAS_VERSION ───────> FileVersion
  └── CURRENT_VERSION ───> FileVersion
FileVersion
  └── HAS_CHUNK ─────────> FileChunk
```

- `QuarryFile` projects workspace ID, file ID, and display name.
- `FileVersion` projects immutable version/content metadata and index generation.
- `FileChunk` projects ordered text, embedding, hashes, token count, page/character ranges,
  section path, and timestamps.
- Keyword and vector search operate on current projected chunks within a caller-supplied workspace
  identity.

SQLite commits before Helix indexing. If the Helix write fails, the canonical SQLite version and
blob remain committed. Re-uploading the exact bytes can reuse that identity and retry projection.
There is no general SQLite-to-Helix reindex command, so canonical rows must not be deleted merely
to hide a projection failure.

## 7. Non-canonical read and process models

Not every type returned by a domain is an entity that Quarry owns durably.

### 7.1 Local Data Room tree

`DealDataRoom` and `DataRoomTreeNode` are request-time filesystem read models. The `data_rooms`
domain resolves a configured root, canonicalizes it, walks visible files/folders, and returns
relative paths plus preview metadata. It does not copy those files into SQLite. Local PDF previews
read validated bytes; DOCX/XLSX/PPTX previews are converted through the injected Office adapter.

The frontend currently merges this local tree with stored `documents` summaries for display. That
merge is a UI read model, not a backend aggregate and not proof that both sources have identical
lifecycle or authorization semantics.

### 7.2 Document jobs

`DocumentJobEvent` is a process model with `processing`, `completed`, `skipped`, and `failed`
states. Job records are in-memory Tokio watch channels, not durable entities. They are lost on
restart and cannot coordinate multiple backend instances.

### 7.3 Summaries and research

Summary text, selected paths, WM AI uploads, WM indexes, status responses, and GraphRAG query
responses are use-case DTOs or externally owned records. Quarry does not persist a Summary or
Research aggregate today.

### 7.4 Slide templates and previews

`TemplatePreviewPage`, pagination, and preview records are validated service-owned read models
supplied by the optional external Diligence Studio process. Single-slide and deck PPTX imports are
service writes whose compact Quarry results contain only the selected mode and imported/warning
counts. Diligence Studio converts and stores one template per selected source slide in its own
in-memory SQLite catalog; Quarry's canonical SQLite and Helix stores receive no template copy. A
template preview is not a deliverable and has no current deal ownership, selection, or generation
lifecycle. Built-ins reseed from tracked catalog files when Diligence Studio starts. Imported
templates, assets, previews, classifications, and app registrations disappear on process restart,
and imports without a generated preview remain absent from Quarry's preview-only gallery.

### 7.5 Frontend view models and fixtures

Types such as `WorkspaceDeal`, `DealRoomData`, `DataRoomTreeNode`, file-review rows, timeline items,
and search results may combine server data with explicit frontend fixtures. They are presentation
models, not automatically canonical backend entities. Fixture-backed findings, metrics, tasks,
timeline entries, and search excerpts must not be documented or treated as persisted facts.

### 7.6 Notebook edit buffers and saves

The Notebook UI holds local edit buffers in frontend module memory for the active
authentication session. Each draft has a UUID, title, native restricted-schema BlockNote blocks,
`contentSchemaVersion: 1`, and creation/edit timestamps. The URL selects a draft; changing routes
retains it, while reload and authentication changes clear it. Search filters all session titles.
Explicit Save chooses/creates a notebook for a new note and later PATCHes the same server note
with its revision. Saved-note summaries load after sign-in and full native blocks load on selection.
A session-scoped external store tracks immutable operation snapshots, jobs, revisions and content
acknowledgements. Completion marks only that submitted snapshot Saved; newer edits remain unsaved.
An uncertain response reuses its operation ID or reconciles status, while indexing retry resumes
the same job. A loaded canonical note is labelled Stored on server until a completed save confirms
indexing. Conflicts require an explicit reload; saved-note deletion is server-backed. The edit buffer
is not canonical storage; only saved content survives reload/sign-out. Autosave and collaboration
remain absent.

### 7.7 Durable notebook aggregate

`notebooks.user_id` is the ownership authority. A notebook may reference one same-owner deal or
stand alone; multiple notebooks may share a deal link. Folders have one notebook and no parent.
Notes require one notebook and may reference a folder in that notebook, enforced by a deferred
composite FK. Notes store native BlockNote blocks/schema version, title, immutable IDs and audit
times, plus a JS-safe revision. Every save/move increments revision. Folder deletion preserves
content by relocating notes to root and incrementing revisions; notebook deletion cascades.

`notebook_save_jobs` stores actor-scoped operation IDs/request hashes, immutable accepted input,
committed revision, pinned pipeline fingerprint, state/stage, attempt/sequence, leases and sanitized
terminal outcomes. A completed save confirms both SQLite and a published Helix revision. Failed
indexing retains canonical content and supports retry without reapplying the mutation. Independent
`notebook_cleanup` tombstones survive parent cascades and drive recoverable physical graph cleanup.
Helix's notebook revision/chunk labels are separate from document and template projections. There
is no notebook search integration yet. See [notebook API and recovery](notebook-api.md).

## 8. State ownership by durability

| Class | Current examples | Owner | Lifetime/recovery |
| --- | --- | --- | --- |
| Canonical durable state | Users, deals, metadata, files, versions, blobs, notebooks/folders/notes/save jobs | SQLite through owning domain repositories | Survives restart; subject to current schema migration policy |
| Search projection | File/version/chunk graph | Helix through `documents::graph` | Rebuild intended from SQLite, but general tooling is missing |
| External read/process state | WM AI indexes/results | External provider; Quarry adapter/service maps it | Provider-defined |
| Legacy service ephemeral state | Diligence Studio templates, assets, previews, classifications, and app registrations | Diligence Studio in-memory SQLite repository | Seeded built-ins return on restart; imports do not |
| Server ephemeral state | Document jobs, ingestion locks, Office preview cache | Owning service/process | Lost on restart; not multi-instance |
| Desktop ephemeral authority | Authorized local roots | Tauri process | Lost on desktop restart |
| Client session state | Activity log, route state, modal/view state | React/module/session storage | Tab/webview or route lifetime |
| Development fixtures | Portfolio, review, search, and other placeholder records | `frontend/src/fixtures/` | Shipped development data; never authoritative |

## 9. Known model gaps

- Identity, workspace, and membership aggregates do not exist, so current resource scoping is not
  tenant authorization.
- `workspace_id` in the document model currently carries a normalized client-supplied identity
  that is checked against deal-owner email; it is not backed by a Workspace table.
- The development `users.api_key` field is exposed through profile DTOs and should not be treated
  as a production-safe secret model.
- Changed-content uploads usually create new logical files rather than later versions of an
  existing attachment.
- SQLite-to-Helix recovery is incomplete without a general reindex command.
- Document jobs, workflow-like processing, caches, and locks are not durable or distributed.
- Local paths and SharePoint links describe mutually exclusive deal sources, but only local
  filesystem browsing is active; SharePoint import is not wired.
- Diligence questions/findings/evidence, deliverables, vaults, assistant conversations,
  notifications, and durable activity have reserved owners but no canonical backend models.
- `reminders` and `app_metadata` tables have no current domain consumer and should not be expanded
  until ownership is decided.
- Search endpoints trust caller-supplied workspace identity, and the Data Room search overlay is a
  separate fixture-backed frontend experience rather than a consumer of those endpoints.

## 10. Rules for extending the model

When adding or changing a domain model:

1. Name the owning domain and decide whether the data is canonical, projected, external, ephemeral,
   or fixture-backed.
2. Define stable IDs, scope/ownership, lifecycle states, invariants, deletion/retention behavior,
   idempotency, and failure recovery before choosing storage.
3. Enforce transport facts in handlers, use-case transitions in services, and storage invariants in
   repositories or model helpers.
4. Keep adapters infrastructure-specific and inject them from bootstrap; domains must not construct
   clients or read environment variables.
5. Use public domain facades for cross-domain collaboration. Do not import another domain's private
   handler, service, or repository modules.
6. Treat authentication and authorization as server boundaries. Client route state, IDs, CORS, and
   Tauri validation are not authorization.
7. For SQLite changes, define forward migration and recovery behavior, update migration/invariant
   tests, and protect existing local data. The current pre-v6 migration is destructive.
8. For Helix changes, preserve SQLite ownership, deterministic projection identity, and the
   reindex/rollback constraints in `ARCHITECTURE.md`.
9. Update this document when domain ownership or model semantics change, and update
   [`ARCHITECTURE.md`](ARCHITECTURE.md) when the runtime boundary, route/API, schema, integration,
   security posture, or operational behavior also changes.

## 11. Primary implementation references

- Domain registry and ownership markers: [`backend/src/domains/`](../backend/src/domains/)
- Application composition: [`backend/src/app/bootstrap.rs`](../backend/src/app/bootstrap.rs)
- SQLite schema: [`backend/src/app/migrations.rs`](../backend/src/app/migrations.rs)
- User model/repository: [`backend/src/domains/users/`](../backend/src/domains/users/)
- Deal model/repository: [`backend/src/domains/deals/`](../backend/src/domains/deals/)
- Local Data Room read model: [`backend/src/domains/data_rooms/`](../backend/src/domains/data_rooms/)
- Document canonical store: [`backend/src/domains/documents/store/`](../backend/src/domains/documents/store/)
- Document ingestion and identity mapping: [`backend/src/domains/documents/ingestion/`](../backend/src/domains/documents/ingestion/)
- Helix projection/search model: [`backend/src/domains/documents/graph/`](../backend/src/domains/documents/graph/)
- Shared identifier functions: [`backend/src/shared/ids.rs`](../backend/src/shared/ids.rs)
- Frontend transport DTOs: [`frontend/src/contracts/quarryApi.ts`](../frontend/src/contracts/quarryApi.ts)
- Frontend presentation models: [`frontend/src/data/`](../frontend/src/data/)
- Current runtime/API/data architecture: [`docs/ARCHITECTURE.md`](ARCHITECTURE.md)
- Versioned Helix rollout and recovery: [`ARCHITECTURE.md` section 9.2](ARCHITECTURE.md#92-helix-versioned-file-graph)
