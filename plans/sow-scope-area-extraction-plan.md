# SOW scope area extraction and Deal Room display plan

Date: 2026-11-01. Status: proposed; this document does not implement the feature.

## Outcome

When a user uploads a statement of work (SOW) in deal creation or through **Add deal resources**,
extract its scope areas, persist them with the deal, and show them on the Deal Room Overview after
save and after reload in both web and desktop. Preserve the structure in the supplied example:
ordered scope-area headings, their ordered subsection headings, and the review activities under
each subsection. Capture the source's wording and ordering rather than rewriting, deduplicating,
or inventing content. Render the extracted material as plain text, not HTML.

This is distinct from the existing `deal_scopes` / `DiligenceScope` fixed categories selected in
the deal form. Do not overwrite, infer, or map one into the other. Existing Key Questions,
resources, authorization, routes, file limits, and layout remain intact. This plan does not add
manual scope editing, search/indexing, automated SharePoint/SOW-link retrieval, document storage,
background jobs, or automatic historical backfill. The user-provided sample is reference material;
use synthetic equivalent examples in committed tests and do not copy its deal-specific content.

## Current state and boundaries

- `backend/src/domains/deals/upload.rs` accepts link fields and repeated `files` parts. Both the
  SOW and project-timeline selections are flattened into that same field in
  `frontend/src/components/hub/sidebar/AddDealModal.tsx` and
  `frontend/src/components/deal-room/DealResourcesDialog.tsx`. The backend therefore cannot
  reliably identify which file is the SOW. `backend/src/domains/deals/service.rs` sends all files
  to the existing OpenAI extraction and persists the resulting `key_questions_json` in
  `deal_metadata`. The prompt is `backend/prompts/diligence_review.md` and currently requests only
  explicitly listed key questions. File bytes sent to this extraction are not a durable SOW record.
- `deal_metadata` is canonical SQLite metadata and is returned on deal create, list/get, update,
  and metadata-save responses. `frontend/src/data/dealExtraction.ts`,
  `frontend/src/hooks/useWorkspaceDeals.ts`, and `frontend/src/data/workspace.ts` map it into
  the shared Deal Room model. The Overview renders `DealSummaryCard`; it already shows separately
  selected scope categories and Key Questions. Web and desktop use the same React tree and Axum API.
- Metadata links are saved before extraction; failed extraction leaves the saved links and prior
  questions in place. Omitted/blank link behavior and the link-only save/cache behavior are existing
  contracts. A stored SOW or SharePoint URL is not fetched. Deals and metadata use authenticated
  membership/capability checks, including write-time authorization in the repository.
- `backend/src/app/migrations.rs` currently uses schema marker 14, supports validated marker-12/13
  upgrades, validates owned DDL and foreign keys, and protects existing data. Reconfirm the live
  marker and current migration paths before implementation. Helix and template processing
  are not involved in this feature.

## Proposed contract and behavior

### Data model

Add a separate ordered `scopeAreas` value to deal extraction and persisted metadata. A scope area
contains its source heading (including an optional displayed number), an ordered list of sections,
and each section contains its heading and ordered activity strings. Allow an area to have direct
activities and a section to be absent where the source has no subsection; do not invent a heading.
Use a concrete typed Rust/TypeScript model with a single consistent shape, for example:

```json
{
  "scopeAreas": [
    {
      "heading": "SCOPE AREA 1: EXAMPLE",
      "activities": [],
      "sections": [
        { "heading": "Example subsection", "activities": ["Review example capability"] }
      ]
    }
  ]
}
```

Preserve source headings, numbered labels, case, punctuation, and order; normalize only extraction
artifacts such as incidental whitespace. Do not treat questions or timeline items as scope
activities unless they are actually within a scope area. Define and enforce finite counts, nesting,
per-string and total serialized-size limits in the backend before storage; reject incomplete,
malformed, oversized, or ambiguous structured output instead of silently truncating it. Keep those
limits documented and tested against long but reasonable SOWs. A missing field, malformed output,
provider refusal, or uncertain/partial result is an extraction failure, not a successful empty result.
A valid explicit empty array means the SOW was successfully reviewed and contained no scope areas.
Do not log document contents or provider response bodies in error messages.

Persist a nullable `scope_areas_json` on `deal_metadata`: `NULL` means not extracted/unknown
(including existing records); a validated JSON array, including `[]`, means a successful extraction.
Use an array validity/type check in SQLite and typed validation of every nested object in Rust.
Mirror the current `keyQuestionsJson` contract with a nullable camelCase `scopeAreasJson` in
metadata, and expose typed `scopeAreas` in the extraction response. Keep `null` separate from `[]`
through the frontend mapper. If a different typed metadata response is chosen instead, document and
test its compatibility explicitly rather than returning both representations inconsistently.

### Identify the SOW without changing legacy meaning

Extend the existing metadata multipart endpoint and compatibility alias with two explicitly named,
optional file parts, `sowFile` and `projectTimelineFile`, with at most one of each. Update the shared
input type, both creation/resource dialogs, the web `FormData` adapter, and the desktop multipart
adapter so they retain the selected role all the way to Axum. Change Tauri's `multipartFiles` /
`fileToMultipart` plumbing only as needed to preserve the per-file field name; keep its existing
path, origin, filename, MIME and byte checks. Validate names, duplicates, combinations, and aggregate
50 MB limits at the HTTP boundary; apply existing file validation to each file. Test local desktop
source-file selection as well as browser uploads. Do not infer a role from file name, list position,
or an unverified model guess.

For compatibility, continue to accept existing repeated `files` parts with their existing
key-question extraction behavior. Treat them as unclassified: do not update scope areas from them.
Reject requests mixing legacy and role-labelled file parts to avoid ambiguous behavior. A timeline-only
request may retain the existing key-question workflow, but must never replace scope areas. A link-only
request does not invoke extraction and preserves both persisted questions and scope areas. A request
with an explicit SOW may send the SOW and timeline to the existing key-question workflow, but the
scope-area extraction must inspect only the explicitly labelled SOW, with a clear source boundary;
use a separate model invocation if needed to make that guarantee. Do not weaken the existing Key
Questions prompt or limit its historical file handling as an incidental change.

### Extraction, persistence, and failure semantics

1. Extend the prompt catalog and `backend/prompts/diligence_review.md` with a dedicated SOW scope
   prompt and typed response. Treat document content and prompt context as untrusted data. Request
   verbatim structured content, not a summary, and require an explicit outcome for no scope areas.
   Preserve heading/paragraph hierarchy across supported SOW formats; do not claim full OCR or
   extraction fidelity until tested. Missing OpenAI capability must fail explicitly when invoked.
2. Parse and validate model output without exposing raw document/provider contents to clients or
   logs. Test ambiguous headings, missing sections, unsupported/empty files, fenced or malformed JSON,
   refusal, missing fields, oversize results, and source instructions that attempt prompt injection.
   Use representative synthetic SOW examples; run provider-free automated tests with fakes.
3. On a successful explicit SOW extraction, replace the prior scope snapshot as one unit. A
   successful explicit empty result replaces it with `[]`. A no-file, link-only, timeline-only, or
   unclassified legacy request leaves it untouched. A failed or incomplete extraction must preserve
   the previous snapshot and surface a retryable error; it must never store partial/empty data as if
   successful. Preserve the existing policy that valid link updates may already be committed before
   extraction fails, and tell the user when a retry is needed.
4. Make successful question and scope updates atomic when both are derived in the same request;
   failed scope extraction must not leave a half-updated pair. Avoid overwriting newer link changes
   or an intervening scope update with stale metadata: use narrow transactional updates plus an
   explicit compare-and-swap/version or equivalent per-deal concurrency control. Recheck the actor's
   current edit capability and archive status in the final write transaction. Define deterministic
   conflict behavior and return a sanitized, actionable conflict response. Do not keep a SQLite
   transaction or lock open while awaiting the model.
5. Existing uploaded file bytes are not durably stored by this metadata flow. Do not fabricate
   source IDs, evidence citations, durable filenames, automatic re-extraction, or provenance. A
   link-only source remains a link until an authorized SOW upload is provided.

## Implementation sequence

1. **Schema and migration.** Add the nullable metadata column in `backend/src/app/migrations.rs`;
   use a new schema marker (15 if 14 is still current). Implement and test non-destructive,
   transactional upgrades from every currently supported predecessor (12, 13, 14), as well as fresh
   creation and current-schema validation. Adapt owned-DDL validation for old and new layouts rather
   than comparing a legacy table against the new DDL. Preserve every existing deal, question, link,
   membership, notebook, template and document; legacy scope remains `NULL`. Validate rollback on
   failure and idempotent restart. Rollback is a drained restore of the matching pre-upgrade backup
   with its matching binary; never invoke the development reset or alter a real database as a test.
2. **Backend contract and repository.** Add typed area/section DTOs and validation in the deals
   domain, extend `DealMetadata`, metadata reads, aggregate joins and save responses, and bind all
   SQL values through existing repository/client patterns. Distinguish unknown from an extracted
   empty result. Keep `deal_scopes` unchanged. Test fresh and joined reads, malformed persisted data,
   link-only preservation, authorized/forbidden writes, concurrent saves, and atomic rollback.
3. **Role-aware uploads and extraction.** Implement and validate the explicit multipart roles,
   backwards-compatible legacy handling, SOW-only prompt input, and transactional success/failure
   rules above in the Axum handler/service/repository. Include both `/api/v1` and compatibility
   mount/alias behavior where applicable. Review source formats and upstream Responses API limits
   against the installed adapter; add no provider dependency or model configuration without need.
4. **Shared contracts and transports.** Update `frontend/src/data/dealExtraction.ts`,
   `frontend/src/contracts/quarryApi.ts`, both API adapters and the Tauri transport types/relay as
   necessary. Preserve camelCase, URL encoding, both source-selection flows, optional links, and
   existing request bounds. Extend adapter and route tests for labelled fields, legacy callers,
   absent fields, errors and byte/MIME validation.
5. **Deal Room.** Map the saved snapshot into `DealRoomData` and the workspace cache. In
   `DealSummaryCard`, add a responsive **Scope areas** section below the existing summary/resources
   and above Key Questions, using the current Deal Room card styling and semantic nested headings
   and lists. Display headings, sections and activities in source order; wrap long text and ensure
   keyboard and screen-reader access. Show distinct honest states for not yet extracted and
   successfully extracted but empty. On extraction error, retain the previous snapshot and show the
   existing resource-save error/retry affordance; never present stale data as newly extracted.
   Verify immediate save, link-only save, core-deal edit, navigation, reload, and direct-entry paths
   all use persisted metadata consistently. Do not silently fall back to demo fixture data.
6. **Documentation.** Update `docs/ARCHITECTURE.md` and relevant schema/recovery and API
   documentation to describe the actual migration, storage, extraction sources, limits, failure
   behavior, legacy-client compatibility, and no-backfill limitation. Plans are design inputs, not
   canonical implementation documentation.

## Verification and acceptance

- With a synthetic SOW containing multiple numbered areas, multiple subsections, and activities,
  both creation and Add deal resources show the complete ordered hierarchy immediately and after
  browser refresh, desktop restart, and direct URL entry. The existing form-selected Scope and Key
  Questions are unchanged. An uploaded timeline never supplies scope areas.
- Verify SOWs with only area-level items, no sections, an explicitly empty scope, ambiguous/partial
  output, large bounded input, Unicode and special characters, and multiple source formats supported
  by the existing flow. Confirm retry, link-only updates, a timeline-only upload, existing clients,
  model failure, stale/concurrent saves, revoked membership, archive state, and data preservation
  after migration. Include two deals and two users to verify isolation. Validate that frontend
  rendering never interprets source text as HTML and that no content or secrets enter logs.
- Add focused backend parser/prompt, service, multipart/route, repository, migration and concurrency
  tests under the existing manually included test trees. Run focused `cargo test <name-filter>`, then
  from `backend/`: `cargo fmt --all -- --check`, `cargo check --locked --all-targets`,
  `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets`.
- Run relevant frontend mapper, cache, creation, resources, Deal Room and both adapter tests; then
  from `frontend/`: `npm run typecheck`, `npm run check:boundaries`, `npm test`,
  `npm run build:web`, `npm run check:web-bundle`, and `npm run build:desktop-ui`. If native relay
  code changes, also run from `frontend/src-tauri/`: `cargo fmt --all -- --check`,
  `cargo clippy --locked --all-targets -- -D warnings`, and `cargo test --locked --all-targets`.
  Inspect web and desktop UI at relevant widths, keyboard/focus behavior, and applicable themes;
  use isolated fixtures and disposable dependencies. Report existing failing checks separately.
- Re-read architecture documentation, review the implementation and final diff, run
  `git diff --check`, and review `git status --short`. No live model calls, database resets, Helix
  clearing, or other destructive startup operations are routine verification.
