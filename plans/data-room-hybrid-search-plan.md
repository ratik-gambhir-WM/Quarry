# Data Room hybrid keyword and semantic search plan

## Status, decision, and scope

Refreshed 2026-10-15 against the live working tree, including uncommitted
deal/company, schema-9, and deal-linked Assistant changes. Hybrid search,
advanced criteria, deal-scoped Helix indexes, and backfill/rebuild tooling are
still proposed; none is marked complete by this refresh.

This is a proposed implementation plan, not the current product contract. The
live code and [architecture](../docs/ARCHITECTURE.md) remain authoritative until
the implementation is complete.

Build one server-side hybrid document search for the selected Data Room. It
will retrieve both keyword/BM25 and semantic/vector candidates from the
existing Helix projection, fuse them with reciprocal-rank fusion (RRF), and
return a document result with a truthful match label, excerpt, and location.
The Data Room UI will replace its fixture-backed selected-document filter with
this cross-document search.

The initial UI also includes an Advanced Search disclosure modeled on the
requested controls: **All these words**, **Any of these words**, **None of
these words**, and **This exact phrase**. These are structured criteria with
defined, testable semantics; they are not a browser-composed Boolean or Helix
query string. The initial scope remains the selected Data Room. An **Entire
Project** scope is a later product/API expansion, not an alias for the current
owner-wide workspace search.

The initial release searches the selected deal's uploaded, persisted, and
successfully indexed current document versions. It does not claim to search
every row visible in the Data Room: the current UI also lists local filesystem
files, while the SharePoint modal is not an implemented import. Neither source
has entered SQLite or Helix. They must not be searched directly from a browser
or silently shown as indexed.

Keep SQLite canonical and Helix as the recoverable projection. Do not add a
second vector store, OpenAI Vector Store, Elasticsearch, or client-generated
embeddings.

## Current-state findings

| Area | Current implementation | Gap to close |
| --- | --- | --- |
| Ingestion | PDF, DOCX, XLSX, PPTX, and image parsers create cl100k_base chunks of up to 800 tokens, embed them with the configured text-embedding-3 model, then commit SQLite before Helix. | Chunks, embeddings, sections, page ranges, and character ranges already exist; no replacement parser is needed. |
| Helix | FileChunk has workspace-partitioned cosine vector and BM25 text indexes. QuarryFile has a display-name text index. | The signals are exposed separately rather than as a Data Room search result. |
| API | Keyword search accepts workspace ID, text, and limit. Vector search accepts a client-supplied embedding. | These are low-level calls, not a deal-scoped, authorization-ready product contract. |
| UI | QuarryApi, HTTP, and Tauri adapters expose the low-level endpoints. DataRoomDocumentSearch filters deterministic fixture excerpts for the current PDF. | There is no genuine semantic search, cross-document result, or search before opening a document. |
| Scope | SQLite quarry_files records both deal_id and workspace_id. Helix nodes carry only workspace_id. | A user can own multiple deals, so an owner-scoped top-K cannot accurately retrieve the selected deal. |
| Deal model | Schema 9 has typed engagement/transaction/status fields, reusable owner-scoped companies, deal parties/scopes, and archive timestamps. | Use stable deal ID and canonical user ownership, never deal name, company name/role, status, or portfolio Current/Historic grouping as a search partition. |
| Assistant | Synthesis Canvas uses persisted deal-linked threads; DealContextReader supplies bounded deal reference data. | Thread association is not document retrieval or authenticated access; preserve it while changing search. |
| Operations | Schema 9 is initialized only for an empty database; compatible layouts are reused and incompatible layouts fail safely. There is no general Helix backfill/rebuild tool. | Add bounded ownership backfill and prove index population; neither reset_development nor clear_helix is a search migration. |

Two facts make a frontend-only implementation incorrect:

1. The existing searches query FileChunk nodes directly; they do not prove that a
   candidate is reached through the published CURRENT_VERSION edge.
2. The current partition is the normalized owner email, not the deal. Filtering
   a limited owner-wide result set in Axum can lose good in-deal matches and
   hides an important authorization boundary.

### Recent changes and implementation anchors

- Search is now reached through the explorer's four-action toolbar in
  [DataRoomQuickActions](../frontend/src/components/data-room/DataRoomQuickActions.tsx).
  [DataRoomWorkspace](../frontend/src/components/data-room/DataRoomWorkspace.tsx)
  supplies the panel boundary and current preview page/focus callbacks. The
  explorer mounts only with a selected preview; the unselected view keeps the
  Deal Room sidebar and file-review table. A search trigger in the unselected
  view remains new work.
- The fixture adapter is
  [document-search/DataRoomDocumentSearch.tsx](../frontend/src/components/data-room/document-search/DataRoomDocumentSearch.tsx),
  with records under [fixtures/data-room/documentSearch.ts](../frontend/src/fixtures/data-room/documentSearch.ts).
  Its filename-based availability check and page-only target must become stable
  file/version identities. The file-review table's local filter is a separate
  control and must not be presented as semantic document search.
- [SQLite document persistence](../backend/src/domains/documents/store/sqlite.rs)
  now resolves ownership through `deals.user_id -> users.email` and rejects
  ingestion into an archived deal. Do not recreate the removed duplicated
  metadata owner field. Search needs its own narrow canonical read collaborator;
  the existing deal reference reader returns serialized chat context, not
  document identities or an authorization decision.
- The [permanent deletion proposal](deal-permanent-deletion-api-plan.md) shares
  the need for graph deal ownership, resumable backfill, and canonical liveness
  checks. It is a design dependency, not implemented behavior. Coordinate one
  ownership rollout; do not independently add competing migrations or assume
  its deletion ledger, lifecycle gate, or remote write fence already exists.

## Research basis and recommendation

Keyword and semantic retrieval solve complementary needs:

- HelixDB provides native BM25 full-text search and approximate vector search
  in one engine. This is a good fit for retaining exact names, acronyms, IDs,
  values, and filename terms alongside semantic similarity. [HelixDB
  overview](https://github.com/HelixDB/helix-db)
- Semantic embeddings retrieve related text even with few overlapping words.
  Quarry already defaults to text-embedding-3-small at 1,536 dimensions; query
  embeddings must use the same configured model and dimensions as the indexed
  chunks. OpenAI recommends cosine similarity for its normalized embeddings.
  [OpenAI embeddings guide](https://developers.openai.com/api/docs/guides/embeddings)
- BM25 scores and vector distances have incompatible numeric scales. RRF uses
  only their ranks, making it a low-tuning, robust first combination method.
  [OpenSearch RRF documentation](https://docs.opensearch.org/latest/vector-search/ai-search/hybrid-search/rrf/)
- OpenAI's own retrieval API exposes independent embedding and text weights for
  RRF, which supports making weights explicit and tuning only against judged
  results. [OpenAI retrieval guide](https://developers.openai.com/api/docs/guides/retrieval)

Use weighted RRF initially:

    hybridScore(file) = sum(sourceWeight / (60 + sourceRank(file)))

The sources are body keyword/BM25, body semantic/vector, and filename keyword
matches. Start all weights at 1.0; make them server configuration, never
browser input. Do not add or normalize raw BM25 scores and vector distances.

For each source, first retain its highest ranked chunk per file, then fuse at
file level. This prevents a long document with many adjacent chunks from
monopolizing the result page. Prefer a keyword anchor when a result has one, so
the user sees a literal excerpt; otherwise use its semantic anchor. A
filename-only result intentionally has no fabricated content excerpt.

## User experience contract

1. The trigger reads Search this Data Room and is available while the Data Room
   shows its file-review view and while a document preview replaces the sidebar.
   The floating panel remains bounded to the Data Room, not the selected PDF.
2. The panel has a Basic Search input and an Advanced Search disclosure. The
   disclosure has accessible, separately labelled fields for All these words,
   Any of these words, None of these words, and This exact phrase; it retains
   typed values while the panel stays open. A concise scope label says This
   Data Room. Do not render an actionable Entire Project choice until its
   separate scoped API and authorization work exist.
3. Search starts on explicit Enter or the Search button, never every keystroke.
   This avoids needless embedding cost. A later submit supersedes a stale
   completion and the panel exposes an accessible loading state.
4. All is the default hybrid view. Cards display one or more truthful badges:
   Keyword, Semantic, and Filename. Filter chips can show those source groups
   without duplicating a card.
5. Activating a stored result opens the stored file through the existing
   document-session flow. If pageStart is present, request that PDF page only
   after the viewer has confirmed its page count. Non-PDF, image-description,
   and filename-only results open the document and show their location label;
   they do not promise a page jump.
6. Highlight only escaped literal positive terms that occur in the keyword
   anchor: the basic query, All/Any terms, or exact phrase. Never highlight
   excluded terms or construct HTML from a query. Semantic-only cards get a
   Semantic label, not an invented highlight.
7. Make loading, empty result, no indexed documents, semantic unavailable,
   error/retry, and success states visually and programmatically distinct.
   Preserve current dragging, keyboard navigation, focus restoration, reduced
   motion, and accessible labels.

### Advanced-query semantics and scope

Normal user text remains a bound keyword query plus semantic input. The
Advanced Search fields refine that search; they do not expose raw Helix syntax,
quoted operators, wildcards, prefix matching, or client-supplied vectors.

| Field | Semantics in v1 | Retrieval and display rule |
| --- | --- | --- |
| Basic Search | Optional free-text query. | Supplies normal keyword/title retrieval and the semantic query when present. |
| All these words | Whitespace-separated literal terms; every term must occur in the same returned body chunk. | A hard body constraint for keyword, semantic, and hybrid results. |
| Any of these words | Whitespace-separated literal terms; at least one must occur in the same returned body chunk. | A hard body constraint when supplied. |
| None of these words | Whitespace-separated literal terms; none may occur in the returned body chunk. | A hard exclusion; it never supplies a semantic query or a highlight. |
| This exact phrase | One trimmed literal phrase, not quote-delimited query syntax. | The normalized contiguous phrase must occur in the returned body chunk. It is a hard body constraint and one literal highlight range. |

All supplied structured positive groups combine with AND: a match must satisfy
every All term, at least one Any term when given, and the exact phrase when
given. Basic Search remains the normal hybrid retrieval intent rather than a
second raw Boolean operator. These constraints apply to the matching passage,
not an unbounded scan of every document blob. A filename alone never satisfies
an advanced content criterion; filename-only candidates remain available only
for a basic query with no advanced body criterion.

The handler canonicalizes Unicode whitespace, trims blank fields, rejects
control-only input, deduplicates equivalent terms for matching, and enforces
term boundaries through one shared server-side normalizer. At least one
positive value (Basic Search, All, Any, or exact phrase) is required; None
alone returns 400. Cap the combined supplied text at 1,000 Unicode scalars,
bound the number of parsed terms, and give the phrase its own smaller bounded
length. The concrete caps and case/diacritic/identifier normalization are
compatibility-fixture decisions, not UI choices.

The Phase 0 Helix spike must prove that all positive and negative body
predicates can be pushed into a deal/current-version-scoped candidate query,
including exact-phrase and punctuation-heavy identifier behavior on the pinned
analyzer. A pure matcher using the same normalizer rechecks every returned
anchor before it is shaped as a defense-in-depth assertion; it must not become
a global blob scan or a post-filter of an owner-wide top-K. Do not enable the
exact-phrase field if that proof fails. Until then, retain the basic Keyword
label rather than falsely advertising phrase precision.

For semantic and hybrid mode, form at most one embedding input from the
positive user intent: Basic Search when present, otherwise the canonicalized
All/Any/phrase values. Never send the None field to OpenAI. Semantic ranking
can broaden candidate ordering, but it never relaxes a supplied advanced body
constraint. Keyword mode makes no OpenAI call.

The initial scope selector has exactly one implemented value, `dataRoom`,
represented by the `/deals/{dealId}` route. A future Entire Project choice
requires a separately defined canonical project-to-deal relationship,
principal-to-project authorization before retrieval, a project-scoped endpoint
that is not based on caller-supplied workspace identity, result deal/context
metadata, and cross-Data-Room navigation. It must not be enabled by filtering
the selected deal endpoint in the client or by widening a Helix query.

## Product API

Create one product-shaped endpoint; new clients use /api/v1 through QuarryApi:

    POST /api/v1/deals/{dealId}/documents/search

Request:

    {
      "query": "customer concentration risk",
      "criteria": {
        "allWords": "customer concentration",
        "anyWords": "renewal churn",
        "noneWords": "draft internal",
        "exactPhrase": "top 10 customers"
      },
      "mode": "hybrid",
      "limit": 20
    }

`query` and `criteria` are each optional, but the request needs at least one
positive value as defined above. `criteria` is optional as a whole and contains
only optional strings; an empty canonical criteria object is omitted. Preserve
the user's field boundaries in the request rather than requiring the browser
to split terms or construct Boolean syntax. Mode is optional and closed:
hybrid, keyword, or semantic. Limit defaults to 20 and is bounded to 1 through
50. Validate the path identifier, mode, limit, every text field, the combined
Unicode-scalar ceiling, positive-query requirement, and criteria shape before
an OpenAI request. Bound each internal Helix source to no more than its current
100-candidate maximum, for example min(100, max(50, 5 times requested limit)).

Response fields use camelCase and expose identity/location rather than raw
graph rank values:

    {
      "query": "customer concentration risk",
      "criteria": {
        "allWords": ["customer", "concentration"],
        "anyWords": ["renewal", "churn"],
        "noneWords": ["draft", "internal"],
        "exactPhrase": "top 10 customers"
      },
      "mode": "hybrid",
      "results": [
        {
          "fileId": "stable-file-id",
          "versionId": "current-version-id",
          "displayName": "FY25 Customer Analysis.pdf",
          "mimeType": "application/pdf",
          "chunkId": "stable-chunk-id",
          "excerpt": "bounded, safe display excerpt",
          "pageStart": 12,
          "pageEnd": 12,
          "charStart": 1840,
          "charEnd": 2369,
          "sectionPath": "optional section",
          "matchSources": ["keyword", "semantic"]
        }
      ]
    }

The response echoes canonicalized criteria so the UI can accurately show what
was applied; omit absent criteria fields. It does not expose raw graph query
syntax, rank values, or internal analyzer state. Omit chunk/excerpt/location
fields for filename-only hits. Semantic and hybrid mode return a sanitized 503
if the server OpenAI embedding capability is absent or query embedding fails;
they must not pretend keyword-only results are hybrid. Keyword mode must work
without OpenAI. Use existing AppError envelopes for 400, 404/authorization,
503, and internal errors; do not expose provider bodies, index internals, query
text, or document text in an error.

Keep the existing keyword/vector endpoints as temporary compatibility routes,
but mark them deprecated in the client contract and add no consumers. Do not
join them in the browser. Remove them only through an intentional
compatibility/version decision.

## Target ownership and request path

    Data Room panel
      -> QuarryApi.searchDealDocuments(dealId, input)
      -> browser HTTP or validated Tauri relay
      -> Axum handler validates path and JSON
      -> DocumentSearchService
           -> canonical deal/access lookup
           -> consumes validated canonical criteria
           -> OpenAiClient embeds positive intent once when required
           -> DocumentGraphSearch issues bounded, predicate-aware Helix reads
           -> rechecks anchor, validates current version, collapses, RRFs, shapes excerpt
      -> typed response
      -> existing stored-document preview and optional page request

Maintain the repository dependency direction:

- Bootstrap constructs the optional Arc OpenAiClient, typed index, canonical
  deal/document collaborator, and one DocumentSearchService. Ambient
  configuration remains in AppConfig/bootstrap.
- The handler extracts and validates transport input. It does not construct a
  client or rank candidates. Json remains the last body-consuming extractor.
- The handler owns transport criteria normalization and validation through a
  pure shared criteria helper. The search service owns availability policy, one
  query embedding, candidate orchestration, anchor/identity verification, pure
  ranking, and result shaping. Clone Arc values before awaits, bound provider
  concurrency/timeouts, and hold no lock across await.
- The index repository owns typed, deal/current-version-scoped Helix reads and
  receives typed body predicates. It must not know HTTP DTOs, OpenAI, UI labels,
  or RRF policy.
- The canonical document/deal collaborator confirms that the deal is active
  and supplies owner workspace plus current, non-deleted file/version metadata
  through bounded batch reads. Derive workspace from canonical ownership; accept
  no client workspace or company scope on the new endpoint. Reject mismatched
  graph identities before excerpts are returned. Recheck deal/file liveness
  before returning an in-flight response, so archive/removal cannot expose stale
  hits. Once the deletion lifecycle gate exists, honor its barrier as well.
  Archived deals return 404 under this initial active-Data-Room policy; completed
  or cancelled engagements are not automatically archived. Once real identity
  exists, a deal-access policy must bind the authenticated principal before
  retrieval. Current caller-supplied email is development isolation, not
  authorization.
- The frontend maps the DTO to cards. It creates no embeddings, performs no
  rank fusion, and keeps no browser-storage cache of document content.

## Graph and index migration

Add deal_id to FileNode, FileVersionNode, and FileChunkNode; populate it from
the SQLite-validated FilePersistenceInput.deal_id. Preserve workspace_id as
the current owner projection. Extend graph write preflight, deterministic
response validation, and all result integrity checks with the deal identity.

Create additive v2 index definitions rather than changing an existing
create-if-not-exists definition:

- deal-partitioned QuarryFile.display_name BM25 index for filenames;
- deal-partitioned FileChunk.text BM25 index for document text; and
- deal-partitioned FileChunk.embedding cosine vector index for semantic search.

Use names such as file_display_name_by_deal_v2, chunk_text_by_deal_v2, and
chunk_embedding_by_deal_v2. Update DOCUMENT_INDEX_NAMES, bootstrap readiness
checks, and the opt-in Helix compatibility suite. Do not retire the old
workspace indexes during ordinary startup.

New repository queries must scope by deal and prove that candidates belong to
the current published path:

    QuarryFile(deal_id)
      -CURRENT_VERSION-> FileVersion(deal_id)
      -HAS_CHUNK-> FileChunk(deal_id)

First prove the pinned helix-db 3.0.0 Rust SDK can combine that path, the v2
partition, keyword/vector retrieval, and every structured body predicate. The
keyword query must retain only chunks meeting the All/Any/None/phrase rules;
semantic candidates must be bounded and receive the same typed predicate.
If the SDK cannot express a necessary scoped predicate, write bounded,
parameterized Helix verification queries and keep the control unavailable; do
not permanently post-filter a global owner-wide top-K in Axum.

Do not duplicate every filename/full document into chunks merely to obtain
title search. Keep raw chunk text for excerpts and use the indexed file title
as the third retrieval source.

### Safe ownership backfill and index population are prerequisites

Existing graph nodes lack deal_id. Prefer an operator-only, bounded, resumable
property backfill from canonical SQLite file ownership, shared with the deletion
rollout. Retain workspace/file/version/chunk identities, chunk text, embeddings,
and published edges. Cover historical, soft-deleted, and unpublished/partial
nodes for ownership reconciliation, while search continues to admit only live,
current, published versions. Never infer deal ownership from matching content
hashes, filenames, company names, or chat associations.

Drain writers, verify backups and the selected environment, support dry-run,
checkpoint batches, and make retry idempotent. Missing or contradictory canonical
ownership is reported and suppressed from search rather than guessed or silently
purged. Prove on the pinned Helix target whether additive v2 indexes populate
existing nodes after the property update. Keep old indexes until parity passes.
A reader-first transition may tolerate absent deal properties only for canonical
reconciliation; absent/mismatched ownership never qualifies a search candidate.

If index population requires rewriting nodes, or parser/chunk/model changes
require new content projection, implement a separate safe rebuild path that:

1. reads current, non-deleted SQLite documents and canonical blobs only from a
   disposable or explicitly authorized environment;
2. reuses the validated parser, embedding, graph-preflight, and stable
   file/version/chunk identity pipeline with the new deal projection;
3. bounds parse/embed work like normal ingestion and logs only opaque IDs,
   counts, statuses, and error categories;
4. performs one idempotent file/version at a time, supports dry-run and retry,
   and never deletes canonical SQLite rows to conceal a projection error; and
5. verifies identity/count parity plus keyword and semantic fixtures before
   traffic is enabled.

Neither backfill nor rebuild may invoke reset_development or clear_helix.
Schema 9 must be preserved; if durable tooling checkpoints need SQLite tables,
design a targeted data-preserving upgrade from the then-current marker and
coordinate it with deletion's proposed ledger upgrade. Do not reserve marker 10
independently in both plans. The command needs a documented
backup, approval, retry, and rollback procedure. A per-file graph rewrite can
briefly leave that file without chunks after a failure; retry it and keep the
feature disabled until parity passes.

## Implementation phases

### Phase 0: storage and retrieval spike

1. Add synthetic, non-sensitive Helix compatibility fixtures: multiple owners,
   two deals for one owner, current and historical versions, duplicate content,
   filename-only matches, numeric IDs, punctuation, mixed case, advanced
   all/any/none/phrase cases, a phrase spanning chunk boundaries, and a
   paraphrase with no common keywords.
2. Prove v2 index readiness, deal partitioning, current-version traversal,
   text/vector/title queries, source ordering, and analyzer behavior on the
   pinned local compatibility target. Include case/diacritic normalization,
   word boundaries, negative terms, exact phrase, punctuation-heavy IDs, and
   the invariant that all advanced predicates apply before the top-K is chosen.
3. Implement and test a pure RRF helper with fixed ranked lists. Verify
   deterministic ties by hybrid score, best source rank, file ID, then chunk ID.
4. Measure the existing 800-token non-overlapping chunks on representative
   staging material before changing them. Do not add overlap speculatively:
   it increases embedding/storage cost and forces reindex. OpenAI's 800-token,
   400-token-overlap default is a reference point, not a Quarry mandate.
   [OpenAI retrieval chunking](https://developers.openai.com/api/docs/guides/retrieval)
5. Write the shared ownership-backfill, index-population, and conditional rebuild
   operational design before changing graph models. Stop
   if Helix cannot accurately express deal/current-version retrieval.
6. Specify one shared criteria normalizer/matcher and publish its fixture table
   before wiring UI fields. Decide the concrete term-count, phrase-length, and
   Unicode-normalization bounds from this spike; do not let TypeScript and Rust
   independently reinterpret the user's fields.

### Phase 1: backend contract and service

1. Add transport-free request/result types, typed AdvancedSearchCriteria, and
   a closed SearchMode. Make the basic query optional only when a positive
   criteria field is present; use typed file/version/chunk/source/location
   fields and serde camelCase DTOs. Leave legacy search DTO casing alone until
   deprecation.
2. Add the deal_id graph projection, v2 DDL, readiness, and typed repository
   queries for predicate-aware body keyword/body vector candidates and
   unconstrained filename candidates. Reject filename-only hits when advanced
   body criteria were supplied.
3. Add a narrow OpenAiClient query-embedding method accepting a borrowed query,
   using the configured ingestion model/dimension and validating finite
   response dimensions. It receives the canonical positive intent only; the
   browser supplies neither vector nor model.
4. Refactor DocumentSearchService to depend on the index, canonical
   deal/document collaborator, and optional embedding client. Consume criteria
   validated once by the handler/helper, run independent eligible lexical/title
   reads while creating the query embedding where useful, then retrieve semantic candidates,
   recheck anchors/identities, collapse, fuse, and shape bounded excerpts.
   Keyword mode makes no OpenAI call.
5. Add the deal-scoped route in the documents-search router and preserve thin
   handlers/sanitized error mapping. Invalid criteria, an exclusion-only
   request, or unavailable exact-phrase capability have explicit client-safe
   errors rather than silent broadening.
6. Implement and test the shared backfill and any required rebuild. Document
   explicit operator authorization for the selected environment, without running
   either as startup behavior. Enable the endpoint/UI behind validated server
   configuration only after ownership/index/current-version parity passes.

### Phase 2: shared React and desktop integration

1. Add searchDealDocuments(dealId, input) to QuarryApi and both HTTP/Tauri
   adapters. The generic desktop /api/v1 relay should need no new native
   command; prove that with its adapter tests.
2. Replace the DataRoomDocumentSearch fixture import with a request owner that
   tracks basic and advanced drafts, canonical submitted criteria, loading,
   results, error, retry, capability availability, and stale request identity.
   Use a disclosure with labelled inputs, descriptions of AND/OR/exclusion
   behavior, and a visible This Data Room scope. Do not retain a runtime
   fixture fallback.
3. Extend the coordination already in DataRoomWorkspace, where deal ID, stored
   nodes, selection, preview handle, and the panel boundary meet. Keep the explorer
   quick-action trigger for selected previews and add an unselected-view trigger
   without replacing the Deal Room sidebar. Use one deal-keyed request/result
   owner; selecting or closing a preview must not reset the submitted search.
   Closing/switching the deal invalidates requests and clears retained results.
4. On activation locate the stored node by file ID, select it with
   useDocumentSession, then request a page only if the loaded viewer has a
   valid page count. Show an unavailable state if the indexed file disappeared.
5. Preserve the Upload callback, the existing Notes placeholder, and Synthesis
   Canvas's provider keyed by user and deal, persisted thread picker, and
   immutable thread association.
   Search must not inject retrieved document text into chat implicitly; any future
   grounded-chat integration needs a separate explicit contract. Preserve existing
   dialog focus, keyboard, drag, token, and motion behavior.
   Copy must say Search this Data Room, accurately state indexed-document
   scope, and avoid promising phrase/Boolean behavior until the capability
   spike has passed.

### Phase 3: full Data Room source coverage

1. Add explicit server-owned imports for approved local Data Room sources and
   a future SharePoint integration. Each import must become a canonical SQLite
   file/version and run through the same indexing flow. Never search arbitrary
   local paths from web or desktop UI.
2. Show Indexing, Indexed, Unsupported, Index failed with retry, and Not
   imported states so users know why a visible file is absent.
3. With real authentication/tenant authorization, bind principal-to-deal
   membership at the handler/service boundary and test it before calling
   search production-safe.
4. Consider query suggestions or a second-stage reranker only when evaluated
   evidence reveals a clear retrieval gap. No LLM answer generator is needed
   for the initial search feature.

### Phase 4: Entire Project scope only after an owned authorization model

1. Define the canonical project aggregate, its relationship to deal Data Rooms,
   and a principal-to-project authorization policy. Do not overload the current
   caller-supplied workspace email or expose its value to the browser as a
   search scope.
2. Add a separately named project-scoped product route, transport contract, and
   typed canonical-access collaborator. Its query must be partitioned and
   authorized before Helix retrieval, rather than combining deal-local pages in
   React or post-filtering a broad top-K.
3. Extend results with the minimal deal/Data Room context required to open a
   result safely, test navigation/focus restoration across Data Rooms, and only
   then enable the Entire Project control. Preserve the same advanced-criteria
   semantics in both scopes.

## Verification, evaluation, and operations

### Automated and manual checks

- Backend unit tests: criteria canonicalization/bounds, exclusion-only
  rejection, same-model/dimension embedding, positive-intent-only embedding,
  unavailable capability, timeout/error redaction, deal/current identity,
  RRF weights/dedup/ties, source labels, advanced-anchor matcher behavior,
  filename-only shape, and safe excerpts.
- Route/service tests: v1 contract, 400/404/503, no embedding in keyword mode,
  no client vector accepted, no cross-deal result for the same owner, no
  filename-only result when body criteria exist, and no silent widening when a
  predicate or phrase capability is unavailable.
- Helix compatibility tests: v2 DDL readiness and correctly scoped title,
  keyword, vector, all/any/none, and exact-phrase results; historical and
  other-deal chunks never return.
- Backfill/rebuild tests: dry run, idempotent retry, interrupted resume,
  historical/partial nodes, missing/mismatched ownership, v2 population/parity,
  one-file failure reporting, stable IDs/embedding preservation for property-only
  updates, and SQLite preservation, all with temporary databases.
- Lifecycle tests: archived/missing deals, status versus archive distinctions,
  shared companies across deals, same bytes in different deals, and in-flight
  liveness rechecks. When deletion lands, test its barrier during search and
  backfill/rebuild so neither stale results nor tooling can resurrect content.
- Frontend tests: contract/adapters, advanced-disclosure labels/instructions,
  basic and structured draft submission, loading/empty/no-index/error/retry,
  stale result suppression, badges, escaped positive-term/phrase highlighting,
  cross-document activation, page validation, deleted result,
  keyboard/focus/accessibility, reduced motion, narrow viewport behavior, and
  the unavailable Entire Project state until Phase 4 lands. Cover both selected
  and unselected triggers, result retention across preview changes, user/deal
  switches, and unchanged Upload/Synthesis Canvas thread behavior.

Run the repository-native gates from their build roots:

    backend/
    cargo fmt --all -- --check
    cargo check --locked --all-targets
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --all-targets

    frontend/
    npm run typecheck
    npm run check:boundaries
    npm test
    npm run build:web
    npm run check:web-bundle
    npm run build:desktop-ui

    frontend/src-tauri/ when native relay behavior changes
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --all-targets

Do not use cargo run as a test; it can migrate SQLite and requires Helix.
Manually inspect real searches in both web and desktop only with disposable or
explicitly authorized indexed data.

### Relevance evaluation

Build a reviewable, access-controlled query-to-relevant-file/chunk judgment
set. Do not commit customer text or raw queries. Include protected staging
cases and synthetic equivalents for:

- exact company names, acronyms, clauses, dates, amounts, IDs, and filenames;
- natural-language paraphrases and synonyms;
- PDF pages, DOCX sections, XLSX sheet/header/value queries, PPTX slide text,
  and image-description retrieval;
- short/long/case/punctuation/typo variants; structured all/any/none/phrase
  combinations; no result; version conflicts; and cross-deal access attempts.

Compare keyword-only, semantic-only, and hybrid Recall at K and MRR/nDCG at
the UI result limit. Record latency/error rates and human judgment of
excerpt/location. Hybrid must preserve keyword performance for identifiers,
improve paraphrase recall, never leak another deal/version, and fit an agreed
interactive latency budget. Set numeric thresholds only after establishing a
baseline; invented thresholds are not useful.

Add privacy-reviewed feedback cases over time and run evaluation on every
change to fusion weights, chunking, parsers, embedding model/dimension, Helix
version, or retrieval query. [OpenAI evaluation best
practices](https://developers.openai.com/api/docs/guides/evaluation-best-practices)
recommends continuous evaluation with representative cases.

### Privacy, observability, and resilience

- Keep OPENAI_API_KEY server-side. Query text/criteria and bounded results must
  cross the existing validated HTTP/Tauri JSON relay for desktop search; do not
  prohibit that required transport. Query embeddings remain server-side. Never
  put provider secrets in IPC, and never persist query/document values in Vite
  variables, browser storage, analytics payloads, or logs.
- Treat structured criteria as query text for privacy and redaction. Only the
  canonical positive intent may reach the embedding provider; excluded terms
  stay within the server's retrieval/matching path.
- Use tracing for mode, opaque request/deal IDs, source candidate/result counts,
  latency phases, and failure categories. Exclude query/document/user values.
- Bound query-embedding and Helix work with timeouts/concurrency controls.
  One request makes at most one embedding request and bounded index reads; it
  makes no text-generation call.
- Do not add an unbounded search cache. Any later cache must be in-memory,
  short-lived, bounded, scope-keyed by a cryptographic digest, and separately
  threat-modeled.
- Add rate limiting with real identity. Until then, deployment/network controls
  are the only protection; do not imply search has authorization that Quarry
  does not yet implement.

## Completion criteria

1. One Data Room search returns deal-scoped, current-version results carrying
   truthful keyword, semantic, and filename evidence.
2. Exact terms/filenames remain discoverable, paraphrases can surface semantic
   results, advanced all/any/none/phrase rules are truthful and testable, and
   another deal or stale version cannot appear.
3. The backend creates one compatible query embedding; missing capability is
   explicit rather than silently degraded.
4. Cross-document selection and valid PDF-page navigation work; non-PDF and
   unavailable results are safe and truthful.
5. Existing documents pass ownership backfill, index-population, and any needed
   rebuild/parity gates. No destructive Helix clearing or canonical SQLite
   deletion is used.
6. There is no runtime fixture fallback. Both frontend adapters and relevant
   backend/frontend/Tauri tests pass with manual accessibility inspection.
7. The implementation updates docs/ARCHITECTURE.md for API, graph/index,
   rebuild/recovery, source scope, security limitation, and operations. Final
   handoff includes git diff --check, final status, exact checks, and remaining
   external/authorization uncertainty.
8. Entire Project remains unavailable until Phase 4's project ownership,
   authorization, route, retrieval, and cross-Data-Room navigation criteria
   are complete; it is never shipped as a broad workspace search.

## Architecture impact of this refresh

Architecture impact: none — this updates proposed work against the current
implementation without changing routes, runtime boundaries, schema, or behavior.
The relevant architecture sections were rechecked; implementation must update
the canonical architecture as described in completion criterion 7. No migration,
backfill, rebuild, reset, or external integration was executed for this refresh.
