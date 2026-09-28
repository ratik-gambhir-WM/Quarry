# Spreadsheet and PowerPoint ingestion with PDF preview plan

## Goal

Enable the existing document-ingestion workflow for Excel workbooks (`.xls`,
`.xlsx`) and modern PowerPoint presentations (`.pptx`): accept them in the Data
Room upload dialog, extract deterministic searchable text, chunk and embed it,
persist the original bytes in SQLite, project the chunks into Helix, and display
the saved document through the existing PDF editor.

The original Office file remains the canonical stored blob. Quarry will render a
PDF only when the viewer requests `/deals/{deal_id}/documents/{file_id}/pdf`,
where the existing bounded in-memory preview cache can retain it. Do **not** add
a second durable PDF blob, a migration, or a separate preview API: doing so
would duplicate data ownership and change the current SQLite/Helix recovery
model without a product need.

## Scope and decisions

| Input | Ingestion | PDF viewer | Raw-text pane | Decision |
| --- | --- | --- | --- | --- |
| `.xls` | Yes | LibreOffice first; extracted-text PDF fallback | Yes | `calamine` and the existing Office MIME map support it. |
| `.xlsx` | Yes | LibreOffice first; extracted-text PDF fallback | Yes | Primary spreadsheet format. |
| `.pptx` | Yes | LibreOffice first; extracted-text PDF fallback | Yes | `pptx-to-md` is already the selected parser. |
| `.ppt` | No | Already convertible for a separately stored blob, but not ingestible | No new support | `powerpoint.rs` explicitly rejects legacy binary PowerPoint; do not advertise or accept it until there is a real parser. |
| CSV, XLSB, ODS, XLSM, DOC, and image OCR inside Office files | No change | No change | No new support | Keep this implementation limited to the formats the product explicitly accepts. |

Assumptions encoded in this plan:

- The request means the shared Data Room uploader and saved-document viewer,
  not the separate Diligence Studio PowerPoint-template import workflow.
- A successful Office upload must remain searchable even if LibreOffice is not
  installed. In that condition the viewer will show a generated text PDF; a
  configured `QUARRY_SOFFICE` produces the faithful workbook/slide rendering.
- A valid document with no extractable text follows the current ingestion
  behavior for empty PDFs: it is stored and previewable, reports zero chunks,
  and makes no embedding request. Malformed or unsupported Office packages fail
  before SQLite or Helix persistence.

## Current-state findings

- [`backend/src/domains/documents/formats/spreadsheet.rs`](../backend/src/domains/documents/formats/spreadsheet.rs)
  already reads worksheets with `calamine`, including sheet and row context,
  but only accepts a filesystem path and returns a `String`.
- [`backend/src/domains/documents/formats/powerpoint.rs`](../backend/src/domains/documents/formats/powerpoint.rs)
  already extracts text, lists, and tables in visual order, but only accepts a
  `.pptx` path and returns a `String`. Its upstream `PptxContainer` only opens
  `std::fs::File`, so an upload requires safe temporary staging.
- [`backend/src/domains/documents/formats/mod.rs`](../backend/src/domains/documents/formats/mod.rs)
  dispatches only PDF, DOCX, and image variants. Consequently
  [`backend/src/domains/documents/ingestion/service.rs`](../backend/src/domains/documents/ingestion/service.rs)
  never sees spreadsheet or presentation assemblies to embed and persist.
- [`backend/src/domains/documents/ingestion/handler.rs`](../backend/src/domains/documents/ingestion/handler.rs)
  and [`frontend/src/components/data-room/UploadFilesModal.tsx`](../frontend/src/components/data-room/UploadFilesModal.tsx)
  independently allow only PDF, DOCX, and images.
- The existing shared MIME policy already maps `xls`, `xlsx`, and `pptx`, and
  [`backend/src/domains/documents/viewing/service.rs`](../backend/src/domains/documents/viewing/service.rs)
  already recognizes their Office MIME types for on-demand PDF conversion. The
  existing web and Tauri adapters already send generic multipart files and use
  the same stored-document PDF endpoint; no new endpoint or Tauri command is
  needed.

## Stable contracts and invariants

The implementation must preserve these properties:

1. Client requests remain on `/api/v1`. Multipart field names (`userId`,
   `files`), 50 MB per-file/request limits, job events, current response
   shapes, and the desktop relay's path/MIME/size validation stay unchanged.
2. The handler validates the filename extension. The parser validates actual
   workbook/presentation structure rather than trusting browser-provided MIME.
   MIME persisted in SQLite continues to be inferred from the validated
   filename, not from the multipart header.
3. Parsing creates a new `Document` whose `document_id` is content-hash plus
   user scoped, whose `file_id` is new unless exact-content deduplication finds
   an existing attachment, whose `source_type` exactly equals `xls`, `xlsx`, or
   `pptx`, and whose `local_path` is `None` for browser uploads.
4. Chunk text, UTF-8 byte offsets, token counts, content hashes, and chunk IDs
   stay deterministic. Embeddings use the existing OpenAI batch call; SQLite
   commits before Helix just as PDF/DOCX ingestion does.
5. Parsing, temporary-file writes, ZIP/XML work, and Office conversion run on
   a blocking worker. No new mutex is held across an await, and temporary input
   paths, document contents, or provider credentials are logged.
6. The PDF endpoint continues to return `application/pdf`, `inline`, and
   `Cache-Control: private, no-store`; the desktop `quarry_api_get_pdf` command
   remains the only native route that returns preview bytes.

## Implementation steps

### 1. Turn extracted Office text into graph-ready assemblies

1. Add a narrowly scoped shared helper under
   `backend/src/domains/documents/formats/` (for example,
   `office_text.rs`) for the two new parsers. It should construct a
   `Document` and `Vec<DocumentChunk>` from normalized extracted text, upload
   filename, source type, byte length, content hash, user ID, and optional
   source-range metadata. Reuse `token_bounded_ranges`,
   `document_id_from_content`, SHA-256 IDs, and the existing `Document`/
   `DocumentChunk` shapes; do not refactor the working PDF or DOCX parser just
   to share code.
2. Give the helper a small assembly type used by both formats. It must create
   `rendered_pdf_path: None`, retain no temporary path, sum chunk token counts,
   and use `None` for spreadsheet page numbers. Validate all `usize`/`u32`
   conversions with errors rather than unchecked casts or new `unwrap`s.
3. Preserve the spreadsheet parser's high-value context in its canonical text:
   each nonblank row remains `sheet name + row number + tab-separated cells`.
   Add `parse_spreadsheet_from_bytes` using
   `calamine::open_workbook_auto_from_rs(Cursor<Vec<u8>>)` and build its
   assembly from those exact bytes. Restrict dispatcher entry to `.xls` and
   `.xlsx` even though Calamine can recognize more formats.
4. Split PowerPoint parsing into byte/path-independent slide extraction and
   assembly construction. Preserve the current visual ordering of text, lists,
   and tables; add an explicit `Slide N` boundary to normalized text so every
   embedding has useful context. Retain slide byte ranges and attach the
   overlapping one-based slide number(s) to `DocumentChunk.page_numbers`, since
   a normal PPTX-to-PDF conversion makes slides navigable pages. Do not claim
   page locations for spreadsheets.
5. Because `pptx-to-md` 0.4.0 only accepts a path, add the smallest direct
   runtime dependency needed for RAII temporary files (for example,
   `tempfile`), update `backend/Cargo.lock`, and stage only the already
   size-limited upload to a uniquely named `.pptx` temporary file. Flush it,
   parse it, and let RAII remove it on every return path. Keep the stage/parse
   operation inside `spawn_blocking`; do not reuse the Office converter's
   temporary-directory implementation across domain layers.
6. Validate a PPTX is structurally a presentation package before extraction
   (including its presentation entry) and map malformed ZIP/XML/parser errors
   to concise, filename-scoped messages. A generic ZIP renamed to `.pptx` must
   not be accepted. Calamine parse failures must receive the same treatment.

### 2. Connect parsers to ingestion without changing the transport contract

1. Extend `QuarryFile` and `ParsedQuarryFile` in
   `backend/src/domains/documents/formats/mod.rs` with spreadsheet and
   PowerPoint variants. Accept `xls`, `xlsx`, and `pptx` case-insensitively in
   `from_parts`; preserve the existing rejection of `.ppt` and every other
   unlisted extension.
2. In `QuarryFile::parse`, move spreadsheet and presentation bytes into a
   blocking task, construct their graph-ready assemblies, and restore the
   exact browser filename. Convert join failures into normal parser failures.
   These formats do not invoke image description; retain the existing OpenAI
   path only for image inputs.
3. Extend `parse_document` in
   `backend/src/domains/documents/ingestion/service.rs` to unwrap the two new
   parsed variants into the unchanged embedding and persistence flow. Existing
   exact-content deduplication, per-deal locking, embedding batching, SQLite
   transaction, and Helix projection must apply identically.
4. Expand `validate_document_upload_filename` and its user-facing error in
   `backend/src/domains/documents/ingestion/handler.rs` to allow exactly XLS,
   XLSX, and PPTX alongside today's files. Keep body limits, filename-whitespace
   checks, multipart parsing, and path-only deal ID semantics intact.
5. Recheck `backend/src/shared/file_policy.rs` rather than duplicating MIME
   definitions: it already maps these three extensions and its Office preview
   mapping already covers their MIME types. Only alter it if implementation
   exposes a genuine mismatch; do not expand the accepted ingestion set to
   legacy `.ppt` merely because the preview converter can handle it.

### 3. Make saved Office documents reliably viewable as PDFs and raw text

1. Keep `StoredDocumentService::render_pdf` and the existing
   `/deals/{deal_id}/documents/{file_id}/pdf` route as the sole PDF-preview
   path. It already selects an Office extension from the persisted MIME type,
   limits concurrent conversions to two, validates the produced PDF, and caches
   it by MIME/name/content bytes.
2. Extend `render_office_bytes_as_pdf` in
   `backend/src/domains/documents/viewing/service.rs` so XLS, XLSX, and PPTX
   fall back to `render_text_as_pdf` when `OfficeConverter` cannot produce a
   PDF. Use the same canonical byte parsers introduced above, not a second
   extraction implementation. Generalize DOCX-specific fallback messages and
   PDF metadata to say Office document. This gives the viewer a valid PDF for
   text-bearing saved documents without `QUARRY_SOFFICE`, while configured
   LibreOffice remains the first, layout-faithful result.
3. Extend `render_stored_document_as_text` to dispatch the persisted Excel and
   PowerPoint MIME types to those same byte parsers and return source kinds
   `xls`, `xlsx`, and `pptx`. This ensures the raw-text panel and the embedded
   chunks use identical canonical text. Keep the existing 64 MB read cap and
   return a visible validation error for textless/malformed files rather than
   an empty success response from this optional viewer feature.
4. Update the handwritten TypeScript `DealDocumentText.sourceKind` union in
   `frontend/src/contracts/quarryApi.ts`. The web and Tauri adapters keep their
   current generic JSON/PDF mappings; update their contract tests to cover the
   widened response instead of creating platform-specific conversion paths.
5. The data-room explorer already classifies saved `.xls`/`.xlsx` files as a
   sheet and all other extensions (including `.pptx`) as a document. Keep that
   mapping; it will automatically route a saved Office file through
   `useDocumentSession` to the PDF editor once ingestion succeeds.

### 4. Update the shared upload UI

1. In `frontend/src/components/data-room/UploadFilesModal.tsx`, extend the
   extension set, native file-input `accept` list, and visible helper text to
   say PDF, DOCX, XLS, XLSX, PPTX, PNG, JPEG, WebP, and GIF. Include the three
   Office MIME values as well as extensions, because browsers vary in how they
   apply `accept`.
2. Retain client-side empty/50 MB/duplicate checks as early feedback, but treat
   server validation and parser failures as authoritative. Preserve the current
   per-file job/SSE progress, retry, close blocking, and focus behavior.
3. Render the existing sheet icon for spreadsheet upload rows; keep the current
   document icon for PPTX. The page already reloads stored documents when the
   modal closes, so no new refresh state or route behavior is necessary.

### 5. Update the maintained architecture narrative

After implementation, update `docs/ARCHITECTURE.md` in the same change:

- API table: supported formats for both ingestion routes, stored PDF preview,
  and canonical raw-text endpoint;
- document-ingestion sequence and prose: spreadsheet/PowerPoint parser
  assemblies, unchanged embed/persist order, and actual supported extensions;
- stored-preview flow: LibreOffice-first rendering plus text-PDF fallback for
  stored Office inputs, bounded cache/concurrency, and `QUARRY_SOFFICE` fidelity
  implication;
- frontend feature/contract sections and verification coverage; and
- known gaps: legacy `.ppt` remains outside ingestion and generated previews
  stay in-memory rather than durable.

## Test plan

### Backend focused coverage

1. Extend `backend/tests/unit/domains/documents/formats/spreadsheet_tests.rs`
   with compact, source-controlled/generated-in-test workbook bytes. Assert
   in-memory parsing preserves sheet/row/cell context, malformed bytes fail,
   `xls` and `xlsx` source types are correct, uploads have no `local_path`, and
   generated chunks have contiguous UTF-8 ranges, bounded tokens, stable
   hashes, and correct size/content identity.
2. Extend `backend/tests/unit/domains/documents/formats/powerpoint_tests.rs`
   with a minimal valid PPTX package and malformed/renamed-ZIP cases. Assert
   visual text ordering, explicit slide context, stable slide/page ranges,
   temporary-file cleanup on success and parser failure, `.pptx` acceptance,
   and `.ppt` rejection.
3. Extend `backend/tests/unit/domains/documents/formats/tests.rs` to prove the
   dispatcher chooses each new variant from uploaded bytes and yields a
   graph-ready, browser-filename-preserving assembly. Add ingestion/persistence
   tests that each source type resolves to the existing correct MIME, embeds
   only nonempty chunk content, and retains current duplicate/hash behavior.
4. Add handler/route tests for accepted XLS/XLSX/PPTX filenames and preserved
   rejection of `.ppt`, CSV, blank names, oversized files, and disallowed path
   forms. Ensure parser failures become failed document-job results before any
   SQLite/Helix write.
5. Extend
   `backend/tests/unit/domains/documents/viewing/service_tests.rs` to assert
   raw text for every new MIME and valid fallback PDF bytes when the converter
   closure fails. Add an HTTP integration case with persisted XLSX/PPTX fixture
   blobs that verifies list, text `sourceKind`, PDF headers/signature, and
   cross-deal isolation without requiring a live LibreOffice executable.
6. Preserve and run architecture-boundary tests: parsing stays below handlers,
   persistence stays through `DocumentStore`/index writer, services never read
   ambient configuration, and no blocking Office/parser work runs directly on
   a Tokio worker.

### Frontend and desktop coverage

1. Extend
   `frontend/tests/components/data-room/UploadFilesModal.test.tsx` to verify
   XLS, XLSX, and PPTX appear in `accept`, are selectable (including
   mixed-case filenames), show the sheet/document icon, and leave `.ppt` and
   unsupported files visibly rejected.
2. Extend `frontend/tests/data/dataRoom.test.ts`,
   `frontend/tests/hooks/useDocumentSession.test.tsx`, and
   `frontend/tests/components/data-room/DocumentPreviewPanel.test.tsx` with
   saved spreadsheet/presentation nodes. Verify they call the existing PDF
   route, hand validated PDF bytes to the existing editor, and show canonical
   raw text/source kind or a visible error without stale state leaks.
3. Extend HTTP and Tauri adapter tests to send each Office MIME type through
   the existing multipart shape and accept `xls`/`xlsx`/`pptx` raw-text
   responses. Keep the desktop Rust tests focused on the unchanged generic
   multipart and exact PDF-route guard; no new IPC command is expected.
4. Manually inspect web and desktop Data Room flows using disposable sample
   XLSX and PPTX files: chooser/drag-drop, upload progress and failure state,
   stored-list refresh, PDF preview, raw-text action, keyboard/focus behavior,
   relevant viewport, and both themes. When LibreOffice is available, verify
   the faithful preview; when it is unavailable, verify the labeled text-PDF
   fallback. Do not use production documents or a real SQLite database.

## Verification and handoff

Run the narrow tests first, then the relevant full gates:

```sh
cd backend
cargo test spreadsheet
cargo test powerpoint
cargo test stored_document
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

```sh
cd frontend
npm test -- tests/components/data-room/UploadFilesModal.test.tsx
npm test -- tests/hooks/useDocumentSession.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts
npm test -- tests/api/tauriQuarryApi.test.ts
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

```sh
cd frontend/src-tauri
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Do not use `cargo run` or `clear_helix` as verification: normal backend startup
opens/migrates SQLite and needs Helix. A real LibreOffice smoke test is optional
and must use an explicitly disposable configuration; unit tests must prove the
fallback path without starting external processes. Before handoff, inspect the
final diff, run `git diff --check`, re-run `git status --short`, and separately
report any pre-existing user changes.

## Acceptance criteria

- The Data Room accepts `.xls`, `.xlsx`, and `.pptx` in web and desktop builds,
  rejects legacy `.ppt`, and reports unsupported/malformed files clearly.
- Successful files flow through the same parse → chunk → embed → SQLite blob →
  Helix projection path as DOCX/PDF, with deterministic identity and no
  document-local server path persisted.
- Saved workbook and presentation entries open in the current PDF editor by
  calling the existing stored-document PDF endpoint. A layout-faithful Office
  conversion is used when available; a valid, canonical-text PDF fallback
  remains viewable otherwise.
- The raw-text pane, embeddings, and fallback renderer derive from the same
  format-specific canonical text.
- No new public route, Vite secret, Tauri escape hatch, SQLite schema, durable
  preview blob, or unsupported legacy-format promise is introduced.
