# Plan 3: template retrieval in PostgreSQL and complete Helix removal

Date: 2026-11-12. Status: proposed implementation PR, not a runtime change.
Depends on [Plan 1's shared storage/search contract](documents-postgres-pgvector-migration-plan.md#2-common-storage-and-search-decisions-for-all-three-plans)
and follows [the notebook migration](notebooks-postgres-pgvector-migration-plan.md).
This PR owns final removal of Helix only after all three domains have stopped using it.
Baseline: [template ownership](../docs/template-ownership.md) and [architecture](../docs/ARCHITECTURE.md).

## 1. What is actually being migrated

Replace `helix.templates.indexes` and all four `helix.templates.reconcile` category reads, template
publication, deletion and rebuild dependencies. No template graph mirror or reconciliation worker
is needed when canonical content and search artifacts live in the same PostgreSQL transaction.

Templates already store JSON, image assets, previews, app memberships, classification metadata,
subject/capability embedding BYTEA and lexical terms/frequencies in PostgreSQL. Public retrieval
also loads classification records and computes some cosine ranking in Rust. Helix is a recoverable
root/part/subject/capability projection; it is not the authority for private template ownership.

Therefore this migration is not a bulk copy of the whole catalog out of Helix. Replace opaque vector
blobs with typed pgvector values, move semantic top-k into bounded authorized SQL, preserve existing
SQL lexical relevance, then remove the redundant graph and its mutex/reconcile failure modes.
Keep the existing imported-slide/deck save-before-background-enrichment contract.

## 2. Storage and index design

Use the existing template parent/revision and app/owner policy. Keep the shared PostgreSQL pool,
extension installation/readiness, validated vector codec, model/profile identity, error boundaries,
transactional bulk helpers and resource/job/slot lock order specified in Plan 1. Do not repeat the
extension setup in a template service or move vector ownership into the frontend.

Proposed child `template_embeddings` has one row per `(template_id, content_revision, profile_id,
role)` with role constrained to `subject` or `capability`, preparation/input fingerprint, embedding
text/hash and typed `vector(T)`. T is the template profile dimension and may differ from documents/
notebooks. Enforce template identity/profile/FKs, unique roles, finite/nonzero vectors and valid
bounded revision. A ready active embedding set requires both roles for exactly the same current
content/classification fingerprint and profile. Use one authoritative active-set reference or
transactional readiness predicate; do not maintain conflicting active pointers in several tables.

Metadata/status rows retain classification/embedding progress separately. The embedding documents
may remain there if that is the single canonical prepared input; avoid duplicating independently
editable text/hash columns in both records. Replace BYTEA only after validated conversion/readback;
public embedding inspection remains separately gated and serializes the existing numeric shape.

Provide owner/app membership, status/fingerprint/role/profile B-tree indexes and role-specific vector
indexes as needed. Default to exact SQL cosine for small private catalogs. Use Plan 1's measured
HNSW/derived half-precision indexing/full-precision reranking policy for larger scoped catalogs.
Apply owner/app/role/current-content/profile predicates before candidate limits, with tested bounded
ANN underfill handling. Do not scan all embeddings into Rust or compare subject against capability
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

Retain this implementation and captured fixtures for the Helix exit. PostgreSQL native GIN is an
alternative candidate locator, not an equivalent fifteen-field ranker. Do not replace captured
Unicode/BM25 behavior with `english` stemming, four generic tsvector weights, fuzzy trigrams or
`ts_rank_cd` and call it parity. Plan 1's BM25-extension comparison can inform a separate intentional
relevance change, with scored fixture differences and approval of that product behavior.

Move expensive lexical aggregation into a bounded owner/app-scoped SQL query where appropriate,
preserving the precise existing corpus/scoring semantics verified by fixtures. Document whether
corpus statistics currently include rows outside the visible subset; do not silently change scores
or claim score-frequency isolation not established by that contract. Compute candidate corpus and
results in one snapshot; no singleton corpus counter or per-import shared row hotspot.

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
4. In one parent-before-job mutation transaction, recheck content revision/current classification,
   stage token/status/DB-time expiry and owner/app existence. Insert/replace both role rows and
   publish complete embedding readiness, attempt completion and sequence together. A constraint,
   index, second-role or final-fence failure rolls back the whole pair. Existing valid current
   artifacts are not overwritten by an obsolete stage. Failures cannot downgrade a successor.
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

For `similar`, authorize and load the source template/current profile without disclosing a foreign
source, then SQL-rank the permitted catalog. A `both` request ranks each role independently and
retains the existing tested combination/fusion policy; retain existing score polarity and output
shape. Hybrid combines ranks, not raw lexical and cosine values. Exact SQL parity with current Rust
cosine/combination fixtures is the baseline; approximation is an explicitly measured optimization.

Return only requested metadata/fields; avoid vector/BLOB/preview fetches before limits. Facets use
authorized current metadata and documented filters; semantic result counts do not reveal a foreign
catalog. A bounded final owner/app/revision check protects results against concurrent deletion/
revocation. Keep provider and SQL calls cancellable and their pool/admission bounds observable.

## 6. Populated data and staged rollout

First verify actual schema/version, catalog counts, classification/profile fingerprints and old vector
byte lengths. For each template, decode both BYTEA values with explicit f32 validation and insert the
current typed pair in one transaction. Compare round-tripped values and reference cosine results;
missing/malformed/mismatched artifacts remain not ready with a repair report. No zero-vector fill,
padding/truncation, silent model substitution or automatic provider spend.

A fresh development target may skip old-data import, using explicitly selected empty storage and a
new schema version. Do not assume the recent reset means data is still empty. Preserving mode drains
writers, applies validated DDL/backfill and verifies every retained record before readers switch.
Remove old BYTEA fields only when the pinned migration/schema policy and readback evidence permit it.
Old Helix content is not needed to reconstruct valid PostgreSQL template artifacts; any discrepancy
is reported, not resolved by making the graph authoritative.

Switch this domain once within the PR; no permanent Helix fallback or dual write. Reconcile an
uncertain cutover with canonical template/stage identity. Backups/restore and binary/schema/profile
compatibility determine whether rollback is supported; otherwise forward-fix. Old graph volumes
are left intact until a separately authorized cleanup.

## 7. Final Helix removal, owned by this PR

Only execute this section after documents and notebooks are SQL-only and the complete consumer
inventory is reconciled. Replacing three log messages is not sufficient.

| Existing area | Required retirement/update |
| --- | --- |
| `backend/src/domains/templates/slide_templates/{graph.rs,facade.rs,providers/worker.rs}` | Remove projection structs, startup reconcile, project/delete calls, graph-stage error handling and global mutation mutex correctness role. Retain local conversion/preparation/admission permits. |
| `backend/src/domains/templates/slide_templates/{repository.rs,repository/,retrieval/,models.rs,service.rs}` | Typed vector storage/SQL retrieval, stage fences, pair atomicity, preserved native/facade contracts and inspection gate. |
| `backend/src/app/{bootstrap.rs,config.rs}` and domain constructors | Remove Helix client creation, required URL/auth/settings and index initializers. Inject SQL repositories and optional provider services once. Database/profile validation replaces remote index startup. |
| `backend/src/adapters/helix/`, retired document/notebook graph modules | Remove only after no production caller remains; keep feature models needed for APIs under their domain, not orphaned Helix namespaces. Microsoft Graph/SharePoint code is unrelated and stays isolated. |
| `backend/Cargo.toml`, `Cargo.lock`, `backend/helix.toml` | Remove `helix-db`, unused transitive dependencies and obsolete owned Helix metadata only after reviewing live consumers. Cargo owns lockfile regeneration. |
| `backend/src/bin/{helix_query.rs,clear_helix.rs,embedding.rs}` | Retire Helix-only binaries without running them. Rework any needed embedding maintenance utility around scoped PostgreSQL profiles and atomic per-resource publication; no destructive reset utility as replacement. |
| Root `quarry` launcher and its tests | Remove mandatory Docker/Helix image/port/volume/readiness setup and HELIX_URL injection. Retain PostgreSQL service/connectivity, extension/profile readiness, API/UI supervision and cleanup behavior. Do not stop unrelated Docker workloads or delete existing volumes. |
| All server embedding configuration | Replace Helix-named document dimension/search-limit settings with domain/PostgreSQL names; map documents/notebooks to the same explicit embedding profile. Keep independent template dimensions. Coordinate deployment configuration before startup; never silently fall back from an old configured dimension to a different default. |
| API/client contracts and error tests | Remove remote-projection failure outcomes only where no longer possible; preserve `/api/v1`, camelCase, revision errors, SSE names and historical `sqliteCommitted`/deletion phase `sqlite` values. Audit other retained phase strings and document compatible SQL-only transitions. |
| Architecture/onboarding/storage/verification/project guidance | Describe PostgreSQL+pgvector as the only canonical/search store, actual keyword policy, extension provisioning, atomic document/note writes, current stage semantics and recovery. Label old Helix reports historical; do not present old graph tests as current evidence. |

A deployment without a running Helix server or Docker daemon must boot, import/search documents,
save/search notes, import/classify/retrieve templates and permanently delete resources successfully.
Missing vector extension/profile mismatch must fail readiness clearly; absent optional providers
must not prevent keyword/catalog/native read operations. No normal route opens a Helix connection.

Removing the remote store eliminates the need to build its proposed distributed generation/delete
fences. Retain SQL parent/lifecycle/attempt guards and durable jobs. The previous concurrency plan
still owns remaining replica quotas, streaming recovery and removal of global writer ownership/
advisory serialization. Do not remove those controls in this migration without its separate race/
two-process acceptance gates; do not implement a Helix-only outbox for a store being removed.

## 8. Implementation order and acceptance

Order: freeze retrieval/import fixtures; reuse Plan 1 infrastructure; add typed dual-vector SQL
storage and guarded pair backfill; push retrieval into SQL; connect stage guards and atomic dependent
invalidation; remove template graph/reconcile; run the complete cross-domain no-Helix matrix; remove
SDK/config/launcher/dead code and update documentation in the same implementation PR.

Required evidence:

- Pair failure on first/second vector, keyword/asset/preview constraints, index maintenance,
  cancellation and deferred commit error leaves the old consistent template/stage or no new pair;
  stage readiness never says ready with one missing role. Import/deck atomicity remains intact.
- Native/facade private owner/app behavior, foreign `similar` denial, facets, selected fields,
  delete CAS, app/builtin races and old Unicode/weighted BM25 captured fixture parity.
- SQL cosine and dual-role/finding/hybrid rankings match reference exact fixtures, stable ties and
  score polarity; profile/input mismatch is rejected. Selective ANN follows Plan 1's recall gate.
- Pause providers around delete/refresh, steal attempt, crash around commit/ack and reconnect:
  stale success/failure cannot publish/reset, jobs recover on another process and no graph cleanup
  or process-local mutex is required for correctness.
- Run fresh initialization, populated typed-vector conversion, rollback on malformed old vectors,
  concurrent startup, checksum/catalog/index/extension readiness and backup/restore tests.
- Run all three domains with Helix absent, fake providers and real pgvector; verify normal startup
  and launcher require no Docker/Helix. Check code/manifests/commands contain no live Helix client,
  graph startup, old vector BLOB ranking scan, remote call or automatic old-store deletion.
- Relevance, filter selectivity, p50/p95, write/rollback latency, index/storage/WAL, vacuum and
  update/delete churn benchmarks on representative template/deck sizes; no claimed measurements
  until collected. Pin extension/server/library versions in the verification report.

All backend tests/support remain manually wired under `backend/tests/`. Run focused owning suites,
then backend fmt/check/Clippy/full isolated `_test` tests with real pgvector; run launcher syntax and
its real test suite. If public mapping/client behavior changes, run both frontend typechecks,
boundaries/tests/build/bundle gates and native fmt/Clippy/tests as applicable. Use repository manifests
for exact commands and document skipped/manual/provider evidence. Never `cargo run` against valuable
data or run `clear_helix` as verification.

Review the entire consumer/dead-code diff, authorization placement, bulk rollback, dependency and
operational removal. Perform the architecture-impact/review/refactoring gates and update actual
architecture with implementation. Architecture impact of this plan: none — proposal only.
