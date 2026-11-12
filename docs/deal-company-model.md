# Deal, company, and chat model

Current storage is fresh PostgreSQL schema 1 with retained Helix. See
[PostgreSQL storage and Helix](postgresql-storage.md) for setup, tests and recovery; former
SQLite marker upgrades are historical and are not executed by this runtime.

Current storage uses PostgreSQL schema 1, including the existing nine diligence stages and all
access/ownership constraints. The replacement starts fresh without importing prior records or
converting statuses. See [storage setup and recovery](postgresql-storage.md).

PostgreSQL owns the engagement aggregate. Helix owns only a document search projection; creating or
editing a deal has no graph write. Diligence Studio's process-local template catalog is unrelated
to deal persistence.

A deal represents one diligence engagement on a transaction. Companies are reusable within the
current development user's directory. A company's name or website is never a unique identity and
never triggers implicit merging. UUID company IDs remain stable when details change. Roles and
notes belong to `deal_parties`, with uniqueness only on `(deal_id, company_id, role)`. Party ordering
is target, buyer, seller, PE sponsor, portfolio company, parent company, client, other, then company
name and ID. Unchanged company/role assignments retain their server-owned row IDs on replacement.
There are no company hierarchy, merge, delete, shared-workspace, or production identity flows.

| Dimension | Persisted codes |
| --- | --- |
| Engagement | `buy_side`, `sell_side`, `other` |
| Transaction | `platform_acquisition`, `add_on_acquisition`, `strategic_acquisition`, `carve_out`, `merger`, `divestiture`, `recapitalization`, `growth_equity`, `other`, `unknown` |
| Status | `pipeline`, `prepare`, `review_plan_discovery`, `interviews_site_visit`, `debrief_flash_report`, `follow_ups_analysis`, `qa_readout_closeout`, `closed`, `pencils_down` |
| Role | `target`, `buyer`, `seller`, `pe_sponsor`, `portfolio_company`, `parent_company`, `client`, `other` |
| Scope | `product_architecture`, `corporate_it`, `cybersecurity`, `application_security`, `sdlc`, `it_organization`, `it_financials` |

Buyers and sponsors are optional. Pipeline and Pencils Down drafts may omit all companies and dates. Working-stage or
Closed acquisitions of the three named acquisition structures require exactly one target;
other structures allow multiple targets. `startDate`, `expectedCloseDate`, `actualCloseDate`, and
`engagementCompletedDate` are nullable date-only values. Engagement completion cannot precede a
known start; transaction closing can precede diligence. Closed/Pencils Down status determines
Historic lifecycle independently of transaction dates. Lists sort by actual close, falling back
to expected close, with nulls last and deal ID as the tie-breaker. `archivedAt` hides a deal from
the default list without changing status or removing children. Direct lookup still works.

The JSON API is coordinated across Axum and both frontend adapters; there is no legacy company-
string write format. `POST /deals/{id}/update` requires complete scalar/date fields, parties, and
scopes. Null clears a nullable field; `[]` clears a collection subject to target rules; omission
of required fields is an error. Owner and deal ID are immutable. Metadata/source updates retain
their separate multipart operation. Deal create/read/update/archive responses include current
party/company and scope data in their established envelopes. Metadata has one row per deal and no
redundant owner; `keyQuestionsJson` remains a JSON-encoded string array in the wire contract.

All core deal, initial metadata, party, and scope creation writes use one `BEGIN IMMEDIATE`
transaction. Updates replace scalar and child state in one transaction, after same-owner company
validation under that lock. Any failed write rolls the entire operation back. Reads assemble
aggregates in one snapshot using bounded batches. Shared company changes become visible in every
linked deal. A company explicitly saved from the directory picker remains a directory entry if
subsequent deal creation fails or is cancelled; it was saved by a separate user operation.
Extraction still saves links before optional provider work and returns only explicitly listed
qualifying key questions, never inferred questions.

Assistant association is optional and immutable. Freeform threads store null; linked threads
reference an existing same-owner deal. List scope is All, Freeform, or a specific Deal and is
applied before cursor pagination. Runtime state is keyed by user, scope, and deal. A linked run
receives bounded current name/type/role/scope reference data through the public deals reader,
never paths, credentials, unrelated deals, or document contents. That reference is untrusted data,
not policy or retrieval evidence. Completed request replay precedes current-context/provider
resolution and returns persisted text unchanged. Archiving a deal retains its chats.

## Fresh storage and recovery

Startup creates or validates the selected PostgreSQL schema without deleting records. Use the
`Quarry` database and a matching fresh Helix store; leave old SQLite files and graph data untouched.
There is no reset binary, historical data migration, distributed transaction or general bulk
reindex command. Back up PostgreSQL and Helix with the matching binary/configuration, and drain
writers before a coordinated restore. A Helix failure preserves committed file identities and
bytes; exact-content re-upload retries the projection. See [storage operations](postgresql-storage.md).
