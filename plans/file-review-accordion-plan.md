# File review table with expandable rows: implementation plan

Date: 2026-11-01. Status: proposed; no product code or component installation is performed by this plan.

## Outcome and chosen component

Keep **File review** as a table. Replace the five review-text columns with one compact file row
containing **File name**, **File type**, **Size**, and **Scope area**, plus an expansion control.
Expanding a row reveals its full key findings, risks, opportunities, and key question impact in a
detail row immediately beneath that file. Scope area displays **N/A** for every file for now.

Use [Stealth Expandable Rows](https://www.stealth.pm/ui/expandable-rows), requested via:

```sh
npx shadcn@latest add @stealth/expandable-rows
```

This supersedes the earlier accordion/list design. Keep the existing `FileReviewTable` component
name and its Data Room placement. Do not rename it to `FileReviewList`, replace table semantics with
standalone disclosures, or enable/redesign the separate Deal Room Findings or File Summary tabs.

## Current implementation and source findings

- `frontend/src/components/data-room/FileReviewTable.tsx` receives `files` and `onSelectFile`,
  builds rows, and owns search, status/type filters, sorting, pagination, and column pinning. It
  currently renders a ReUI/TanStack grid with truncated review text. Filename clicks open a preview.
- `frontend/src/data/fileReview.ts` defines `FileReviewRow` and the four review text fields, three
  signal levels, and review status. The original `DataRoomTreeNode` is preserved for preview routing.
- `frontend/src/fixtures/data-room/fileReview.ts` supplies illustrative summaries. The mapper cycles
  through them by input index; they are not document-specific AI analysis returned by the server.
  Build these rows before filtering/sorting, so table operations do not reassign fixture summaries.
- `frontend/src/components/data-room/DataRoomWorkspace.tsx` unmounts the review table while displaying
  `DocumentPreviewPanel`. `useDocumentSession` owns preview requests and error handling.
- `frontend/src/hooks/useDataRoomContents.ts` merges local and stored nodes into `reviewFiles`.
  Neither `DataRoomTreeNode` nor the TypeScript `DealDocumentSummary` currently carries file size.
  Stored document versions already retain canonical `byte_size` in SQLite; the list DTO does not
  currently expose it. Do not mistake uploaded-file bytes or rendered preview bytes for a complete,
  reload-safe listing-size contract.
- The Stealth registry definition at `https://www.stealth.pm/r/expandable-rows.json` exposes
  `ExpandableRows<T>`, `ExpandableColumn<T>`, `columns`, `rows`, `getRowId`, `getRowLabel`,
  `renderDetail`, controlled `expanded`/`onExpandedChange`, `single`, `rowClickExpands`, `caption`,
  `empty`, `size`, and `minWidth`. It renders a real table and a second `tr` with a spanning detail
  cell for each expanded row. It also supplies expand-all, arrow-key toggles, overflow cues, and
  reduced-motion-aware detail animation.

## Table layout

| Visible column | Data and presentation | Sorting |
| --- | --- | --- |
| Expansion control | Chevron button named “Show/Hide details for [filename]” | Not sortable; header offers expand/collapse visible rows |
| File name | Original filename with file icon; folder/source and stored uploader attribution as secondary metadata | Filename, case-insensitive |
| File type | Existing `PDF`, `Document`, or `Spreadsheet` classification | Type label |
| Size | Original file byte count formatted consistently as B, KB, MB, or GB; unknown shows “Size unavailable” | Numeric bytes; unknown last in both directions |
| Scope area | Literal `N/A` for every file | Disabled until real classification exists |

Keep full filenames available through wrapping and accessible names. Secondary uploader information
applies only to stored files and retains `Unknown` for absent legacy attribution. Do not add invented
upload dates, severity counts, or classification. Review status remains available through the current
status filters and in the detail panel, rather than becoming another main column.

Use one unit convention consistently, with a reusable formatter and an exact byte count available
in an accessible description/title. Treat zero bytes as a known size, not missing. Reject invalid,
negative, non-finite, or unsafe numeric values at the appropriate boundary; represent unavailable
size explicitly rather than coercing it to zero.

### Expansion interaction

- Start collapsed; allow multiple files open (`single={false}`) for comparison.
- Use `rowClickExpands` so clicking non-interactive row content expands/collapses it. Native toggle
  buttons remain the keyboard path. Clicking a file row no longer opens the document.
- Preserve native controls/links inside cells: they perform only their own action, with no second
  row toggle. Selecting/copying text should not expand a row.
- Expand/collapse all operates on the currently filtered, paginated rows. Adapt the source helper
  to union/remove those IDs against controlled expansion state; it must not erase expansion choices
  for other pages or temporarily filtered files.
- Keep expansion separate from preview, document selection, analysis jobs, and status changes.

## Expanded detail row

Render a single cell spanning all five visible table columns, including the expansion-control
column. Inside it, use four readable sections with wrapping text, two columns at wide content
widths and one column on narrow screens:

| Section | Existing fields |
| --- | --- |
| Key findings | `keyFinding` |
| Risks | `risks`, `riskLevel` |
| Opportunities | `opportunities`, `opportunityLevel` |
| Key question impact | `impact`, `impactLevel` |

Keep this reading order and include the file's review status. Show text labels alongside signal
colors; `none` means **Not assessed**, not low risk. Preserve pending/unavailable wording instead
of inventing a reassuring conclusion. Scope area remains **N/A**; do not infer it from the filename,
folder, findings, or another deal's scope. Render plain text safely, without untrusted HTML or
fabricated citations.

Provide **Open document** in the expanded panel. It calls `onSelectFile(row.node)` exactly once with
the original node. Reuse existing local/stored PDF, Office, image, authorization, and preview-error
handling. No new document download, analysis request, or content fetch happens just to expand a row.
Stealth calls `renderDetail` for all supplied rows, so keep that renderer pure and cheap.

## File size data work

1. Add a nullable/optional byte-size field to the shared file metadata and carry it into
   `FileReviewRow`. Keep it distinct from review summaries and scope classification.
2. For stored files, expose the current version's existing canonical SQLite `byte_size` in the
   authenticated document-list response. Follow the current version/live-file joins and ownership
   checks; do not read BLOB contents, inspect rendered preview size, or issue a request per row.
3. Coordinate the additive camelCase DTO field across Axum, `contracts/quarryApi.ts`, the web and
   desktop TypeScript adapters, and the Tauri relay DTO/mapping where it participates. Extend
   `buildStoredDocumentNodes` to preserve it. Verify old/legacy absence remains a truthful unknown.
4. For local desktop files, use filesystem metadata from the existing authorized tree traversal and
   pass it through the typed native/runtime tree contract. Do not add browser filesystem reads,
   broader Tauri permissions, arbitrary stat commands, or document-content reads to calculate size.
5. Use “Size unavailable” only where metadata is genuinely absent or inaccessible. Actual stored
   and authorized local-file sizes are part of this implementation's completion target; do not ship
   every size as a fabricated number or silently leave all sizes as placeholders.
6. No schema migration should be needed for stored bytes already present. Confirm this against the
   current schema and queries before implementation. Keep scope-area persistence/classification
   outside this change; its temporary `N/A` is deliberate.

## Search, filters, sorting, and pagination

- Preserve the existing search expand/clear/Escape behavior, accessible labels, and focus return.
  Search filenames, folders, and all four review fields, including details in collapsed rows.
- Retain **All**, **Needs attention**, **Reviewed**, and **Pending** and the current file-type filter.
  Filters compose; Reset clears filters and query. No new file taxonomy is required.
- Use sortable table headers for File name, File type, and Size, with `aria-sort` and stable-ID
  tie-breaking. Default to existing input order. Scope area has no sort affordance while all
  values are `N/A`.
- The old review-text columns move into details. Preserve their existing text-based sorting through
  a compact **Sort by** control with direction when needed; it shares one controlled sort state
  with the visible column headers. Do not silently substitute severity for text sorting.
- Build rows, filter, sort, then paginate. Keep sizes 5, 10, 25, and 50, matching-file counts, and
  previous/next controls. Expanded detail rows do not count as files or consume pagination slots.
- Reset to page one on query/filter/sort/page-size changes and clamp the displayed page when files
  disappear. Remove feature-specific column-pinning state/menus unless a proven need remains; do
  not modify shared ReUI grids used by other screens.
- Stealth's component does not itself supply sorting/filtering/pagination. Keep that logic in
  focused selectors/state outside the primitive; do not assume the install command replaces it.

## Component adoption and state ownership

During implementation, stage and inspect the requested shadcn command in an isolated checkout
before adopting files into the active tree. Its registry dependencies include tokens, `cn`, motion
helpers, and a controllable-state helper; `motion` is already installed. Adapt the selected
component under `frontend/src/components/ui/stealth/expandable-rows.tsx` using Quarry's existing
`cn`, motion tokens, semantic theme variables, and controlled-state conventions. Do not overwrite
project-wide styles/helpers or change dependencies/lockfiles unnecessarily. The current source uses
loose row access and effect-based measurement; tighten types and preserve the rule against effect-body
state setters while keeping ResizeObserver updates in its callback.

Keep `FileReviewTable.tsx` as the feature owner, composing `ExpandableRows<FileReviewRow>` and a
focused `FileReviewDetails.tsx` for expanded content. Retain its existing `files`/`onSelectFile`
consumer contract where practical. Replace this feature's grid rendering without a duplicate table
or an accordion wrapper. Keep domain types/selectors in `src/data/` and runtime samples in
`src/fixtures/`.

Control expansion with `getRowId={(row) => row.id}`, `getRowLabel` returning the filename, and
`expanded`/`onExpandedChange` from a deal-scoped state owner at the `DataRoomWorkspace` lifetime.
That owner also retains query, filters, sorting, pagination, preview-origin ID, and scroll position
while the table unmounts for preview. Do not key rows by position or persist UI state to localStorage.

Preserve known file expansion across sorting/filtering/pages; newly uploaded files start collapsed.
Ignore removed/unauthorized IDs and prune through normal events/resource transitions or derived
valid IDs, without effect-body state setters. Reset the owner when deal identity changes.

On preview close, restore the table's context and focus its originating Open document button once
remounted. Use an explicit callback-ref/restoration path. If the file disappeared or is no longer
visible/authorized, focus the File review heading instead. Never revive an inaccessible file to
restore selection. Preview failure must not erase table state.

## Data truth, styling, and accessibility

Keep real file metadata, sizes, and uploader information distinct from illustrative review content.
Add a visible **Illustrative review content** explanation and identify expanded sample analysis as
illustrative. Update the current test expecting no such label. A real findings API, scope extraction,
and persisted review-status editing remain outside this plan.

Preserve parent-owned loading/unavailable/error/retry/upload/empty-room states and safely handle an
empty `files` input. Distinguish **No files yet** from **No file reviews match these filters**, with
Reset for the latter. Do not retain details for a file removed from authorized input.

Match Quarry's existing navy/neutral colors and Data Room surface. Adapt Stealth's frame, raised,
border, hover, and overlay tokens rather than importing a parallel palette. Use restrained separators
and the current muted-to-transparent, 8-pixel-corner card treatment where appropriate. Avoid nested
large cards for each review section.

Retain real `table`, `thead`, `tbody`, `tr`, `th scope="col"`, and `td` structure and an accessible
caption. Expand controls have descriptive names, `aria-expanded`, and correctly linked panel IDs.
Enter/Space toggle; retain Stealth's ArrowRight-open/ArrowLeft-close behavior. Row click is a
convenience, not a substitute for keyboard controls. Avoid nested interactive elements.

Keep closing animated details inert/non-focusable and hidden from assistive technology; restore
focus to the row toggle if focus would otherwise remain inside a collapsed panel. Avoid excessive
region landmarks when many rows are expanded. Preserve reduced-motion behavior and hover gating,
with adequate touch targets. Use Stealth's single expansion-animation controller rather than adding
another collapsible/height animation around the table.

At narrow widths, retain all four metadata columns with a bounded horizontal table scroller instead
of shrinking text or overflowing the page. Keep the expanded details pinned/constrained to the
visible table width as the Stealth component supports, so reading findings does not require sideways
scrolling. Check long text, 200% zoom, sticky headers, vertical scroll ownership, and persistent
navigation insets; remove unexplained padding that would strand content below the viewport.

## Implementation sequence and verification

1. Inspect/adapt Stealth Expandable Rows; characterize its controlled expansion, visible-page
   expand-all, keyboard behavior, retained closing panels, and narrow-screen detail sizing.
2. Wire real size metadata through stored/local contracts and mapper tests before presenting Size
   as available. Add the four metadata columns and the explicit `N/A` scope placeholder.
3. Implement full detail content and independent Open document behavior, then connect the existing
   toolbar, shared sorting state, pagination, and deal-scoped preview-return state.
4. Remove obsolete review-column/pinning rendering from this feature. Update canonical
   `docs/ARCHITECTURE.md` for the table layout, added size DTO, state lifetime, and fixture limitation;
   this plan remains a design input, not canonical product documentation.
5. Inspect the Data Room using disposable/synthetic data on web and desktop where native metadata
   participates. Stop task-owned preview servers and verify their ports are free.

Extend existing `frontend/tests/components/data-room/FileReviewTable.test.tsx`,
`frontend/tests/data/fileReview.test.ts`, and `frontend/tests/pages/DataRoomPage.test.tsx`. Add focused
primitive tests and mapping/contract tests at the actual file-size transport boundaries. Cover:

- File name/type/size/scope columns, byte formatting including zero and unknown, and `N/A` scope.
- All four review sections, status and signal levels visible only in expanded details, without
  truncation, fake analysis requests, or fixture/metadata confusion.
- Multiple independent expansions, visible-page expand-all preserving other IDs, and stable state
  after reorder/filter/upload/deal switch.
- Header/toolbar sorting, numeric size ordering, unknown sizes last, text-review sorting, filter
  composition, page reset/clamping, and file counts independent of expanded detail rows.
- Keyboard controls, correct table relationships, collapsed/closing focus behavior, preview return,
  and the exact original local/stored node forwarded once to document preview.
- Live/current-version size mapping, legacy absence, local authorization, and no N+1 content reads.
- Empty/error/loading/retry paths, long filenames, unknown uploader, light/dark themes, reduced
  motion, 200% zoom, and mobile horizontal scrolling with readable fixed-width detail content.

From `frontend/`, run focused changed tests and then:

```sh
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Because real file-size metadata crosses runtime boundaries, also run the affected adapter/route
checks and repository-native backend and Tauri gates from their respective build roots:

```sh
# backend/
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets

# frontend/src-tauri/
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Use temporary databases/test transports. Do not start normal backend migrations, alter existing
local data, call AI providers, or broaden native permissions as a shortcut for the size column.
