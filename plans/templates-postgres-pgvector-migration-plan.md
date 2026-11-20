# Plan 3: template retrieval in PostgreSQL and complete legacy storage removal

Historical design input. PostgreSQL with pgvector now owns all product persistence and search.
Use [the current architecture](../ARCHITECTURE.md) for implemented behavior; removed graph
operations and maintenance instructions below are historical, not executable rollout guidance.

Date: 2026-11-12. Updated against repository HEAD `708d73a` on 2026-11-16.
Status: proposed implementation PR, not a runtime change.
Retains design context from [Plan 1's shared storage/search contract](documents-postgres-pgvector-migration-plan.md#2-common-storage-and-search-decisions-for-all-three-plans)
and follows [the notebook migration](notebooks-postgres-pgvector-migration-plan.md).
This PR owns final removal of legacy storage only after all three domains have stopped using it.
Baseline: [template ownership](../ARCHITECTURE.md#75-template-catalog) and [architecture](../ARCHITECTURE.md).
Documents and notebooks now use PostgreSQL search and durable jobs; their implementation, not the
older migration proposals, is the execution reference. The current product schema is 4, with SQLx
0.8.6, pgvector Rust 0.4.2 and the separately provisioned pgvector extension 0.8.6 in
`quarry_extensions`. Templates are the remaining product legacy storage consumer. Recheck this inventory
before implementation; no database, retained graph store or configuration is changed by this plan.

## 1. What is actually being migrated

Replace remote template index initialization and all four reconciliation category reads, template
publication, deletion and rebuild dependencies. No template graph mirror or reconciliation worker
is needed when canonical content and search artifacts live in the same PostgreSQL transaction.

Templates already store JSON, image assets, previews, app memberships, classification metadata,
subject/capability embedding BYTEA and lexical terms/frequencies in PostgreSQL. In-process retrieval
loads classification records and computes cosine ranking in Rust; there is no public template
search endpoint. Public retrieval-inspection routes expose processing status and gated artifacts.
legacy storage is a recoverable root/part/subject/capability projection; it is not the authority for private
template ownership.

Therefore this migration is not a bulk copy of the whole catalog out of legacy storage. Replace opaque vector
blobs with typed pgvector values, move semantic top-k into bounded authorized SQL, preserve existing
SQL lexical relevance, then remove the redundant graph and its mutex/reconcile failure modes.
Keep the existing imported-slide/deck save-before-background-enrichment contract.

### 1.1 Implemented patterns to reuse

| Current implementation | Reuse in the template migration | Keep domain-owned |
| --- | --- | --- |
| `adapters/postgres/vector.rs`: `CosineVector` | Official qualified SQLx codec; validate dimensions, finite/nonzero values and the stored f32 cosine norm before publication | Template provider/model/preparation/role identity; equal dimensions do not prove compatibility |
| `adapters/postgres/client.rs` and `query/` | Existing pool, retained write transaction, repeatable-read read transaction, typed locks, mutation RETURNING, database-clock deadlines, bound `CosineDistance` and bounded `insert_batch` | Template tables, joins, scope predicates, pair readiness and query composition in template repositories |
| `domains/documents/store/search_repository.rs`: `PreparedChunks` | Prepare and validate owned artifacts before mutation; consume vectors into bounded inserts on the caller's transaction; verify affected row counts | Document chunk IDs and limits stay in documents |
| `domains/notebooks/{search_repository.rs,job_repository.rs,save_jobs.rs}` | Exact fingerprint reuse, active immutable artifact identity, final token/status/DB-clock fence after waits and batches, and atomic terminal publication | Notebook save slots, 32/256 quotas, stage strings and 120/180-second deadlines are not template defaults |
| `app/schema.sql`, `app/migrations.rs` | Composite identity FKs, deferred complete-set checks, startup integrity/index/catalog validation and the existing explicit reset utility | A template pair contains exactly subject and capability for one current classification/profile |
| `domains/templates/slide_templates/repository/contracts.rs` | Preserve asset, catalog, processing and retrieval capability interfaces backed by the same scoped repository | Extend processing claims/results and retrieval queries through their narrow interfaces; do not restore a service-locator repository |
| Template `service.rs`, `pptx/reader/`, document viewing service | Preserve the recently shared import-record preparation, focused PPTX reader and preview conversion lifecycle | This storage migration does not authorize another parser/preview/chat refactor |

Reuse these mechanisms directly. Introduce a small shared helper only when the implementation
reveals the same invariant in multiple callers and it removes duplication without importing domain
policy. Do not create a generic search engine, job framework, profile registry, cache, or alternate
backend selector. Keep a template-owned prepared-pair type and focused `search_repository.rs` if
needed rather than enlarging `repository/classification.rs` or `retrieval/query.rs` with all concerns.
Use explicit role/claim/prepared-result types for the new protocol; a cross-domain job-enum cleanup
remains outside this migration.

`shared/search_text.rs` now shares the document/notebook English/technical analyzer, including
notebook positional phrase preparation. It is not the template Unicode 6.1/BM25 contract. Reuse the
vector and SQL mechanisms without replacing template tokenization, phrase policy or ranking.

### 1.2 Recent implementation findings that change this plan

- Notebook `PreparedNote` and document `PreparedChunks` already prove the preparation/one-commit
  pattern. For templates, apply it to each publication boundary: catalog import, classification
  plus lexical publication, and the complete semantic pair. Import must still acknowledge saved
  content before background enrichment; do not make all three stages one long transaction.
- Template claims currently use caller-supplied timestamp strings. Completion checks processing
  status and input fingerprints; failure updates select by template ID/status, and `save` writes
  both stages' fields. These are not the notebook attempt-token protocol. Add independent stage
  claim tokens and database-clock deadlines, pass the owned claim through success/failure/retry,
  and replace stale whole-row updates with stage-specific conditional mutations before removing
  the projection mutex. Fingerprints cannot distinguish two attempts for identical input.
- Notebook search fuses at most 100 candidates per branch. That is its documented contract, not
  evidence of exact template parity. Template dual-role/finding modes currently rank complete
  compatible role lists; section 5 must preserve that behavior or report an intentional change.
- Document and notebook profile identities are currently domain-specific. Share numeric encoding
  and validation, but retain independent fingerprints and template dimensions. No profile-table
  consolidation or re-embedding of document/notebook data is required.

## 2. Storage and index design

Use the existing template parent/revision and app/owner policy. Keep the shared PostgreSQL pool,
extension installation/readiness, validated vector codec, model/profile identity, error boundaries,
and transactional bulk helpers already implemented. Use template parent-before-stage lock order;
notebook admission slots are not a prerequisite. Do not repeat extension setup in a template service or move vector ownership into the frontend.

Proposed child `template_embeddings` has one row per `(template_id, content_revision, profile_id,
role)` with role constrained to `subject` or `capability`, preparation/input fingerprint, embedding
text/hash and typed `vector(T)`. T is the template profile dimension and may differ from documents/
notebooks. Enforce template identity/profile/FKs, unique roles, finite/nonzero vectors and valid
bounded revision. A ready active embedding set requires both roles for exactly the same current
content/classification fingerprint and profile. Use one authoritative active-set reference or
transactional readiness predicate; do not maintain conflicting active pointers in several tables.
Define whether the existing public `templates.revision` is also the content generation or a separate
generation is needed. Classification/embedding progress must not invalidate a client's displayed
delete revision merely because enrichment finished. App attachment and metadata-only operations
must not re-embed unchanged input. Enforce the chosen content/profile relationship with composite
keys/FKs or equivalent transactional checks, not unrelated valid IDs alone.

Make pair completeness enforceable at commit, following notebook complete-set publication: a
ready set has exactly the two allowed roles, matching dimensions/profile and current input hashes.
If rows are stored separately, use deferred validation so a missing second role or a changed child
cannot leave readiness true; include that integrity check in startup validation. Use a single
prepared pair and one transaction, not two independent successful role writes. Bind template T
separately in DDL/checksum generation: the current migration renderer replaces `vector(1536)` for
documents/notebooks and must not accidentally resize template columns to their shared dimension.
Test intentionally different document/notebook and template dimensions.

Metadata/status rows retain classification/embedding progress separately. The embedding documents
may remain there if that is the single canonical prepared input; avoid duplicating independently
editable text/hash columns in both records. Replace BYTEA storage with the typed pair; public
embedding inspection remains separately gated and serializes the existing numeric shape. Preserving
existing values requires the explicitly requested conversion/readback path in section 6.
Under the default MVP recreation rollout in section 6 there is no legacy conversion path; omit its
BYTEA decoder instead of carrying an unused compatibility layer.

Provide owner/app membership and status/fingerprint/role/profile B-tree indexes supported by actual
queries. Use exact SQL cosine as the initial implementation, matching documents and notebooks.
HNSW, half-precision projections and alternative rankers are deferred optimizations requiring a
separate measured relevance/load decision; no ANN index or dependency is required for this cutover.
Apply owner/app/role/current-content/profile predicates before candidate limits. Do not scan all embeddings into Rust or compare subject against capability
unless the request mode explicitly combines separately ranked branches.

Template JSON/assets/preview replacement increments content revision and invalidates classification,
lexical data and embedding readiness atomically. Old artifacts may be retained for internal recovery
only if never selected as current; there is no public previous-template-revision retrieval requirement.
Builtin replacement uses canonical checksum/version guards and must invalidate dependent artifacts
when its embedding input changes. No provider or preview rendering occurs inside that transaction.

## 3. Keyword retrieval: keep the working inverted index

The existing `template_lexical_terms` posting table is already an inverted term-to-template index.
`slide_classifications_fts` stores fifteen lexical fields and lengths; the query implements weighted
FTS5-compatible BM25 with Unicode 6.1 normalization, prefix behavior, app/owner filtering and stable
ties. The statistics singleton has already been removed in favor of exact snapshot aggregates.

Retain this implementation and captured fixtures for the legacy storage exit. PostgreSQL native GIN is an
alternative candidate locator, not an equivalent fifteen-field ranker. Do not replace captured
Unicode/BM25 behavior with `english` stemming, four generic tsvector weights, fuzzy trigrams or
`ts_rank_cd` and call it parity. Plan 1's BM25-extension comparison can inform a separate intentional
relevance change, with scored fixture differences and approval of that product behavior.

The current `adapters/postgres/query/lexical.rs` deliberately computes document frequency, total
corpus size and average length over the complete lexical corpus, then filters results by owner/app.
Preserve those scoring semantics and captured fixtures unless a separately reviewed relevance/privacy
change is accepted. Scope candidate authorization before result limits without silently redefining
the scoring corpus; do not claim score-frequency isolation from owner-filtered results. Compute
corpus and results in one snapshot; no singleton corpus counter or per-import shared row hotspot.

Use indexed term equality/prefix ranges and relational filter indexes, and inspect real query plans
for broad queries/large catalogs. Any normalized facet JSON must have measured containment/index
support or explicit bounded joins; do not create a GIN index for every JSON field speculatively.
A no-match query returns no matches, never an unrelated catalog sample.

## 4. Atomic import, enrichment and update protocols

### 4.1 Import and deck save

Keep preparation, native conversion, preview rendering, validation and cancellation checks outside
SQL. The existing import transaction commits JSON/metadata/assets/preview/app attachment and pending
classification together. Preserve classified single-slide/deck acknowledgment before providers run;
retain existing all-or-nothing deck persistence and cancellation rollback for catalog creation.
Import preparation failure must not leave an orphan asset/preview or a partially saved deck.

No import response waits for a graph projection. The public template revision envelope, previews,
owner/app scope and separate classification/embedding status remain unchanged.

### 4.2 Classification and dual embeddings

1. Durably claim one eligible stage with token/status/DB-time expiry. Job-only claims do not hold
   the template parent over provider time. Capture content revision, prompt/schema/model/profile
   and exact input fingerprint. Preserve bounded provider concurrency, timeout and manual retry.
2. Classification provider/normalization runs outside SQL. Under the current parent/attempt guard,
   commit normalized metadata, lexical fields/postings, embedding input documents/fingerprints and
   classification success while atomically invalidating obsolete vectors and queuing embedding.
   Keyword readiness may precede semantic readiness as it does now.
3. Obtain both subject and capability vectors outside SQL. Validate both after encoding, including
   required role, dimensions, finite/nonzero norms and input/profile fingerprints. One failed
   embedding does not commit the other role or mark the pair ready.
4. In one parent-before-stage mutation transaction, recheck content revision/current classification,
   stage token/status/DB-time expiry and owner/app existence. Insert/replace both role rows and
   publish complete embedding readiness, attempt completion and sequence together. A constraint,
   index, second-role or final-fence failure rolls back the whole pair. Existing valid current
   artifacts are not overwritten by an obsolete stage. Failures cannot downgrade a successor.
   Repeat the conditional token/status/DB-clock fence after row waits and pair insertion, checking
   affected-row counts, as notebook `persist` does. Failure recording must match that same claim;
   an old timeout cannot fail a reclaimed successor with identical input.
5. On an uncertain commit, reconcile the durable stage identity before replaying a provider.
   A prepared result may be reused only under exact profile/input/content fingerprints. There is
   no follow-up remote publish checkpoint; SQL commit is semantic availability.

Use real SQL tests for simultaneous refresh/delete/classification/embedding and cancellation. Existing
parent locks alone do not replace attempt tokens; a provider can return after its lease expires.
Metadata/lexical and embedding attempts remain independently guarded so stale whole-row saves cannot
reset the other stage. Retain separate retry outcomes and do not leak vectors when inspection is off.

### 4.3 Delete, app attachment and reconciliation

Revisioned authorized deletion removes app membership or canonical template/artifacts according to
existing shared-builtin/private rules in one transaction. A stale expected revision changes nothing.
After the canonical row disappears, a late stage fails its FK/revision/attempt guard; no remote
projection tombstone or graph cleanup step remains.

Close app-registration versus builtin-attachment races with explicit per-app coordination or durable
idempotent attachment intent; preserve app-specific removed choices. Do not reattach every builtin
on each registration, introduce a global catalog lock or reintroduce the retired startup seeder.
Keep omitted registration fields field-scoped. Batch writes use deterministic parent order.

Replace startup reconciliation with bounded SQL integrity/index/profile validation. Do not scan and
re-embed the entire catalog on every startup. Explicit repair/backfill is separate durable work
with per-template atomic publication, not a silent GET mutation or startup-wide transaction.

## 5. SQL semantic and hybrid retrieval

Push the existing modes `text`, `semantic`, `hybrid`, `filter`, `similar`, `by_id` and `facets` through
bounded owner/app-scoped repository queries. Preserve modes, filter names, limits, output selection,
semantic target `subject`/`capability`/`both`, stable ties and finding retrieval behavior. Query
embedding calls happen before the SQL snapshot; missing providers fail only invoked semantic work.
Keyword/filter/catalog operations remain usable without an embedding provider.

Freeze the actual mode-specific behavior in `retrieval/query.rs` before replacing it. Hybrid currently
returns lexical results with a `semantic_unavailable` warning when semantic ranking is unavailable;
other semantic failures propagate. Semantic with no compatible records returns an empty list before
calling a provider. `similar` ranks stored source vectors and needs no query embedding provider;
source lookup is owner/app-scoped independently of destination metadata filters, and excludes the
source from results. Preserve these cases explicitly rather than imposing one provider rule on all
modes. This migration does not add a public search endpoint or broaden native routes.

For `similar`, authorize and load the source template/current profile without disclosing a foreign
source, then SQL-rank the permitted catalog. A `both` request ranks each role independently and
retains the existing tested combination/fusion policy; retain existing score polarity and output
shape. Hybrid combines ranks, not raw lexical and cosine values. Exact SQL parity with current Rust
cosine/combination fixtures is the baseline; approximation is an explicitly measured optimization.

Do not truncate each vector branch to the requested output limit and claim exact fusion parity.
Current semantic `both` and `similar` use reciprocal-rank fusion with constant 60 over complete
compatible role rankings; hybrid fuses lexical and semantic ranks. Finding retrieval adds lexical,
subject and capability reciprocal ranks plus metadata boosts (`exact_finding_boost * 0.005`), and
derives topic matches from the scoped catalog. Preserve those boosts and authorized topic discovery
without hydrating every vector/metadata record into Rust. Exact SQL may rank the scoped corpus within
a deadline while returning bounded rows; an approximate candidate budget needs explicit relevance
evidence, including winners below individual branch cutoffs. Capture the current lexical 200-hit
candidate cap separately from the final output limit. Filter-before-limit fixes must have explicit
regression expectations rather than being hidden as parity. Test recency/ID ties, including non-ASCII
IDs: Rust uses English ICU collation for vector/fusion ties, while lexical SQL uses `COLLATE "C"`.
Set numeric comparison tolerances for SQL versus reference cosine scores before acceptance; storage
precision and near-tie behavior must be tested, not hidden behind a bit-for-bit float equality claim.

Keep feature SQL in the template repository. Existing `CosineDistance` handles a bound query
vector, but does not by itself express complete multi-role SQL ranking/fusion, finding boosts or
stored-vector `similar` joins. Add only the reusable typed query expressions actually required
(e.g. column-to-column distance and window/rank composition), with query-builder tests; keep feature
names, weights and scope policy out of `adapters/postgres/query/`. Do not bypass the typed boundary
with service SQL or return a full vector catalog to Rust because the builder needs an extension.

Use one authorized repeatable-read snapshot for candidate metadata, lexical corpus statistics,
role rankings and fusion; query embeddings precede it. Recheck bounded selected identities and
current owner/app/content/profile state in a fresh read before delivery. Follow notebook final-set
checks and document final authorization, and revalidate sessions at the HTTP boundary for any
newly exposed long-running retrieval response. Ordinary read transactions remain independent of
writer-lease health and use no mutation/advisory/row locks. Retain the production global write lease
and mutation serialization; independent-pool tests do not authorize multi-instance deployment.

Return only requested metadata/fields; avoid vector/BLOB/preview fetches before limits. Facets use
authorized current metadata and documented filters; semantic result counts do not reveal a foreign
catalog. A bounded final owner/app/revision check protects results against concurrent deletion/
revocation. Keep provider and SQL calls cancellable and their pool/admission bounds observable.

## 6. MVP schema rollout and recovery

The repository's 2026-11-15 MVP policy is the default: affected product tables may be recreated and
existing MVP data discarded for this schema change. Do not build a preserving upgrade/backfill tool
unless the user explicitly requests data preservation. This plan update authorizes no reset now.
During implementation, state the selected reset scope and recovery procedure before executing it.

Current `app/migrations.rs` initializes an empty selected schema or validates schema 4 with exact
source checksum/catalog/index/extension checks. Choose the next schema version against the live
implementation (expected 5 if no intervening change), update the source and tests together, and
retain fail-intact startup. Neither startup nor failed validation may drop data, rewrite the marker,
backfill, re-embed, or retry with a reset.

Use a fresh explicitly selected target or the existing guarded `reset_postgres --confirm-reset`
procedure for a full product-schema recreation. That operation discards recognized SQL product
objects, including documents, notebooks, users, deals and templates; it is not a template-only reset.
If choosing affected-table-only recreation, define and test the dependency closure and checksum/
catalog transition explicitly instead of inventing a partial mode for the full-reset utility.
Keep unrelated databases, `quarry_extensions`, SQLite files and retained legacy stores intact.

Drain the old writer/workers and take matching backups before a destructive rollout. Do not infer
emptiness from an earlier reset. Validate fresh initialization/restart, both vector dimension
families, deferred pair integrity and document/notebook invariants after recreation. No live provider
spend is needed to verify schema installation. Repopulation and any reclassification/re-embedding
must be explicitly scoped; missing pairs remain not ready.

Only if preservation is explicitly requested: inventory actual records and exact profile/input
identities, decode legacy little-endian f32 BYTEA values, validate/read back both roles atomically,
and report malformed/missing pairs without padding, truncation or silent model substitution. That
scope requires an explicit source-version/checksum-validated upgrade procedure and restore rehearsal;
ordinary startup still does not perform an upgrade. Do not implement this optional path by default.

Switch templates to SQL once, with no permanent legacy storage fallback or dual write. Reconcile uncertain
commits through canonical template/stage identity. Rollback needs the old binary plus matching SQL/
legacy storage backup; otherwise forward-fix after drain. Old graph discrepancies are not resolved by treating
legacy storage as authoritative. Volume/container/index deletion is a separately authorized retirement task.

## 7. Final legacy storage removal, owned by this PR

Only execute this section after documents and notebooks are SQL-only and the complete consumer
inventory is reconciled. Replacing three log messages is not sufficient.

| Existing area | Required retirement/update |
| --- | --- |
| `backend/src/domains/templates/slide_templates/{graph.rs,facade.rs,providers/worker.rs}` | Remove projection structs, startup reconcile, project/delete calls, graph-stage error handling and global mutation mutex correctness role. Retain local conversion/preparation/admission permits. |
| `backend/src/domains/templates/slide_templates/{repository.rs,repository/,retrieval/,models.rs,service.rs}` | Typed vector storage/SQL retrieval, stage fences, pair atomicity, preserved native/facade contracts and inspection gate. |
| `backend/src/app/{bootstrap.rs,config.rs}` and domain constructors | Remove legacy storage client creation, required URL/auth/settings and index initializers. Inject SQL repositories and optional provider services once. Database/profile validation replaces remote index startup. |
| the removed graph adapter and remaining test support | Documents/notebooks already have no production graph modules; verify all remaining callers before removing the adapter. Keep feature models needed for APIs under their domain, not orphaned legacy storage namespaces. Microsoft Graph/SharePoint code is unrelated and stays isolated. |
| `backend/Cargo.toml`, `Cargo.lock`, the removed graph runtime metadata | Remove the graph SDK, unused transitive dependencies and obsolete owned legacy storage metadata only after reviewing live consumers. Cargo owns lockfile regeneration. |
| the retired graph maintenance utilities | Retire legacy-store binaries without running them. Rework any needed embedding maintenance utility around scoped PostgreSQL profiles and atomic per-resource publication; no destructive reset utility as replacement. |
| Root `quarry` launcher and its tests | Remove mandatory Docker/legacy storage image/port/volume/readiness setup and previous graph URL setting injection. Retain PostgreSQL service/connectivity, extension/profile readiness, API/UI supervision and cleanup behavior. Do not stop unrelated Docker workloads or delete existing volumes. |
| All server embedding configuration | Replace legacy storage-named document dimension/search-limit settings with domain/PostgreSQL names; preserve the shared document/notebook configured dimension and their distinct existing profile identities. Keep independent template dimensions. Coordinate deployment configuration before startup; never silently fall back from an old configured dimension to a different default. |
| API/client contracts and error tests | Remove remote-projection failure outcomes only where no longer possible; preserve `/api/v1`, camelCase, revision errors, SSE names and historical `sqliteCommitted`/deletion phase `sqlite` values. Audit other retained phase strings and document compatible SQL-only transitions. |
| Architecture/onboarding/storage/verification/project guidance | Describe PostgreSQL+pgvector as the only canonical/search store, actual keyword policy, extension provisioning, atomic document/note writes, current stage semantics and recovery. Label old legacy storage reports historical; do not present old graph tests as current evidence. |

### 7.1 Mandatory template code and query cleanup

Remove newly unused template code in the same implementation PR as the SQL cutover; do not leave
a disabled legacy storage path, optional constructor switch, compatibility shim or `allow(dead_code)` behind.
The current queries are Rust SDK DSL builders in `slide_templates/graph.rs`, not standalone `.hx`
query files. Inventory actual definitions and callers again at implementation time.

- Delete the template graph module and its `mod.rs` export after replacing its callers. This includes
  `TemplateGraph`, `TemplateProjectionRecord`, `TemplatePreviewDescriptor`, `prepare_record`,
  `scoped`, `bounded`, projection-only serialization/fragment hashing and size limits, and labels
  `SlideTemplate`, `SlideTemplatePart`, `SlideTemplateSubject`, `SlideTemplateCapability`.
  Retire index creation/status, publication, deletion and all four reconciliation label reads. No replacement graph query or graph reconciliation worker is needed.
- Remove `ImportService::projection_record` in `service.rs`; facade `projection`, `project`,
  `reconcile`, `projection_failure` and the impossible `template_projection_failed` outcome; and
  worker projection publication, its projection-only `import` option and mutex plumbing. Update
  constructors, bootstrap index/reconcile calls and every test fixture that supplies those arguments.
  Preserve the shared engine, worker lifecycle, preview/conversion/admission bounds and SQL guards.
- Audit `retrieval/vector.rs`, `retrieval/query.rs`, repository classification mapping and exports.
  Remove obsolete normal-runtime BYTEA encoding/decoding and full-catalog Rust vector ranking when
  SQL owns them. Keep a legacy decoder only in an explicitly owned migration path if preserving
  backfill still uses it. Put reference cosine/fusion helpers needed only for parity in the manually
  wired `backend/tests/` tree; retain any production helper with a verified live caller. Keep provider
  vector validation, public numeric inspection serialization and classification input preparation.
- Remove projection-only `graph_tests.rs` and its module hook after moving useful pair-atomicity,
  failure and authorization assertions to SQL tests. Replace graph-failure expectations in
  `integration_tests.rs` and `user_scope_tests.rs`; preserve denial/no-resurrection coverage.
  Review architecture checks that name `TemplateGraph::new`, shared mock legacy storage support, bootstrap
  fixtures and ignored live graph tests. Update guards to reject reintroduced dependencies rather
  than retaining obsolete string checks or deleting unrelated security tests.
- With documents/notebooks already SQL-only, complete the consumer audit and retire shared legacy storage SDK/client/config/binaries,
  launcher setup, unused dependencies/imports, test modules and exports. Preserve all SQL document/
  notebook code throughout this cleanup. Never delete SQL lexical postings, native conversion, Microsoft Graph code,
  old data stores or Docker volumes as part of template dead-code cleanup.

Acceptance requires a before/after symbol and caller inventory, no template legacy storage operations or
projection-only types/callers in live source/tests, no newly unused exports/dependencies, and passing
all-target compilation/Clippy. Add an architecture regression guard for the retired template dependency.
Use `rg` over maintained source/tests/manifests, excluding generated/local outputs; explain intentional
historical documentation matches. Tests must run the SQL template path with legacy storage absent, not merely
mock or disable the old projection. Global legacy storage removal additionally requires the full three-domain
SQL-only acceptance matrix. Do not execute query binaries or drop remote indexes to prove removal.

A deployment without a running legacy storage server or Docker daemon must boot, import/search documents,
save/search notes, import/classify/retrieve templates and permanently delete resources successfully.
Missing vector extension/profile mismatch must fail readiness clearly; absent optional providers
must not prevent keyword/catalog/native read operations. No normal route opens a legacy storage connection.

Removing the remote store eliminates the need to build its proposed distributed generation/delete
fences. Retain SQL parent/lifecycle/attempt guards and durable jobs. The previous concurrency plan
still owns remaining replica quotas, streaming recovery and removal of global writer ownership/
advisory serialization. Do not remove those controls in this migration without its separate race/
two-process acceptance gates; do not implement a legacy-store outbox for a store being removed.

## 8. Implementation order and acceptance

Implement in reviewable steps, with each compiled/tested state preserving current behavior:

1. Freeze retrieval/import/inspection/provider-unavailable fixtures and inventory remaining legacy storage
   callers. Resolve content-generation and pair-readiness design against existing revision behavior.
2. Reuse the current PostgreSQL infrastructure; add template dimension rendering, typed complete-pair
   schema/preparation and stage-specific token/deadline claims/results/failure mutations. Cover stale
   identical-input attempts before removing any local serialization.
3. Add scoped SQL retrieval through the existing retrieval capability, with only necessary generic
   query-builder additions. Prove exact lexical/vector/fusion/mode parity and bounded result mapping.
4. Wire atomic classification/lexical and pair publication; remove template graph/reconcile and its
   mutex/callers. Test catalog acknowledgment independently from enrichment completion.
5. Remove the remaining legacy storage SDK/config/launcher/binary/test dependencies and update live docs and
   guidance. Verify cross-domain behavior with legacy storage and Docker absent using disposable fixtures.
6. Rehearse the explicit fresh/MVP recreation rollout and fail-intact restart behavior. Record the
   chosen schema version, reset scope, backup/restore limits and deferred performance work.

No preserving backfill, ANN work, cross-domain job redesign or global write-guard removal is part
of the default sequence.

Required evidence:

- Pair failure on first/second vector, keyword/asset/preview constraints, index maintenance,
  cancellation and deferred commit error leaves the old consistent template/stage or no new pair;
  stage readiness never says ready with one missing role. Import/deck atomicity remains intact.
- Native/facade private owner/app behavior, foreign `similar` denial, facets, selected fields,
  delete CAS, app/builtin races and old Unicode/weighted BM25 captured fixture parity.
- SQL cosine and dual-role/finding/hybrid rankings match reference exact fixtures, stable ties and
  score polarity, including fusion candidates below individual branch limits and finding boosts;
  mode-specific unavailable-provider warnings/errors and provider-free `similar` are preserved.
  Profile/input mismatch is rejected. ANN remains deferred pending separate recall evidence.
- Pause providers around delete/refresh, steal attempt, crash around commit/ack and reconnect:
  stale success/failure cannot publish/reset, expired jobs recover after a drained process restart,
  and no graph cleanup or process-local mutex is required for correctness.
- Run fresh initialization/MVP recreation, refused incompatible populated startup, atomic DDL
  rollback, concurrent startup, checksum/catalog/index/extension readiness and backup/restore tests.
  Legacy typed-vector conversion tests apply only to an explicitly requested preservation scope.
- Run all three domains with legacy storage absent, fake providers and real pgvector; verify normal startup
  and launcher require no Docker/legacy storage. Check code/manifests/commands contain no live legacy storage client,
  graph startup, old vector BLOB ranking scan, remote call or automatic old-store deletion.
- Complete section 7.1's template symbol/query/caller inventory and retire obsolete tests/support;
  record remaining shared consumers before removing any cross-domain legacy storage infrastructure.
- Relevance, filter selectivity, p50/p95, write/rollback latency, index/storage/WAL, vacuum and
  update/delete churn benchmarks on representative template/deck sizes; no claimed measurements
  until collected. Pin extension/server/library versions in the verification report.

All backend tests/support remain manually wired under `backend/tests/`. Run focused owning suites,
then run from `backend/`:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings -D clippy::redundant_clone
QUARRY_TEST_DATABASE_URL=postgresql://localhost/Quarry_test cargo test --locked --all-targets
```

Inspect formatting changes without overwriting unrelated work. Provision extension 0.8.6 once using
`backend/tools/provision-pgvector.sql` against each selected database; tests never
install/drop it. Reuse notebook rollback/fence/snapshot/independent-pool fixtures and document
codec/read-independence tests, keeping all added test helpers manually wired under `backend/tests/`.
Run launcher syntax (`bash -n quarry`) and its suite (`python3 tests/test_quarry_launcher.py`) from
the repository root. Update their existing dependency stubs instead of starting real Docker.

If public mapping/client behavior changes, run both frontend typechecks,
boundaries/tests/build/bundle gates and native fmt/Clippy/tests as applicable. Use repository manifests
for exact commands and document skipped/manual/provider evidence. Never `cargo run` against valuable
data or run the retired graph maintenance utilities as verification.

Review the entire consumer/dead-code diff, authorization placement, bulk rollback, dependency and
operational removal. Perform the architecture-impact/review/refactoring gates and update actual
architecture with implementation.

Architecture impact of this plan: none — proposal only; the schema-4 runtime and existing contracts
remain unchanged. Implementation must update `ARCHITECTURE.md`, storage/recovery,
configuration, launcher instructions and `AGENTS.md` when legacy storage is actually retired.

Plan-update verification (2026-11-16): inspected current manifests/lockfile versions, schema and
migration policy, document/notebook preparation and publication, template repository capabilities,
classification claims/completion/failure, retrieval modes and remaining legacy storage configuration/launcher
consumers. Runtime gates above are implementation acceptance commands, not results of this plan edit.
