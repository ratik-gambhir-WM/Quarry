# Whole-product backend domain modularization plan

Status: proposed
Scope: product-wide backend domain boundaries, including implemented, UI-implied, and planned capabilities
Baseline: inspected on 2026-09-02 against the active working tree
Architecture style: modular monolith with vertical domain modules, followed by optional crate extraction

Design inputs: the live source tree, [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md),
[`quarry-send-query-sse-plan.md`](quarry-send-query-sse-plan.md), and
[`quarry-query-chat-ui-plan.md`](quarry-query-chat-ui-plan.md). The local plans are design inputs,
not implemented product contracts.

## Outcome

Define the backend domain map for the whole Quarry product, then reorganize the implemented Axum
code around those business capabilities instead of the current top-level `handlers/`, `services/`,
`repository/`, and `core/` layers. Each implemented capability should keep its HTTP delivery, use
cases, domain types, persistence code, and focused tests together behind a narrow module facade.
The map must also reserve clear ownership for product capabilities already implied by the UI and
existing local design plans—users, user settings, workspaces, LLM interactions, durable conversations,
diligence, tasks, deliverables, vaults, integrations, and unified search—without pretending those
capabilities are implemented today.

The first implementation should remain one `quarry-backend` crate. Rust modules already provide
privacy and compile-time boundaries, while an immediate multi-crate workspace would force many
currently internal types to become public before the document boundaries are clean. Structure the
modules so a later crate extraction is mechanical, but do not create crates merely to move files.

The target should:

- make a feature discoverable from one folder;
- preserve `/api/v1`, the temporary `/api` compatibility mount, JSON shapes, statuses, multipart
  fields and limits, PDF headers, and document-job SSE events;
- keep `main -> config -> bootstrap -> modules -> router` as the construction direction;
- replace the global service-locator-style `AppState` with feature-owned router state;
- keep adapter construction in the composition root;
- keep SQLite canonical and Helix a recoverable document-search projection;
- retain all transaction, path, file, MIME, PDF, concurrency, and error-sanitization invariants;
- expose cross-domain reads through explicit, narrow facades instead of importing another
  domain's repository;
- leave the frontend and Tauri contracts unchanged during the reorganization;
- avoid schema, migration-version, provider, or dependency changes in the move-only phases.

## Recommendation in one sentence

Use a product-wide `domains/` tree organized around accounts, workspaces, deals, content,
diligence, workflows, AI assistance, knowledge, and platform operations; make `documents` only one
of those contexts, keep concrete technical adapters under `adapters/`, keep application
composition under `app/`, and keep `shared/` deliberately small.

## Whole-product domain landscape

The backend should not derive its module map only from today's API routes. The shared frontend and
the two existing query/chat plans show both current capabilities and the next product seams. Use
four maturity labels throughout implementation and architecture documentation:

- **Implemented:** backed by active Axum behavior and/or durable data.
- **Partial:** some real behavior exists, but important data or workflow remains local, transient,
  fixture-backed, or disconnected.
- **Planned:** specified in a local design plan but not present in executable code.
- **Conceptual:** visible in navigation or product composition, but not yet specified as a backend
  contract.

### Domain catalog

| Bounded context | Domain modules | Maturity on this baseline | Intended ownership |
| --- | --- | --- | --- |
| Accounts | `users`, `user_settings` | Users: implemented development profile flow. User settings: partial client-only theme/preferences UI. | User profile and lifecycle in `users`; persisted personal preferences, accessibility, notification choices, and safe model defaults in `user_settings`. Neither owns authentication tokens or provider secrets. |
| Identity and tenancy | `identity`, `workspaces`, `memberships` | Conceptual; current email/router state and caller-supplied workspace IDs are not security. | Authentication principals, workspace lifecycle, membership, roles, and tenant authorization. This becomes the policy source for all workspace-scoped domains. |
| Deal management | `deals` | Implemented core records and metadata extraction; activity/site visits/deliverables are partial or placeholders. | Deal aggregate, lifecycle, ownership reference, source selection, metadata, and deal-specific extraction. |
| Data sources | `data_rooms`, `connections` | Local data rooms: partial/implemented. SharePoint connection: UI/dormant client only. | Deal data-room roots/source inventory in `data_rooms`; user/workspace-managed external connection metadata and status in `connections`. Provider protocol remains an adapter concern. |
| Documents | `documents::{catalog, ingestion, viewing, indexing}` | Implemented for stored PDF/DOCX flows; other parsers are isolated only. | Canonical logical files, versions, blobs, parsing, preview, and the document projection writer. |
| Search | `document_search`, `unified_search` | Document keyword/vector API: implemented. Unified search UI: placeholder. | Document-only retrieval remains close to documents; cross-product search owns a read model spanning deals, documents, diligence, research, and conversations when it exists. |
| Diligence | `diligence::{workstreams, questions, reviews, findings, evidence, data_points, requests}` | Mostly fixture-backed or placeholder; extracted key questions currently live in deal metadata. | Structured diligence work, source-grounded findings, evidence links, risks/opportunities, key questions and answers, requested data, and synthesis inputs. |
| Workflow and collaboration | `workflow::{tasks, assignments, calendar, site_visits}`, `notifications` | Tasks/timeline are UI-local; `reminders` table has no service; operations navigation is conceptual. | Assignable work, statuses, due dates, scheduling, site visits, and user notification delivery/preferences. |
| Deliverables | `deliverables` | Placeholder UI. | Deal outputs, versions, review/approval state, exports, and links to supporting findings/evidence. |
| Knowledge library | `vaults`, `notebooks`, `templates` | Vault staging/activity and navigation placeholders only. | Reusable workspace content, saved evidence, notes/notebooks, and reusable diligence or deliverable templates. |
| AI assistant | `assistant::{interactions, conversations, context}` | Single-shot query transport/UI is planned; current chat panel is inert; no durable history. | LLM request/run lifecycle, streaming/cancellation, durable conversations/messages, context attachments, citations, and model-visible interaction metadata. |
| Analysis | `summaries` initially; later generalized analysis only with evidence | Implemented summary endpoints; generated diligence outputs are mostly fixture-backed. | User-requested document summaries and their inputs/outputs. Deal extraction remains in deals; embeddings remain in documents; chat remains in assistant. |
| Research | `research` | WM AI operations implemented; research library/expert calls/market signals are conceptual. | Research jobs, sources, saved research artifacts, expert-call records, and market signals; WM transport is an adapter. |
| Activity and audit | `activity` | Browser activity log is session-local only; deal timeline is local/fixture-backed. | Product activity/events and a future durable audit history. Transport diagnostics and server telemetry remain operational concerns, not product activity. |
| System operations | `system`, `dev_support` | Implemented health/capabilities/database diagnostics and demo endpoints. | Health/capability reporting and isolated development-only routes. |

This catalog is also the scaffold map. During the structural reorganization, create one directory
for every planned or conceptual top-level domain and place a literally empty `mod.rs` in it. Do not
declare that module from `domains/mod.rs`, register routes, construct state or services, add tests,
or create tables, DTOs, adapters, dependencies, or other implementation until its first real use
case lands. The empty file reserves the folder structure without pretending the capability is
implemented. Keep each scaffold's ownership and maturity documented here so new features do not
default into `core`, `services`, or a generic "AI" folder.

### Frontend surface to backend ownership

| Current or implied product surface | Backend owner(s) | Baseline truth |
| --- | --- | --- |
| Login and Account Info | `users`; later `identity` and `user_settings` | Profile lookup/creation exists; authentication and persisted settings do not. |
| Theme/profile preferences | `user_settings` | Theme is client-local and the picker is currently disabled; no backend contract exists. |
| Portfolio hub and Deals table/Kanban | `deals`, later `unified_search` | Persisted deals exist; some portfolio content remains fixture-derived. |
| Deal Room overview | `deals`, `diligence`, `workflow`, `activity` | Deal fields persist; metrics, insights, tasks, and timeline are local/fixture-backed. |
| Deal Activity and pending tasks | `activity`, `workflow` | UI-local state only; the dormant `reminders` table is not an implemented workflow domain. |
| Site Visits | `workflow::site_visits` | Placeholder only. |
| Deliverables | `deliverables` | Placeholder only. |
| Data Room Vault/explorer | `data_rooms`, `documents` | Local-source listing plus stored document listing exist. |
| File review, findings, risks, opportunities, key-question impact | `diligence::reviews/findings/evidence/questions` | Current values are illustrative fixtures and must not be silently persisted as facts. |
| Diligence Graph and Synthesis Canvas | `diligence` plus explicit read models | Placeholders; Helix's document graph is not automatically the product diligence graph. |
| Data Room Notes and workspace Notebook | `notebooks` | UI affordances only; no canonical note model exists. |
| Global/initiative Vault | `vaults`, `documents` | File selection/staging or fixture activity only; no durable vault aggregate. |
| Explore/Summarize | `summaries`; planned chat under `assistant` | Summary API exists. The current Chat tab is inert; planned query remains single-shot. |
| Hub “Synthesis AI” ask box | `assistant::interactions` and later `unified_search` | Presentational only; it must not bypass the shared API contract when activated. |
| Research Library, Expert Calls, Market Signals, Saved Evidence | `research`, `vaults`, `diligence::evidence` | Navigation concepts only; ownership should follow the record's purpose rather than sidebar grouping. |
| Assignments, Calendar, Operations Activity | `workflow`, `activity`, `notifications` | Navigation concepts only. |
| Integrations | `connections` plus concrete provider modules in `adapters` | SharePoint modal and dormant Rust client exist, but no completed product connection/import flow. |
| Templates | `templates` | Navigation concept only. |
| Logs | `system` diagnostics; later `activity` only for durable product events | Current log is bounded client-session diagnostics, not an audit store. |

This mapping prevents backend folders from mirroring the screen hierarchy. One screen can compose
several domains, and one domain can support several screens.

### Candidate aggregate roots

These are ownership candidates for future feature design, not a proposed schema migration:

| Aggregate root | Domain | Important references |
| --- | --- | --- |
| `User` | `users` | Stable user ID; linked to settings and memberships. |
| `UserSettings` | `user_settings` | One settings record per user, with independently evolvable preference groups. |
| `Workspace` | `workspaces` | Tenant boundary and policy scope. |
| `Membership` | `memberships` | Workspace + authenticated principal/user + role/status. |
| `Deal` | `deals` | Workspace/owner reference; source metadata should evolve into explicit source IDs. |
| `DataRoom` or `DataSource` | `data_rooms` | Deal + connection/local-source reference; must not store provider credentials. |
| `Document` | `documents` | Workspace/deal/source references; owns immutable versions and canonical bytes. |
| `DiligenceCase` | `diligence` | Usually deal-scoped; owns workstreams, questions, findings, and evidence relationships. |
| `Task` | `workflow` | Workspace/deal/diligence reference plus assignee, status, priority, and due date. |
| `Deliverable` | `deliverables` | Deal/diligence reference plus document/output version and approval state. |
| `Vault` | `vaults` | Workspace or deal scope plus references to documents/saved evidence. |
| `Notebook` | `notebooks` | User/workspace/deal scope plus ordered notes and source references. |
| `Conversation` | `assistant::conversations` | Workspace/user scope; owns ordered messages and archive state. |
| `InteractionRun` | `assistant::interactions` | One LLM execution; may reference a conversation/message but remains independently observable. |
| `Connection` | `connections` | Workspace/provider/account scope, connection state, and redacted credential reference. |
| `ActivityEvent` | `activity` | Immutable product event with actor, scope, subject reference, and redacted metadata. |
| `Notification` | `notifications` | Recipient, triggering event, channel, delivery/read state, and retry policy. |

Prefer IDs and explicit references across aggregates. Do not pass another domain's full storage
model through constructors or duplicate its canonical fields for convenience.

### Domain rules that matter most

- A UI page or navigation label is evidence of a possible domain, not proof that it deserves its
  own aggregate, table, or crate.
- A provider is not a product domain. OpenAI, WM AI, SharePoint, Helix, SQLite, and LibreOffice
  stay under `adapters/`; the consuming business capability owns the use case and product
  DTOs.
- Do not put every AI-assisted workflow under `assistant`. Deal extraction belongs to deals,
  document embeddings/indexing belong to documents, summaries belong to summaries, and
  conversation/query runs belong to assistant.
- Do not call profile lookup authentication. `users` can exist before `identity`; tenant-scoped
  authorization must eventually come from `identity`/`memberships`, not caller-supplied email or
  workspace IDs.
- Do not make cross-product unified search own canonical records. It owns search requests and
  read models/projections; source domains retain their aggregates.
- Do not split each noun into a crate. A domain earns a module through behavior and ownership, and
  earns a crate only through a stable dependency boundary and measurable benefit.

## User, settings, and workspace boundaries

These three concepts should remain distinct:

| Domain | Owns | Must not own |
| --- | --- | --- |
| `users` | Profile ID, name, email/contact fields, role display metadata, creation/update lifecycle | Login sessions, tenant authorization, OpenAI keys, theme/model/notification preferences |
| `user_settings` | Persisted personal preferences such as theme, accessibility, locale/time zone, default model policy reference, notification preferences, and UI defaults | Provider secrets, workspace-wide policy, deal configuration, auth credentials |
| `workspaces` + `memberships` | Workspace identity, membership, role/permission assignments, invitations, and tenant-scoped policy checks | Personal UI preferences or product records owned by deals/documents/etc. |

The current `users.api_key` field is development-era profile data. Do not rename it into a user
setting or imply that it configures AI. Server-side model providers continue using server-held
configuration. When real identity is added, prefer an authenticated principal injected at the
HTTP boundary over passing `userId`, email, or `workspaceId` through request bodies.

The current theme state can remain client-local until settings synchronization is deliberately
implemented. When it is implemented, the frontend theme hook should consume a `user_settings`
contract; it should not make the `users` aggregate absorb unrelated preference fields.

## LLM interactions and conversations

Model calls need two levels of ownership:

1. `assistant::interactions` owns one execution/run: validated prompt and attachments, selected
   model policy, lifecycle, streaming events, cancellation, terminal status, usage metadata, and
   sanitized failure. The planned `POST /api/v1/query_model` belongs here, even while it remains a
   stateless single-shot operation.
2. `assistant::conversations` owns durable user-facing chat: conversation ID, workspace/user
   scope, title, ordered messages, attachments/context references, created/updated timestamps,
   archive state, and the association between an assistant message and the interaction that
   produced it.

`assistant::context` is an internal submodule, not necessarily a separate service. It resolves
explicit attachments, document/search references, citations, and context-window policy for an
interaction. It must use public read facades from documents/search and must not query their tables
directly.

Suggested ownership when durable chat arrives:

```text
Conversation
  ├── Message (ordered user/assistant/system-visible product turns)
  │     └── AttachmentRef / CitationRef
  └── InteractionRun (one model execution)
        ├── requested/resolved model
        ├── started/completed/cancelled/failed timestamps
        ├── usage and finish metadata
        └── provider operation ID kept private/redacted as appropriate
```

Streaming deltas are transport events, not the canonical conversation record. Persist complete
messages and run state through an explicit durability policy; do not append every token to SQLite
by default. Cancellation must stop upstream provider work where supported. Prompt text, generated
content, attachments, and citations are sensitive product data and must not enter ordinary
transport logs.

The existing query/chat plans currently specify no conversation ID, history, resume, or durable
messages. The modularization must preserve that truth: implement the planned endpoint first under
`assistant::interactions`; add `assistant::conversations` only with a separate storage/API design.

## Current-state findings

The current horizontal layers are internally disciplined, but feature code is distributed across
many roots:

| Capability | Current locations | Main coupling/problem |
| --- | --- | --- |
| Users | `routes/users.rs`, `handlers/users/`, `services/user_service.rs`, `repository/user_repository.rs` | One small capability requires traversing four directories. |
| Deals | `routes/deal.rs`, `handlers/deal/`, `services/deal_service.rs`, `repository/deal_repository.rs`, `core/prompts/deal_extraction.rs` | Deal lifecycle and extraction belong together, but database diagnostics are incorrectly grouped into the deal route. |
| Data rooms | `routes/data_room.rs`, `handlers/data_room/`, `services/data_room_service.rs`, `core/data_room_helpers.rs` | Local-source browsing and preview are spread across delivery, service, and a 478-line generic helper file. |
| Document ingestion | `handlers/documents/process.rs`, two document services, `document_jobs.rs`, parsers, models, SQLite/Helix repository code, nodes, and insert queries | The write workflow is split across many generic locations and shares oversized files with unrelated capabilities. |
| Document viewing | `handlers/documents/stored.rs`, `stored_document_service.rs`, the SQLite half of `document_repository.rs`, parsers, and Office/PDF helpers | Catalog reads, blob loading, text extraction, and PDF rendering are one capability but have no single home. |
| Document search | `handlers/documents/search.rs`, `document_search_service.rs`, the Helix half of `document_repository.rs`, and Helix query/node modules | Search DTOs and repository behavior live under document persistence, obscuring the projection boundary. |
| Summaries | `handlers/research/summaries.rs`, part of `handlers/research/upload_support.rs`, and roughly half of `document_service.rs` | Filesystem collection and OpenAI summarization are mixed into an ingestion persistence file and grouped under `research` only at the HTTP layer. |
| External research | `handlers/research/files.rs`, `handlers/research/indexes.rs`, `research_service.rs`, and `wm_ai_services.rs` | This is a coherent WM AI capability, but it shares routes and multipart helpers with local/OpenAI summaries. |
| System/dev support | system handlers, database status, greet/login demo handlers, and `events/` | Operational endpoints and development demonstrations are mixed into product domains. |

Two files are the main seams to split before meaningful encapsulation is possible:

- `services/document_service.rs` combines document persistence/Helix graph construction with
  filesystem enumeration, OpenAI summarization, and Markdown output.
- `repository/document_repository.rs` combines the canonical SQLite file aggregate with Helix
  index writes, graph reads, and keyword/vector search.

The present `AppState` also exposes ten service handles to every handler. The architecture tests
prevent lower layers from importing it, but it is still a global feature registry rather than a
composition of independently state-bound routers.

## Logical domain map

### Currently implemented product domains and subdomains

| Module | Responsibility | Current API surface | Data/integration ownership |
| --- | --- | --- | --- |
| `users` | Development-era user/profile creation and lookup | `POST /users`, `GET /users/by-email` | Owns `users`; does not claim authentication. |
| `deals` | Deal lifecycle, deal metadata, source selection, and AI-assisted key-question extraction | Deal CRUD/archive and metadata upload routes | Owns `deals` and `deal_metadata`; uses a narrow profile lookup and OpenAI. |
| `data_rooms` | Resolve a deal's configured local source, enumerate its tree, and preview a source file | Deal data-room list/preview routes | Owns local-source access policy and tree DTOs; reads deal source metadata; uses Office conversion. |
| `documents::ingestion` | Validate uploads, parse/chunk/embed, enforce attachment identity, commit SQLite, project to Helix, and report jobs | Batch process, start job, and job-event routes | Command side of the document aggregate; writes document tables and Helix. |
| `documents::viewing` | List current stored documents, load canonical blobs, return text, and render PDF previews | Stored document list/PDF/text routes | Query side of canonical document storage; uses PDF/DOCX readers and Office conversion. |
| `documents::search` | Validate and execute keyword/vector chunk search | Document search routes | Reads the Helix projection; does not own canonical documents. |
| `summaries` | Collect supported files, summarize them with OpenAI, and write Markdown | `/summarize*` and `/summaries/markdown` | No durable database state; owns summary prompts and filesystem workflow policy. |
| `research` | WM file extraction, WM index creation/status, and WM GraphRAG queries | `/files/extract`, `/indexes*`, `/graphrag/query` | Owns the WM-facing application contract; no Quarry persistence today. |

This table describes today's backend code, while the whole-product catalog above defines where
new capabilities should land. `documents` is one bounded context because ingestion, viewing, and search share file/version
identity and one projection schema. The three user-visible capabilities remain separate
submodules with separate state and routers. This avoids inventing a fourth top-level
"document-store domain" merely so the three can share their aggregate safely.

### Supporting modules, not product domains

| Module | Responsibility |
| --- | --- |
| `system` | Health, static capabilities, and explicitly development-only database diagnostics. |
| `dev_support` | Greet and login-demo endpoints/events; easy to remove or feature-gate later. |
| `app` | Configuration, bootstrap, migrations orchestration, top-level HTTP middleware, and router composition. |
| `adapters` | Concrete SQLite, Helix, OpenAI, Office, WM AI, and dormant SharePoint adapters. |
| `shared` | Only stable primitives genuinely used by multiple domains: application error vocabulary, common validation, file-size/MIME policy, and deterministic ID/hash helpers. |

Do not turn `shared/` into a replacement for `core/`. If a type has one business owner, keep it in
that domain and export only the read operation another domain needs.

## Canonical ownership and dependency direction

```text
main
  -> app::config
  -> app::bootstrap
       -> concrete adapters
       -> domain module constructors
       -> domain routers
  -> app::http (merge routers + global middleware + /api mounts)

domain::route
  -> same-domain handler

domain::handler
  -> same-domain application service
  -> app HTTP error adapter

domain::service
  -> same-domain repository/mechanism
  -> explicitly exported read facade from another domain
  -> injected adapter capability

domain::repository
  -> concrete adapter
  -> same-domain model
```

Forbidden directions:

- no domain imports another domain's `http`, handler, repository, storage model, or private
  implementation module;
- no handler imports a repository or constructs a client;
- no service/repository reads ambient configuration or imports Axum application state;
- no adapter imports a product domain;
- no domain registers itself by mutating a global registry;
- no new code targets the `/api` compatibility prefix.

Known cross-domain dependencies should point one way:

```text
deals ----------> users::UserDirectory
data_rooms -----> deals::DataRoomSourceReader
documents ------> deals::DocumentAttachmentPolicy

user_settings --> users + identity::AuthenticatedPrincipal (when implemented)
memberships ---> identity::AuthenticatedPrincipal + users/workspaces IDs
tenant domains -> memberships::AccessPolicy (when implemented)

assistant::conversations --> assistant::interactions
assistant::context -------> documents/search public read facades
unified_search -----------> source-domain projections, never source tables

documents::viewing ----> documents private store/formats
documents::ingestion --> documents private store/formats/index
documents::search -----> documents private index

summaries -------> adapters::openai + filesystem policy
research --------> adapters::wm_ai
```

Use concrete, cloneable read facades during the first reorganization. Add traits only when a
second implementation or a test seam needs one; do not add `async-trait`, boxed futures, or a
generic repository framework solely for architectural appearance.

## Proposed source tree

```text
backend/src/
├── lib.rs
├── main.rs
├── app/
│   ├── mod.rs
│   ├── bootstrap.rs
│   ├── config.rs
│   ├── migrations.rs
│   └── http/
│       ├── mod.rs                 # merge modules, /api + /api/v1 mounts
│       ├── error.rs               # AppError -> HTTP response
│       └── middleware.rs          # request ID, tracing, gzip, timeout, CORS
├── domains/
│   ├── mod.rs
│   ├── users/
│   │   ├── mod.rs                 # narrow facade: module constructor + public read types
│   │   ├── model.rs
│   │   ├── service.rs
│   │   ├── repository.rs
│   │   ├── route.rs               # domain router and route-specific layers
│   │   ├── handler.rs             # extractors, handlers, DTOs, response mapping
│   │   └── tests.rs
│   ├── user_settings/              # future; mod.rs is an empty, unregistered scaffold
│   │   └── mod.rs
│   ├── identity/                   # future authentication principal/session boundary
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── workspaces/                 # future workspace + membership/authorization owner
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── memberships/                # future workspace authorization relationship
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── deals/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── service.rs
│   │   ├── repository.rs
│   │   ├── extraction.rs          # prompt, response parsing, OpenAI orchestration
│   │   ├── upload.rs
│   │   ├── route.rs
│   │   ├── handler.rs
│   │   └── tests.rs
│   ├── data_rooms/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── service.rs
│   │   ├── local_source.rs        # containment, tree walk, source-file reads
│   │   ├── preview.rs
│   │   ├── route.rs
│   │   ├── handler.rs
│   │   └── tests.rs
│   ├── documents/
│   │   ├── mod.rs                 # bounded-context facade; internals stay private
│   │   ├── model.rs               # Document, DocumentChunk, file/version identities
│   │   ├── policy.rs              # identity and aggregate invariants
│   │   ├── store/
│   │   │   ├── mod.rs
│   │   │   ├── sqlite.rs          # canonical aggregate reads/writes
│   │   │   └── tests.rs
│   │   ├── formats/
│   │   │   ├── mod.rs
│   │   │   ├── pdf.rs
│   │   │   ├── docx.rs
│   │   │   ├── image.rs
│   │   │   ├── spreadsheet.rs
│   │   │   └── powerpoint.rs
│   │   ├── index/
│   │   │   ├── mod.rs
│   │   │   ├── model.rs           # FileNode/FileVersionNode/FileChunkNode
│   │   │   ├── writer.rs          # insert/init projection
│   │   │   ├── reader.rs          # current/version/chunk lookups
│   │   │   ├── query.rs           # Helix query builders
│   │   │   └── tests.rs
│   │   ├── ingestion/
│   │   │   ├── mod.rs
│   │   │   ├── model.rs           # Uploaded/Processed request and response types
│   │   │   ├── service.rs
│   │   │   ├── persistence.rs     # SQLite-then-Helix orchestration
│   │   │   ├── jobs.rs
│   │   │   ├── upload.rs
│   │   │   ├── route.rs
│   │   │   ├── handler.rs
│   │   │   └── tests.rs
│   │   ├── viewing/
│   │   │   ├── mod.rs
│   │   │   ├── model.rs
│   │   │   ├── service.rs
│   │   │   ├── pdf_renderer.rs
│   │   │   ├── route.rs
│   │   │   ├── handler.rs
│   │   │   └── tests.rs
│   │   └── search/
│   │       ├── mod.rs
│   │       ├── model.rs
│   │       ├── service.rs
│   │       ├── route.rs
│   │       ├── handler.rs
│   │       └── tests.rs
│   ├── summaries/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── service.rs
│   │   ├── file_collection.rs
│   │   ├── prompt.rs
│   │   ├── upload.rs
│   │   ├── route.rs
│   │   ├── handler.rs
│   │   └── tests.rs
│   ├── assistant/                  # future; interactions/conversations/context grow here
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── research/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── service.rs
│   │   ├── upload.rs
│   │   ├── route.rs
│   │   ├── handler.rs
│   │   └── tests.rs
│   ├── diligence/                  # workstreams/questions/reviews/findings/evidence/data requests
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── workflow/                   # tasks/assignments/calendar/site visits
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── deliverables/
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── vaults/                     # workspace/deal knowledge library
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── notebooks/                  # notes/notebook ownership
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── templates/
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── unified_search/             # future cross-domain query/read model
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── connections/                # user/workspace integration configuration
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── activity/                   # future durable product activity/audit
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── notifications/
│   │   └── mod.rs                  # empty, unregistered scaffold
│   ├── system/
│   │   ├── mod.rs
│   │   ├── route.rs
│   │   ├── handler.rs
│   │   └── tests.rs
│   └── dev_support/
│       ├── mod.rs
│       ├── route.rs
│       ├── handler.rs
│       ├── events.rs
│       └── tests.rs
├── adapters/
│   ├── mod.rs
│   ├── sqlite/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   └── query/                  # current sqlbuilder files
│   ├── helix/
│   │   ├── mod.rs
│   │   └── client.rs
│   ├── openai/
│   │   ├── mod.rs
│   │   └── client.rs
│   ├── office/
│   │   ├── mod.rs
│   │   └── converter.rs
│   ├── wm_ai/
│   │   ├── mod.rs
│   │   └── client.rs
│   └── sharepoint/                 # dormant; retain isolation and current tests
│       └── ...
└── shared/
    ├── mod.rs
    ├── error.rs                    # application/repository-neutral error vocabulary
    ├── file_policy.rs              # common upload limits + supported MIME mapping
    ├── ids.rs                      # hashes and deterministic document/version IDs
    └── validation.rs
```

This is the full-product target map. Create each shown future domain directory with only a
literally empty `mod.rs` so the intended folder structure exists. Keep those scaffolds absent from
`domains/mod.rs` and all application composition; an unregistered empty file is not an executable
Rust module and must not imply feature maturity. Add implementation files only when the domain has
real behavior and a clear owner. Small implemented domains can begin with `mod.rs`, `route.rs`,
`handler.rs`, `service.rs`, and `repository.rs` and split further when warranted.

## Repeatable domain folder shape

Use the same vocabulary across implemented domains, but create only the files a domain needs:

```text
domains/<domain>/
├── mod.rs              # facade; exports constructor, router, and intentional cross-domain reads
├── model.rs            # aggregates, value objects, domain IDs, invariants
├── commands.rs         # mutating use cases when service.rs becomes crowded
├── queries.rs          # read use cases/read models when service.rs becomes crowded
├── service.rs          # small-domain application orchestration
├── repository.rs       # domain-owned persistence; may become repository/ as it grows
├── events.rs           # product events, not transport telemetry
├── route.rs            # domain router construction and route-specific middleware/layers
├── handler.rs          # extractors, handlers, transport DTO mapping, private feature state
└── tests.rs            # private domain tests
```

For a planned or conceptual domain with no backend behavior, the repeatable shape is only
`domains/<domain>/mod.rs`, with `mod.rs` left completely empty and the module omitted from
`domains/mod.rs`. Replace that scaffold with the implemented shape incrementally when the first
use case is approved; do not pre-create the remaining files.

Guidelines:

- Start shallow. Do not create `commands.rs`, `queries.rs`, `events.rs`, or a repository folder
  until real behavior needs them.
- Keep transport DTOs in `handler.rs` unless they are also genuine application inputs/outputs.
- Keep `route.rs` declarative: bind paths and methods to handlers, attach route-specific limits or
  layers, and return the domain's fully state-bound router. Business logic stays in `service.rs`.
- Keep persistence row types private to `repository.rs`; do not expose SQLite-shaped records as
  domain models.
- Keep a domain's route paths together even when URLs are nested beneath another resource, such
  as document routes beneath `/deals/{deal_id}`.
- Export cross-domain query facades from `mod.rs`; never make another domain reach through the
  facade to storage.
- Put shared provider clients, SQL mechanics, and subprocess wrappers in `adapters/`, and
  configuration parsing in `app/`, not in every domain folder.

## Feature-module composition contract

Each domain should expose one construction result, while keeping handler state and implementation
types private. Conceptually:

```rust
// domains/deals/mod.rs
pub(crate) struct DealsModule {
    router: axum::Router,
    source_reader: DataRoomSourceReader,
    attachment_policy: DocumentAttachmentPolicy,
}

pub(crate) fn build(dependencies: DealsDependencies) -> DealsModule;
```

The actual API may use methods instead of public fields, but it should have these properties:

- `bootstrap` supplies already-created adapters and explicit upstream domain facades;
- the domain constructs its repository, service, feature state, and fully state-bound router;
- handlers extract a private `DealsHttpState`, not global `AppState`;
- `app::http` merges `Router<()>` values and applies global middleware once;
- only intentional cross-domain read facades escape the module;
- tests can construct one domain module without assembling unrelated services.

Do not add a generic `Module`, `Repository`, or service-container trait. The value comes from
explicit constructors and Rust privacy, not a framework inside the framework.

## Current-to-target move map

| Current source | Target owner | Split/move notes |
| --- | --- | --- |
| `routes/users.rs`, `handlers/users/`, `user_service.rs`, `user_repository.rs` | `domains/users/` | Keep DTO casing and profile-not-authentication semantics unchanged. |
| Deal route/handlers/service/repository | `domains/deals/` | Move database status out; move the deal extraction prompt/parser into `extraction.rs`. |
| `routes/data_room.rs`, `handlers/data_room/`, `data_room_service.rs` | `domains/data_rooms/` | Depend on a narrow deal source reader, not `DealRepository`. |
| `core/data_room_helpers.rs` | `domains/data_rooms/local_source.rs`, `domains/data_rooms/preview.rs`, and shared PDF/Office mechanisms | Split path/tree policy from conversion mechanics; preserve canonical containment exactly. |
| `handlers/documents/process.rs` | `domains/documents/ingestion/handler.rs`, `route.rs`, and `upload.rs` | Keep body-limit placement, accepted fields, filename checks, and SSE behavior. |
| `document_ingestion_service.rs` | `domains/documents/ingestion/service.rs` | Keep bounded concurrency, per-attachment lock, exact-content retry, and optional OpenAI failure semantics. |
| `document_job_service.rs`, `document_jobs.rs` | `domains/documents/ingestion/jobs.rs` | Keep process-local watch channels, event names, and retention. |
| persistence half of `document_service.rs` | `documents/policy.rs` and `ingestion/persistence.rs` | Keep identity validation and SQLite-before-Helix error reporting. |
| SQLite half of `document_repository.rs` | `documents/store/sqlite.rs` | Preserve one transaction and all file/version/blob invariants. |
| Helix insert/read halves of `document_repository.rs` | `documents/index/writer.rs` and `reader.rs` | Use separate writer and reader facades over the same Helix client. |
| `core/helix_queries/files/`, `core/nodes/document_node.rs` | `documents/index/query.rs` and `model.rs` | Search result DTOs may live under `search/model.rs`; graph identity remains private to documents. |
| `core/models/document.rs`, `file_persistence.rs` | `documents/model.rs` and private store input types | Remove unused/CLI-only models only in a separate cleanup with evidence. |
| `core/parsers/` and `core/text_chunking.rs` | `documents/formats/` | Keep PDF/DOCX active ingestion paths; label image/spreadsheet/PowerPoint helpers as not wired to ingestion. |
| `handlers/documents/stored.rs`, `stored_document_service.rs` | `documents/viewing/` | Put fallback PDF rendering and cache policy under viewing; share only format readers. |
| `handlers/documents/search.rs`, `document_search_service.rs` | `documents/search/` | Move search request/response DTOs with the feature; keep caller-supplied workspace limitation explicit. |
| summary half of `document_service.rs` and summary HTTP/upload handlers | `domains/summaries/` | Remove the misleading `DocumentSummaryService`/`research` placement without changing routes. |
| WM research handlers/service and `wm_ai_services.rs` | `domains/research/` plus `adapters/wm_ai/` | Domain owns product DTOs; adapter owns HTTP/auth/wire mapping. |
| `routes/system.rs`, system handlers, `database_service.rs`, deal database handler | `domains/system/` | Keep database path endpoint explicitly development-only. |
| user demo handlers and `events/` | `domains/dev_support/` | Preserve endpoints initially; consider feature-gating/removal separately. |
| `core/clients/*` | `adapters/*` | Provider clients remain concrete adapters and are constructed only in bootstrap. |
| `core/prompts/*` | Owning domains, except generic OpenAI defaults | Deals/summaries own their prompts; CLI-only Helix prompt stays with the CLI or OpenAI tooling. |
| `utils.rs` | `shared/ids.rs`, `shared/validation.rs`, or owning document module | Split by owner; do not recreate a miscellaneous utilities file. |
| `bootstrap.rs`, `config.rs`, `state.rs`, `routes/mod.rs`, `errors.rs` | `app/` | Move migration orchestration out of bootstrap, eliminate `state.rs` after feature routers own state, and split middleware/error mapping. |
| `src/bin/*.rs` | Remain delivery entrypoints | Update them to consume domain/adapter facades; do not bury CLI behavior inside a product domain. |

## Important boundary decisions

### Document ingestion, viewing, and search

Treat these as distinct subdomains with separate application APIs, but keep the canonical file
aggregate and Helix graph schema private to the parent `documents` context.

- Ingestion may write the store and index.
- Viewing may read the canonical store and invoke format renderers.
- Search may read only the Helix projection.
- Search must not become the owner of file/version identity merely because it queries the graph.
- Viewing must not depend on ingestion service types; both depend on private document models.
- Job state belongs to ingestion, not to generic application state.

Split `DocumentIndexRepository` into capability-specific handles such as `DocumentIndexWriter`
and `DocumentSearchIndex`. Even if both wrap the same `Arc<HelixClient>`, the type system should
prevent viewing from writing the graph and search from writing canonical SQLite state.

### Deal relationships

The `deals` domain owns deal status and data-room source metadata. Other domains must not import
`DealRepository`:

- `data_rooms` receives a read-only `DataRoomSourceReader` returning only the configured local
  source needed for its use case;
- document ingestion receives a `DocumentAttachmentPolicy` or equivalent facade for active deal
  and owner checks;
- deal creation receives a profile lookup facade, not `UserRepository`.

The current document-store transaction joins `deals` and `users` to re-check existence, archive
status, and workspace ownership atomically. Preserve that guarantee during the initial move. It
may remain one explicitly named and tested cross-context integration query inside the document
SQLite adapter until a replacement offers the same transaction-time protection. Do not weaken it
to a stale pre-transaction service lookup merely to make the folder graph look pure.

### Migrations and table ownership

Keep schema version 6 and the existing recreate behavior unchanged during modularization. Move the
single migration runner to `app/migrations.rs`; do not fragment one atomic schema transaction into
domain callbacks in the first pass.

Document ownership in code even though SQLite remains shared:

- `users`: `users`;
- `deals`: `deals`, `deal_metadata`;
- `documents`: `quarry_files`, `quarry_file_versions`, `quarry_file_blobs`;
- legacy/unowned: `app_metadata`, `reminders` until an active capability claims them.

Cross-domain foreign keys are allowed. Arbitrary cross-domain queries are not. The atomic
document-attachment check above is the initial, named exception.

### Errors

Keep the existing HTTP statuses and sanitized internal-error response. During the move, retain a
small shared application error vocabulary equivalent to the current `ServiceError`, then map it
to `AppError` only in `app::http::error`.

Do not require every small domain to invent five identical error enums. A domain-specific error is
useful only when it adds domain meaning or recovery behavior; it should convert into the shared
application error at the module boundary.

### Adapters and ports

Provider clients remain injected mechanisms:

- OpenAI is used by deals, document ingestion, and summaries;
- Office conversion is used by data rooms and document viewing;
- Helix is used only by the documents context;
- WM AI is used only by research;
- SharePoint remains a dormant adapter until a product workflow owns it.

Keep provider wire DTOs private to their adapter where possible. Product request/response DTOs
belong to the consuming domain and should not be re-exported from an HTTP client by default.

## Migration sequence

Each phase should compile and pass focused tests before the next phase. Prefer move-only commits
before behavior or naming cleanup so regressions are easy to locate.

### Phase 0 — freeze observable contracts and add boundary coverage

1. Record the current route inventory under both `/api/v1` and `/api` in router tests.
2. Add characterization coverage for JSON casing, PDF headers, multipart body limits, and the four
   document-job SSE event names if any are not already explicit.
3. Add tests for the document repository's cross-domain attachment check and SQLite-before-Helix
   failure contract before moving it.
4. Extend architecture tests to define the target forbidden dependencies before old directories
   disappear.
5. Capture `cargo test --locked --all-targets` as the behavioral baseline.

Exit condition: a pure file move that changes a route, DTO, transaction, or event fails a test.

### Phase 1 — establish `app`, `adapters`, and minimal `shared`

1. Move concrete clients and SQL builder code under `adapters/` without changing behavior.
2. Split hashes/IDs, common validation, and shared file policy out of `utils.rs` and `core/mod.rs`.
3. Move configuration and HTTP error/middleware composition under `app/`.
4. Move the unchanged schema-v6 migration code into `app/migrations.rs`.
5. Keep compatibility re-exports temporarily so domain moves can be small; mark them for removal
   in Phase 7.

Exit condition: existing routes and tests pass with no schema/config/endpoint changes.

### Phase 2 — move the small vertical slices

1. Move users, system, and development-demo code into their domain folders.
2. Give each feature a private handler state and a fully state-bound router.
3. Merge these routers with the still-existing legacy router.
4. Move their focused tests beside the modules where private access is useful.

Exit condition: these features no longer import top-level `handlers`, `services`, or `repository`
modules and can be constructed independently in tests.

### Phase 3 — modularize deals and data rooms

1. Move deal models/repository/service/HTTP/upload/extraction prompt into `domains/deals/`.
2. Replace the direct `UserRepository` dependency with a narrow profile lookup facade.
3. Export a read-only deal data-room source facade.
4. Move data-room HTTP/service code and split filesystem tree policy from preview conversion.
5. Replace `DataRoomService`'s `DealRepository` import with the deal source facade.
6. Preserve the local-path/SharePoint mutual-exclusion rule and all path-containment checks.

Exit condition: deals and data rooms are separately constructible, with one explicit dependency
from data rooms to the deals facade.

### Phase 4 — split and move the documents bounded context

Perform this in smaller compiling steps:

1. Move document models, deterministic identities, chunking, and parsers under `documents/`.
2. Split `document_repository.rs` into the canonical SQLite store and Helix index reader/writer.
3. Split the persistence half of `document_service.rs` into document policy and ingestion
   persistence orchestration.
4. Move `DocumentIngestionService`, upload extraction, jobs, and process/SSE handlers into
   `documents/ingestion/`.
5. Move stored listing/blob loading, raw-text extraction, PDF rendering, conversion semaphore, and
   preview cache into `documents/viewing/`.
6. Move search DTOs, validation, Helix query builders, and handlers into `documents/search/`.
7. Introduce capability-specific repository handles so each subdomain receives only read/write
   powers it needs.
8. Keep the named atomic deal-attachment integration query and document its exception in the
   architecture test allowlist.

Exit condition: there is no generic `document_service.rs` or `document_repository.rs`; ingestion,
viewing, and search have separate constructors, state, route/handler modules, and tests.

### Phase 5 — separate summaries from external research

1. Move filesystem collection, summary DTOs, prompt construction, OpenAI orchestration, upload
   parsing, and Markdown output into `domains/summaries/`.
2. Move WM AI product operations into `domains/research/`; leave HTTP/auth mechanics in
   `adapters/wm_ai/`.
3. Split the shared research upload helper so the two domains do not import each other's handlers.
4. Keep every existing URL unchanged even though its implementation owner changes.

Exit condition: summaries and WM research can be enabled/constructed independently.

### Phase 6 — finish router composition and remove global state

1. Make every domain return a fully state-bound `Router<()>` plus any intentional read facade.
2. Have `app::bootstrap` construct modules in dependency order and pass routers to `app::http`.
3. Apply request ID, tracing, compression, timeout, CORS, and the two API mounts once, after
   merging domain routers.
4. Delete `state.rs` and update tests to construct either one domain module or the full test
   application.
5. Keep `main.rs` thin and keep all environment reads in `app::config`.

Exit condition: handlers cannot see services from unrelated domains because no global
`AppState` exists.

### Phase 7 — remove compatibility modules and harden boundaries

1. Remove temporary re-exports from old `handlers`, `services`, `repository`, `core`, `routes`,
   `events`, `document_jobs`, and `utils` paths.
2. Delete the empty legacy module trees only after `rg` finds no consumers.
3. Update architecture tests for the final paths and forbidden imports.
4. Move private unit tests beside domains; retain full-router/API tests and architecture tests as
   crate-level tests.
5. Update `docs/ARCHITECTURE.md` topology, composition, module responsibility, source map, and
   verification narrative in the same implementation change.

Exit condition: the crate compiles only through domain facades, and the canonical architecture
document matches the new tree.

### Phase 8 — place planned and future capabilities in their owners

This phase creates the folder scaffold and establishes a routing rule for subsequent feature
work; it does not implement any conceptual domain. Create every planned or conceptual top-level
domain directory shown in the target tree with a completely empty `mod.rs`. Do not add `mod`
declarations for these scaffolds to `domains/mod.rs`, and do not connect them to bootstrap, router
composition, state, adapters, persistence, tests, frontend contracts, or Tauri. Existing
extra code is outside this phase and does not need cleanup merely to make the scaffold uniform.

1. Put the planned single-shot query endpoint in `assistant::interactions`, not in a generic
   top-level `query_service`, `research`, or `core::ai` module.
2. Implement `assistant::conversations` only when conversation IDs, ordered messages, ownership,
   persistence, archive/delete policy, and resume semantics have an explicit design.
3. Implement `user_settings` when the first server-synchronized preference lands; migrate only
   that preference from client storage and keep offline/default behavior explicit.
4. Implement `identity`, `workspaces`, and `memberships` before exposing tenant-scoped production
   data; retrofit deals, documents, search, assistant, and settings to consume the authenticated
   policy facade rather than caller-supplied identifiers.
5. Put review findings, evidence, questions/answers, workstreams, risks/opportunities, and data
   requests in `diligence`; do not add these as JSON columns on deals merely because the Deal Room
   displays them.
6. Put tasks, assignments, calendars, and site-visit scheduling in `workflow`; link them to deals
   or diligence items by stable IDs.
7. Put versioned output artifacts and approval/review state in `deliverables`; document bytes may
   still be referenced from the documents context.
8. Implement `vaults`, `notebooks`, and `templates` only when their canonical records and scope
   are defined; do not promote current fixtures or staged browser selections to durable data.
9. Implement `unified_search` only when it searches more than the document projection. Feed it
   explicit source-domain projections and keep canonical writes in the source domains.
10. Implement saved connection configuration/health in `connections` while keeping SharePoint and
    other provider protocols in `adapters`.
11. Separate durable user-visible `activity` from request tracing/client diagnostics, and
    implement `notifications` only when delivery channels and user preferences have defined
    ownership.

Before replacing any empty domain scaffold with implementation, record its aggregate root, IDs,
owner/scope, commands, queries, events, storage, authorization policy, retention/deletion behavior,
and allowed dependencies before adding routes or tables.

### Phase 9 — optional crate extraction, only with evidence

After the module graph has remained stable, consider a Cargo workspace only if independent reuse,
compile-time isolation, ownership/team boundaries, or build performance justifies it. Likely first
candidates are generic adapters or the documents context—not one crate for every small feature.

Before extraction, require:

- no private cross-domain imports;
- an acyclic dependency graph;
- small reviewed public facades;
- no need to expose Axum handler internals or SQLite transaction details;
- a clear answer for where shared errors, IDs, and configuration types live;
- measured benefit beyond a deeper folder tree.

Do not extract `users`, `system`, or `dev_support` merely for symmetry.

## Test organization and enforcement

### Test placement

- Put private domain unit tests in `src/domains/<domain>/tests.rs` or a local `tests/` submodule so
  they can exercise private invariants without widening visibility.
- Keep full-router contract tests at crate level because they verify composed behavior across
  domains and both API mounts.
- Keep architecture tests at crate level.
- Preserve the crate's `autotests = false` behavior unless changing the test model is a separate,
  intentional decision. Every retained file under `backend/tests/` must still have an inclusion
  hook.

### Architecture tests to add

1. Domain sources do not import `crate::domains::<other>::repository`, `::route`, `::handler`, or
   other private modules.
2. Only `app::bootstrap` and tests construct adapters or repositories.
3. Only `app::config` reads ambient environment variables.
4. Domain route and handler modules do not import repositories or concrete adapters.
5. Adapters do not import `crate::domains`.
6. Every domain router is state-bound before top-level merge.
7. No source references the removed global `AppState` after Phase 6.
8. The old horizontal module roots have no consumers before deletion.
9. The one transaction-time documents-to-deals ownership query is narrowly allowlisted and
   behavior-tested rather than hidden as an accidental exception.

Prefer syntax-aware checks if a lightweight existing mechanism becomes available, but extend the
current source-scanning tests first rather than adding a dependency just for the migration.

## Stable contracts during implementation

- All existing paths remain mounted under both `/api/v1` and `/api`; clients continue targeting
  `/api/v1`.
- GET/POST CORS allowlisting and global middleware order remain unchanged.
- `MAX_FILE_BYTES`, aggregate request limits, and route-specific Axum body limits remain unchanged.
- Document upload accepts `userId`/`user_id` and rejects multipart `dealId`/`deal_id` exactly as
  today.
- Document job events remain `processing`, `completed`, `skipped`, and `failed`, with 15-second
  keepalive and current retention behavior.
- Stored PDF responses remain `application/pdf`, `inline`, and `private, no-store`.
- SQLite remains schema version 6 with the current destructive pre-v6 recreation behavior.
- SQLite remains canonical; ingestion commits it before Helix indexing and reports the committed
  file/version on projection failure.
- Exact-content retry, deterministic IDs after `file_id` selection, one-current-version,
  transaction rollback, and archive behavior remain unchanged.
- Optional OpenAI and WM capabilities retain current startup/invocation behavior.
- No new authentication or authorization is implied by this structural work.
- The dormant SharePoint adapter remains dormant.

If implementation reveals a desired behavior change, stop and plan it as a separate change rather
than folding it into file movement.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| A large rename diff hides behavior changes | Use compiling move-only phases; avoid formatting and cleanup in the same commit. |
| Domain folders reproduce the old layers one level deeper | Give each module a narrow facade and feature-owned router state; test forbidden cross-domain imports. |
| `shared/` becomes a dumping ground | Require two real consumers and no clear domain owner before moving a type there. |
| Splitting documents creates circular dependencies | Keep store, formats, and graph identity private to the parent `documents` context; subdomains depend inward. |
| Pure boundaries weaken transaction-time deal checks | Preserve the named cross-context SQLite query until an equally strong transactional design exists. |
| Separate crates create premature public APIs | Stay in one crate until the module graph and exported facades stabilize. |
| Router composition changes middleware/body-limit behavior | Characterize routes first and keep per-route body-limit layers inside the owning domain router. |
| Test files silently stop running | Prefer source-local test modules; verify every remaining `backend/tests/` file has an inclusion hook. |
| Empty future scaffolds are mistaken for implemented modules | Keep each `mod.rs` literally empty and omit it from `domains/mod.rs`, bootstrap, routing, state, tests, and architecture claims until real behavior lands. |
| Existing user work is overwritten | Restrict implementation to backend and required architecture docs; re-check status before every phase. |

## Verification per implementation phase

Start with the smallest affected test-name filter, then run the full backend gate from `backend/`:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Also run from the repository root:

```sh
git diff --check
git status --short
```

Do not use `cargo run` as a structural check because startup opens/migrates SQLite and requires
Helix. Do not run `clear_helix`. No frontend or Tauri gate is required for a pure backend move, but
if any TypeScript contract, desktop relay, route shape, multipart behavior, binary response, or
SSE behavior changes, run the applicable frontend and Tauri contract gates as well.

## Definition of done

- The plan's whole-product registry remains documented with honest implemented/partial/planned/
  conceptual maturity, so future features have an owner before code is added.
- A developer can locate each product capability under one domain folder.
- Users, user settings, identity/workspaces, assistant interactions/conversations, diligence,
  workflow, deliverables, vaults, unified search, connections, activity, notifications, and
  templates have explicit, non-overlapping ownership even when not yet implemented.
- Every planned or conceptual domain with no backend behavior has only an empty `mod.rs` scaffold
  and no declaration, route, state, service, persistence, test, or application integration.
- Ingestion, viewing, and search are separate document submodules with capability-specific access
  to the shared document store/projection.
- Deals and data rooms communicate through a narrow read facade, not repository imports.
- Deal creation no longer imports the profile repository.
- All feature handlers use feature-owned state; global `AppState` is gone.
- Adapters are constructed only in bootstrap and injected explicitly.
- The old horizontal module trees and temporary re-exports are removed.
- API, persistence, security, concurrency, and failure contracts are unchanged and covered.
- Architecture tests enforce the new import directions.
- All backend format/check/Clippy/test gates pass.
- `docs/ARCHITECTURE.md` is updated to describe the implemented topology and composition.
- The final diff contains no unrelated frontend edits, generated artifacts, database changes,
  secrets, or lockfile churn.

## Deliberately out of scope

- Adding anything beyond the empty, unregistered `mod.rs` scaffold for an unimplemented domain,
  including module declarations, routes, state, services, models, repositories, tests, tables, or
  integrations.
- Implementing user settings, durable LLM conversations, identity/workspaces, diligence records,
  workflow, deliverables, vault persistence, unified search, integrations, activity, or
  notifications as part of the structural refactor.
- New endpoints or API versioning.
- Authentication, tenancy, or authorization implementation.
- Schema version 7 or a new migration framework.
- A durable/distributed document job queue.
- Changing SQLite/Helix consistency or implementing reindex tooling.
- Completing SharePoint ingestion.
- Replacing OpenAI or WM AI providers.
- Frontend/Tauri reorganization.
- Converting the repository to a Cargo workspace before module boundaries prove the dependency
  graph.
