# XLSX and PPTX document-ingestion plan

## Goal

Extend the existing deal-document ingestion flow so users can upload modern
Excel workbooks (`.xlsx`) and PowerPoint presentations (`.pptx`). Each upload
must follow the existing PDF/DOCX lifecycle:

1. validate the upload at the HTTP boundary;
2. parse its bytes into normalized, chunked text;
3. generate embeddings and persist the original bytes, file/version records,
   and Helix v3 projection through the existing ingestion service; and
4. return a PDF from the existing stored-document PDF endpoint so the shared
   PDF viewer can display the selected file.

This is deliberately an ingestion extension, not a new document-store,
viewer, API-version, or frontend runtime design. SQLite remains canonical for
the original uploaded bytes; a converted PDF remains an on-demand, bounded
in-memory preview-cache entry rather than a second persisted source of truth.

## Branch baseline: Helix v3 projection constraints

This plan is being updated after the branch's Helix migration. The document
index now uses the pinned `helix-db` 3.0.0 SDK and its `QueryRequest` contract;
the SDK constructs the `/v2/query` request beneath the configured `HELIX_URL`
origin. `DocumentIndexWriter::initialize` waits for all required index DDL
operations to become usable before Axum starts. Its write facade is
process-serialized and requests durability acknowledgement, but a v3 transport
failure is terminal because the SDK does not expose enough outcome information
to safely replay a graph write.

The local `quarry` launcher verifies and starts the configured digest-pinned,
named Helix container before it polls `/healthz`. A process already listening
on port 6969 must therefore make the named container fail to start rather than
be accepted as a compatible runtime. `HELIX_VECTOR_DIMENSION` is the single
positive configuration source for both the Helix v3 vector-index descriptor
and OpenAI's embeddings `dimensions` request field, so generated vectors and
the configured index have the same dimension.

The XLSX/PPTX work must use that existing projection boundary unchanged. In
particular, a graph write is one atomic, version-scoped request and
`insert_file_version_graph` rejects serialized requests over the current
`HELIX_MAX_QUERY_BODY_BYTES` limit (2 MiB). Spreadsheet and presentation text
can expand substantially beyond their compressed OOXML input, and an
embedding is serialized on every chunk. The implementation must therefore
preflight the exact final v3 graph request after embeddings are attached but
before SQLite is committed. An over-limit Office document is a safe rejected
ingestion result, not a SQLite-success/Helix-failure that an identical retry
cannot repair. This preflight does not split a document graph, weaken the
atomic-write invariant, or change the existing recovery behavior for an
otherwise failed post-commit projection.

## Scope and decisions

- Support **`.xlsx` and `.pptx`** in this change. Reject legacy `.xls` and
  `.ppt` at the ingestion boundary. Although the generic Office converter has
  mappings for those legacy formats, the current PowerPoint parser explicitly
  rejects `.ppt`, and accepting formats without a parser would violate the
  ingestion contract. A later legacy-format feature can make parser,
  validation, and preview support consistent as one change.
- Preserve all supported formats and behavior: PDF, DOCX, PNG, JPEG, WebP,
  and single-frame GIF ingestion must remain unchanged.
- Retain the current routes, multipart field names, job/SSE event names, and
  TypeScript `QuarryApi` signatures. This is an allowed-type expansion of
  `POST /api/v1/deals/{deal_id}/documents/process` and
  `.../process_file`, not a new endpoint.
- Treat the filename extension as the transport type selector after existing
  safe-name validation; do not trust browser-provided multipart MIME types.
  The parsers must independently reject malformed or incorrectly labelled
  bytes.
- Keep parsing and Office conversion off Tokio worker threads. OpenAI remains
  required for the existing embedding stage; LibreOffice/`QUARRY_SOFFICE` is
  required only when an XLSX/PPTX preview is requested, not when it is
  ingested.
- Keep `HELIX_URL` as an origin; do not append `/v2/query`, create another
  Helix client, alter index DDL, or add write retries. The existing configured
  `HELIX_VECTOR_DIMENSION` already drives both the index and OpenAI embeddings
  request dimension; Office code must reuse that coupling rather than create a
  second dimension setting. New Office chunks use the normal graph writer and
  its exact request-size preflight.
- Do not expose a local upload path. Byte parser APIs borrow raw bytes where
  possible and produce the same graph-ready assembly shape as the existing
  DOCX/PDF byte parsers. Path APIs may remain only as CLI/backward-compatible
  wrappers.

The current repository already contains most of the downstream plumbing:

- `calamine` and `pptx-to-md` are locked backend dependencies, and the two
  format modules already extract text from paths.
- `shared::file_policy` already maps XLSX/PPTX to their Office MIME types.
- `StoredDocumentService::render_pdf` already maps those MIME types through
  `OfficeConverter::convert_bytes`, validates the resulting PDF, bounds
  concurrent conversions, and uses the existing 16-entry/128-MiB cache.
- The shared viewer already calls the stored-document PDF endpoint and only
  requires `application/pdf`; it does not need a format-specific renderer.
- The branch already initializes the 13 document indexes to v3 readiness and
  serializes one durable, atomic file/version/chunk graph query. The projection
  builder's 2 MiB cap is a required ingestion boundary for the potentially
  text-dense Office formats, not a value an Office feature may bypass.

The missing links are byte-native parser assemblies, `QuarryFile` dispatch,
ingestion and multipart allowlists, and the upload-modal allowlist.

## Current seams to preserve

| Concern | Current owner | Required preservation |
| --- | --- | --- |
| Ingestion lifecycle | `backend/src/domains/documents/ingestion/service.rs` | Exact-content deduplication, one processing lock per deal/document identity, embedding-before-persistence, SQLite-before-Helix ordering, and per-file job outcomes remain intact. |
| Parser dispatch | `backend/src/domains/documents/formats/mod.rs` | `QuarryFile` currently selects only PDF/DOCX and returns graph-ready assemblies; XLSX/PPTX join this closed dispatch rather than bypassing it in a handler. |
| Transport validation | `backend/src/domains/documents/ingestion/handler.rs` | Preserve filename, empty-file, per-file 50-MiB, total-request 50-MiB, `userId`, and path-scoped `dealId` validation. |
| Canonical persistence | `backend/src/domains/documents/ingestion/persistence.rs` | Preserve content-byte hashes, `document_id`, file/version IDs, source-type/extension agreement, MIME mapping, transaction boundaries, and graph invariants. |
| Local Helix runtime | `quarry` | Verify the configured digest-pinned named container, start it before health polling, and reject a foreign listener that prevents the named runtime from starting. |
| Helix projection | `backend/src/domains/documents/index/{writer,repository}.rs` | Keep the v3 `QueryRequest` boundary, index-readiness bootstrap, one atomic graph write, serialized durable writes, terminal unknown transport outcomes, and the 2-MiB exact-query cap. The configured vector dimension must remain coupled to the OpenAI embeddings request. |
| Stored PDF preview | `backend/src/domains/documents/viewing/service.rs` | Reuse the existing Office byte converter, semaphore, PDF validation, and cache; do not persist a generated PDF or add a separate viewer endpoint. |
| Shared UI and transports | `frontend/src/components/data-room/UploadFilesModal.tsx`, `frontend/src/api/*QuarryApi.ts` | Maintain web and Tauri multipart paths, selected-file async status/focus behavior, and the transport-neutral PDF viewer contract. |

## Implementation plan

### 1. Define byte-native spreadsheet parsing and assembly

Update `backend/src/domains/documents/formats/spreadsheet.rs` so the byte path
is the canonical parser:

1. Add `parse_spreadsheet_from_bytes(bytes: &[u8]) -> Result<String, String>`
   (or an equivalently named borrowed-byte API). Construct a `Cursor` over the
   supplied bytes and use Calamine 0.36.1's
   `open_workbook_auto_from_rs`, rather than writing upload bytes to a local
   source path. Map errors to sanitized, format-specific parser errors.
2. Keep `parse_spreadsheet(path)` only as a compatibility wrapper that reads
   the path then delegates to the byte parser. This prevents the path and
   upload parsers from drifting.
3. Add `SpreadsheetAssembly` and
   `parse_spreadsheet_chunks_from_bytes(bytes, optional_path, user_id)` with
   the same invariants as `parse_docx_chunks_from_bytes`:
   - byte length and SHA-256 come from the original workbook bytes;
   - `document_id` derives from user/workspace identity plus that hash;
   - `file_id` is newly generated before the ingestion service substitutes an
     exact-content match's existing ID;
   - default filename/source type are `Document.xlsx`/`xlsx` when no path is
     available; and
   - `local_path` and `rendered_pdf_path` stay absent for browser uploads.
4. Preserve the existing human-readable row representation
   (`<sheet> row <n>: <tab-separated cells>`), including sheet and row
   identity in embedded text. Chunk that canonical text with
   `token_bounded_ranges`; record a sheet section title where a chunk is
   wholly attributable to one sheet, otherwise leave it unset rather than
   falsely assigning a section. This keeps search results useful without
   breaking character-offset invariants when a chunk crosses sheets.
5. Make invalid/empty/unreadable workbooks return an error before embedding or
   persistence. Do not convert cells with lossy `unwrap`/panic behavior; keep
   current explicit `Data`-variant handling.

### 2. Define byte-native PowerPoint parsing and assembly

Update `backend/src/domains/documents/formats/powerpoint.rs` similarly:

1. Add a public `parse_powerpoint_from_bytes(bytes: &[u8]) -> Result<String,
   String>` and a graph-producing
   `parse_powerpoint_chunks_from_bytes(bytes, optional_path, user_id)`.
   Path-based `parse_powerpoint_file` becomes a small wrapper over the same
   byte-driven logic after it retains its extension guard.
2. The locked `pptx-to-md` 0.4.0 `PptxContainer` owns a
   `ZipArchive<File>` and exposes only `open(&Path, ...)`; it cannot consume a
   `Cursor`. Keep that dependency limitation private: the byte API must stage
   bytes in a uniquely named, `create_new` temporary `.pptx` file, invoke the
   current container with the existing parser configuration, drop it, and
   remove the temporary file on every success and error path. Never store,
   return, log, or treat that temporary path as the uploaded document's local
   path. Use a small RAII cleanup helper (or a vetted existing dependency if
   one is already available) rather than a timestamp-only filename.
3. Retain the current visual ordering of slide elements and extraction of
   text, lists, and tables. Add a stable `Slide N` boundary to canonical
   extracted text and retain slide-number ranges on chunks (using the existing
   `page_numbers` field) when determinable. This produces useful search
   context and corresponds naturally to the eventual converted-PDF pages;
   chunks spanning slide boundaries must include every overlapping slide.
4. Build `PowerpointAssembly` with the same original-byte hash/size,
   deterministic document/chunk identities, token-bounded chunks, optional
   local path, and upload defaults as DOCX/PDF. Its source type must be
   `pptx` and its no-path filename `Presentation.pptx`.
5. Accept only modern PPTX content. The bytes parser must reject empty input,
   invalid ZIP/OOXML packages, and presentations with no readable text under
   the explicit no-readable-text policy used for DOCX, rather than creating a
   no-chunk searchable document. Images remain out of scope for this change:
   do not add OpenAI image-description calls for slide images while wiring
   textual PPTX ingestion.

### 3. Bound untrusted OOXML work before parsing

Before handing XLSX or PPTX bytes to Calamine, `pptx-to-md`, or a temporary
file, add a narrow shared OOXML archive validator in the document-format
layer. It should use the already direct `zip` dependency to inspect the
central directory without extracting arbitrary members and enforce explicit,
tested limits for entry count and cumulative declared uncompressed size (use
the existing 256-MiB decoded-content safety budget as the starting policy).
It should also verify the expected package roots:

- XLSX: `[Content_Types].xml` and `xl/workbook.xml`;
- PPTX: `[Content_Types].xml` and `ppt/presentation.xml`.

This is an additional parser safety boundary; it does not replace the current
50-MiB multipart limits or parser-level validation. Return generic safe
messages and never include archive member contents in logs/errors. Keep the
validator scoped to newly exposed OOXML formats unless a separately tested
hardening change deliberately expands it to DOCX.

### 4. Wire the new assemblies into document ingestion and preflight the v3 graph

Modify `backend/src/domains/documents/formats/mod.rs` and
`backend/src/domains/documents/ingestion/service.rs`:

1. Add XLSX and PPTX variants to `QuarryFile` and `ParsedQuarryFile`. Extend
   `from_parts` to select them case-insensitively from safe filenames and
   continue rejecting every other extension.
2. Dispatch each variant to its byte assembly parser. Set the display filename
   from the original multipart filename after parsing, as the current PDF and
   DOCX dispatch does.
3. Extend `parse_document`'s match so both new `ParsedQuarryFile` variants
   become the normal `Document`/`DocumentChunk` pair. They must then travel
   through the existing embedding and `persist_document_and_chunks` path with
   no new direct repository/client imports.
4. Move the synchronous `QuarryFile::from_bytes(...).parse(...)` work into
   `tokio::task::spawn_blocking`, passing owned filename, bytes, and user ID
   into the closure and mapping join failure to a sanitized parse error. The
   existing outer `tokio::spawn` does not make CPU/ZIP/XML parsing safe for a
   Tokio worker. Keep image validation/description and OpenAI calls in their
   established async paths.
5. After `embed_chunks` supplies every Office chunk embedding, but before
   `DocumentStore::persist` is called, preflight the exact graph request that
   would be submitted to Helix. Reuse the v3 writer's
   `insert_file_version_graph` construction/size validation through a narrow
   index-writer API; do not estimate from raw OOXML bytes or duplicate query
   serialization in ingestion. Build it from the same validated persistence
   input and deterministic file/version/chunk identities that the later
   SQLite-and-Helix path consumes, so the preflight and actual write cannot
   drift. If the request exceeds `HELIX_MAX_QUERY_BODY_BYTES`, return a safe
   processing failure before canonical persistence. Do not split chunks over
   several writes, raise the cap, or reimplement a retry loop for this case.
6. Keep the document model, SQLite schema, and Helix graph shape unchanged.
   Persistence already derives XLSX/PPTX MIME types from
   `shared::file_policy`; extend focused tests to prove the parser
   `source_type` and filename extension satisfy the existing persistence
   invariant. The implementation must preserve the branch's v3 behavior:
   successful SQLite persistence still precedes the one Helix projection, and
   a post-commit Helix transport failure remains terminal at the adapter while
   the existing exact-content ingestion retry remains the recovery path.

### 5. Expand the server upload allowlist, without widening authority

In `backend/src/domains/documents/ingestion/handler.rs`:

1. Add `xlsx` and `pptx` to `validate_document_upload_filename` and update the
   user-facing supported-format message in `collect_document_upload`.
2. Keep the validation ordering and all current safe-name, size, integer
   overflow, user/deal identity, and body-limit checks exactly as they are.
   Do not route XLSX/PPTX uploads through the local-data-room path APIs and do
   not add browser-controlled conversion paths.
3. Add handler/route coverage for both extensions on the versioned and
   compatibility mounts, plus explicit legacy/unsupported rejection. Parsing
   failures should continue to become a failed document job, not an accepted
   unsupported format.

### 6. Reuse the stored-document PDF preview flow

`backend/src/domains/documents/viewing/service.rs` already calls
`office_extension_for_mime_type`, which recognizes XLSX/PPTX, then uses
`OfficeConverter::convert_bytes` in a bounded blocking worker. During
implementation:

1. Do not introduce a new preview route, browser converter, or persisted
   derivative. After ingestion writes the original bytes with the XLSX/PPTX
   MIME type, the existing
   `GET /api/v1/deals/{deal_id}/documents/{file_id}/pdf` route must return the
   converter's validated bytes as `application/pdf` with existing
   `inline`/`private, no-store` headers.
2. Preserve existing two-conversion concurrency, 64-MiB resulting-PDF limit,
   cache key, cache eviction, and `spawn_blocking` behavior. XLSX/PPTX preview
   failures must be clear validation errors (including an unconfigured
   `QUARRY_SOFFICE`) and must not alter/delete the canonical upload.
3. Leave the raw-text endpoint's current PDF/DOCX scope unchanged unless the
   product explicitly needs it. Searchable XLSX/PPTX text comes from the
   stored Helix chunks; the user request requires PDF viewing, not another
   read-text contract.
4. Confirm an Office-capable deployment/configuration has `QUARRY_SOFFICE`
   set to a LibreOffice/soffice executable. This is an existing operational
   prerequisite, not a new browser-visible configuration value.

### 7. Allow the shared Data Room UI to select the new formats

Update `frontend/src/components/data-room/UploadFilesModal.tsx`:

1. Add `xlsx` and `pptx` to the client-side `supportedExtensions` set and to
   the file input's extension/MIME `accept` attribute. Include the canonical
   XLSX and PPTX MIME types.
2. Update the dialog description and unsupported-file message so they list
   Excel workbooks and PowerPoint presentations alongside the existing
   formats. Browser selection remains a usability filter; the server remains
   authoritative.
3. Preserve multiple selection, independent process-file jobs, retry,
   cancellation/cleanup, disabled-close behavior while processing, focus
   restoration, and accessible live updates. No new UI workflow or custom
   viewer is needed: after data refresh, the normal stored-document selection
   calls the existing PDF endpoint and `DocumentPreviewPanel` renders it.

The web and desktop adapters already send opaque `File` bytes to the same
multipart routes. Keep their public TypeScript API unchanged. Add coverage
that verifies XLSX/PPTX names and MIME fallbacks survive both adapter mappings
instead of adding a format-specific Tauri command. The Tauri relay's filename,
base64, request-size, and MIME-syntax checks remain in force.

## Tests and verification

### Backend parser and ingestion tests

1. Extend `backend/tests/unit/domains/documents/formats/spreadsheet_tests.rs`
   with a minimal in-memory XLSX package exercising multiple sheets, rows,
   cell data variants, byte parsing, deterministic byte hash/size, no local
   upload path, chunk offsets, and malformed/empty/archive-limit failures.
2. Extend `.../powerpoint_tests.rs` with a minimal PPTX package containing
   ordered text, a list/table, and multiple slides. Assert byte parsing,
   cleanup of the private staging file, source metadata, slide boundaries and
   page-number ranges, offsets/chunk identity, and invalid/empty/unsafe OOXML
   rejection. Generate compact OpenXML archives in test helpers with the
   existing `zip` crate or add sanitized fixture bytes only when generation is
   impractical; do not use real customer files.
3. Extend `.../formats/tests.rs` to prove `QuarryFile::from_bytes` selects
   XLSX/PPTX case-insensitively, retains the multipart filename without a
   local path, and rejects `.xls`, `.ppt`, and arbitrary extensions.
4. Extend `backend/tests/unit/domains/documents/ingestion/service_tests.rs`
   and `.../persistence_tests.rs` to prove new assemblies reach the existing
   embedding/persistence preparation path, preserve byte-hash idempotency,
   derive canonical XLSX/PPTX MIME types, and fail before an embedding call
   when parsing fails. Add a text-dense synthetic Office assembly with final
   embeddings that crosses the actual v3 serialized-query cap; assert the
   same `insert_file_version_graph` preflight rejects it before
   `DocumentStore::persist`, without splitting it or attempting a Helix write.
   Keep a near-limit case to prove ordinary Office graph writes retain the
   normal atomic file/version/chunk structure. Assert that no parser performs
   blocking work on the async executor.
5. Extend `backend/tests/integration/http_tests.rs` multipart boundary cases
   for accepted XLSX/PPTX names, rejected legacy names, and existing body/user
   validations. Confirm both `/api/v1` and temporary `/api` compatibility
   mounts remain intentional.
6. Extend `backend/tests/unit/domains/documents/viewing/service_tests.rs` to
   exercise the existing Office-preview branch for XLSX/PPTX through a
   deterministic converter closure: assert the requested extension, returned
   PDF validation, and cache behavior. A real LibreOffice conversion remains
   an authorized local integration/manual check, not a normal Rust test.
7. Do not change the Helix v3 graph/query builders, client, or index DDL for
   this feature. Their existing unit coverage remains the authority for
   `QueryRequest` construction, 13-index readiness, terminal write failures,
   response mapping, the OpenAI-request dimension coupling, and the
   conservative body limit. If implementation necessarily touches those seams,
   add the corresponding v3 builder/mapper/configuration tests and run the
   ignored compatibility test only against an explicitly supplied disposable
   `QUARRY_HELIX_COMPAT_URL`; never point it at the normal local container.

### Frontend and transport tests

1. Update `frontend/tests/components/data-room/UploadFilesModal.test.tsx` to
   select `.xlsx` and `.pptx`, assert they render as selectable entries, and
   assert unsupported/legacy files receive visible errors without disturbing
   existing image/PDF/DOCX behavior.
2. Update `frontend/tests/api/httpQuarryApi.test.ts` and
   `frontend/tests/api/tauriQuarryApi.test.ts` to prove both files use the
   existing deal-scoped process-file endpoints, preserve exact filenames, and
   send the expected bytes/MIME through `FormData` and the Tauri multipart
   payload. Keep the PDF response assertion unchanged: the viewer consumes
   `application/pdf`, not the Office MIME type.
3. Manually inspect web and desktop Data Room upload/preview with sanitized
   small XLSX and PPTX samples and a configured LibreOffice executable:
   selection, upload job processing/completion, document-list refresh,
   rendered PDF in the EmbedPDF viewer, error state when Office conversion is
   unavailable, keyboard/focus behavior, both themes, and a narrow viewport.

### Required gates after implementation

From `backend/`, run focused parser/ingestion/viewing/HTTP tests first, then:

```sh
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

From `frontend/`, run the focused upload/adaptor tests, then:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Run the Tauri Rust gate only if the native relay changes; the proposed design
does not require a native contract change. Do not use `cargo run`, mutate
`backend/data/`, or call real OpenAI/Helix/LibreOffice as routine verification.
`cargo test --locked --all-targets` leaves the opt-in Helix compatibility test
ignored. Report that separately if an explicitly supplied disposable runtime
is used; its URL must be distinct from the normal `HELIX_URL` and the test
must not create, reset, or reuse a developer's default container.

## Documentation and rollout

Update `docs/ARCHITECTURE.md` in the implementation change, specifically:

1. Expand the Data Room feature table and section 10.2 sequence narrative
   from PDF/DOCX/image to PDF/DOCX/XLSX/PPTX/image, distinguishing text-only
   Office parsing from image-description ingestion.
2. Replace the statement that spreadsheet/PowerPoint parsers are isolated.
3. Update section 10.3 to say stored XLSX/PPTX previews reuse the existing
   on-demand Office conversion/cache and require LibreOffice, while raw-text
   endpoint scope remains PDF/DOCX if it is not expanded.
4. Update the API/multipart supported-format descriptions and test-coverage
   inventory. Document that no schema migration or durable PDF derivative is
   introduced, and retain the known absence of live LibreOffice integration
   tests.
5. Preserve the branch's Helix v3 architecture text: `HELIX_URL` remains an
   origin, SDK request routing remains `/v2/query`-owned, index readiness is a
   bootstrap precondition, the named digest-pinned local container starts
   before health polling, and writes are not retried after an unknown transport
   outcome. Document that `HELIX_VECTOR_DIMENSION` configures both the vector
   index and OpenAI embeddings request. Add the Office graph-request preflight
   and its pre-persistence rejection behavior to the ingestion/recovery
   narrative; do not imply that it changes Helix runtime selection, index
   definitions, or the graph schema.

Before handoff, inspect `git diff --check`, verify only scoped code/tests/docs
changed, and rerun `git status --short`. Report the Office-converter manual
check separately from automated tests and call out whether `QUARRY_SOFFICE`
was available.

## Acceptance criteria

- A valid `.xlsx` or `.pptx` selected in the Data Room UI is accepted by both
  web and desktop transports, becomes a normal per-file ingestion job, and
  reaches completed/skipped/failed states through the unchanged SSE contract.
- The parser consumes upload bytes without a caller-provided local path;
  canonical hashes/sizes identify the original bytes, and every chunk has
  valid identity, offsets, token count, and embeddings before persistence.
- The original XLSX/PPTX blob, correct MIME type, current version, and Helix
  chunks are persisted through the same idempotency/recovery rules as PDF and
  DOCX. No new schema or alternate storage path is introduced.
- Every Office graph request is constructed through the existing Helix v3
  writer with embeddings generated using the same configured dimension and
  remains under its 2-MiB atomic-query limit. A request that would exceed that
  limit fails before the SQLite write; it is neither split nor retried as an
  uncertain graph write.
- Stored XLSX/PPTX requests to the existing PDF endpoint produce a validated
  `application/pdf` response and render in the existing PDF viewer when
  LibreOffice is configured. A conversion failure leaves the uploaded source
  intact and reports a clear, safe error.
- Invalid, empty, mislabeled, archive-bomb, legacy `.xls`/`.ppt`, and
  unsupported uploads fail safely; no parser panics, no unbounded work runs on
  Tokio workers, and no temporary PowerPoint staging file is retained.
- Existing PDF/DOCX/image ingestion, frontend runtime boundaries, `/api/v1`
  contract behavior, Helix v3 index-readiness/terminal-write semantics, size
  limits, and security checks continue to pass their focused and broad
  verification gates.
