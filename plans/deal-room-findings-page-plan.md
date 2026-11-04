# Deal Room Findings page implementation plan

Date: 2026-11-01. Status: proposed; this document does not implement the page.

## Outcome and scope

Enable the **Findings** tab in the Deal Room and render a page whose information hierarchy follows
the supplied screenshot: a heading and action, four summary cards, a search/filter/sort toolbar, a
findings table, and an adjacent selected-finding detail panel. Use Quarry's existing theme and
shadcn/Radix-based controls rather than copying the screenshot's colors, shadows, or button style.
Keep the existing Deal Room sidebar, tab header, dimensions and shared web/desktop navigation.

The screenshot is an illustrative layout reference, not evidence of actual findings or a request
to copy its customer-specific text. For this first implementation, use clearly labeled synthetic
illustrative data with functional local search, filters, sorting, selection and detail dismissal.
Keep **New finding** and **Edit finding** visible but disabled with an accessible explanation that
saving findings is not available yet. Do not simulate a successful save, link sample citations to
real documents, or imply the data belongs to the open deal. Durable creation/editing, evidence
linking, projects, and a findings API are future work requiring a separate product/data contract.

## Current implementation and constraints

- `frontend/src/components/deal-room/DealRoomHeader.tsx` declares the Findings tab but disables it;
  `getDealRoomSection` falls back to Overview for `?section=findings` today.
  `frontend/src/pages/DealRoomPage.tsx` owns query-parameter navigation and preserves router state.
  `frontend/src/pages/deal-room/DealRoomOverviewPage.tsx` renders tab content inside the persistent
  `WorkspaceMain` rail. Enable the existing query-backed tab instead of adding another URL scheme or
  replacing the surrounding shell.
- There is no product findings model, route, or durable store. `backend/src/domains/diligence/mod.rs`
  documents intended ownership only. The fixture-based file-review findings and slide-template
  finding retrieval are separate features; do not use them as if they were the current deal's
  findings. No backend, schema, Helix, Tauri IPC, or provider integration is needed for this UI slice.
- Quarry's shared React/Vite app serves both web and desktop. Reuse the existing semantic tokens,
  current Deal Room card treatment and components under `frontend/src/components/ui/`; use the
  vendored ReUI data-grid already used by the Deal Room and Data Room for the table where its behavior
  fits. `frontend/components.json` configures the existing Radix Nova shadcn setup. Do not add a
  parallel visual system, new dependencies, raw colors from the reference, or raw platform imports.
- Runtime sample records belong under `frontend/src/fixtures/deal-room/`; domain types, formatting,
  filtering and selectors belong under `frontend/src/data/`. Keep the fixture import explicit and
  removable. Do not copy actual customer content from the screenshot into fixtures or tests.

## Layout and components

The screenshot is approximately 1,444 × 1,032 pixels; its proportions are guidance rather than
exact CSS measurements. Compare against the same composition within Quarry's available main content
area, whose sidebar and existing tabs consume space. Preserve the current theme and responsive
spacing instead of reproducing a separate page shell.

1. **Page heading:** show “Findings”, concise supporting text, an always-visible “Illustrative data”
   badge and the disabled **New finding** control with a visible explanation readable without
   focusing the disabled button. Use existing heading, `Badge`, `Button` and description patterns.
   Do not imply a disabled control can create a record.
2. **Summary:** four equally weighted cards for Open findings, High / critical, Evidence pending,
   and Remediation estimate. Reuse `WorkspaceCard` or existing shadcn Card primitives with semantic
   tokens and the Deal Room's current styling. Derive the first three metrics from the complete
   illustrative data set, independent of filters, and label them as sample metrics. Define the last
   as the sum of known **one-time** estimates in one fixture currency, clearly label it “One-time
   remediation estimate,” and show an appropriate missing-value indicator when none are known;
   recurring estimates are not included. Use `Intl.NumberFormat` for counts and currency. Do not
   hard-code reference totals or silently treat unavailable amounts as zero.
3. **Controls:** provide a labeled search field and severity, category, status, owner and sort
   controls using existing `Input`/`SearchField`, `Select` or dropdown primitives. Search titles and
   descriptions case-insensitively. Filters compose, offer an “All” option, and sorting is stable
   with a defined tie-breaker. Start with recently updated, using explicit fixture timestamps;
   ensure an obvious way to clear filters and show a distinct no-results state. Use realistic sample
   categories and owners only from the fixture data.
4. **Table:** present Finding (title plus short description), Severity, Category, Evidence, Status,
   Owner and Estimated cost. Use Quarry's existing ReUI grid and `quarry-data-grid` conventions,
   badges and avatar primitives; keep selected, hover, focus and status states readable and preserve
   words alongside color. One-time and annual recurring amounts are separate, nullable values;
   format them consistently and show an em dash when unknown. Citation counts are illustrative text,
   not links to unavailable sources. Provide a semantic keyboard-operable button in each row to
   select a finding; preserve a real table structure and avoid invalid nested interactive controls.
5. **Detail panel:** beside the table at wide widths, show the selected finding title, severity,
   confirmation state, business impact, evidence labels, mitigation plan, associated project and
   one-time/recurring costs, in the same order as the reference. Use text and unobtrusive separators;
   missing fields say “Not provided” rather than fabricating content. Evidence and project labels
   are non-interactive until there is an authorized, real destination. Include a labeled close
   button and disabled **Edit finding** control with the same truthful explanation. No independent
   dialog is needed for the wide inline panel.
6. **Responsive behavior:** below the width where the table and detail can both remain legible,
   keep the list full width and show selected details using an existing accessible shadcn `Dialog`
   or an equally accessible stacked presentation. Allow horizontal table scrolling where needed,
   rather than shrinking text; let metric cards and controls wrap. For a dialog, move focus into it,
   support Escape and restore focus to the selected-row trigger. Avoid clipping behind the persistent
   header or mobile navigation. Check 200% zoom, narrow viewports, and reduced-motion behavior.

## State and interaction contract

- Create typed `Finding`/`FindingEvidence` models and pure selectors/formatters in a focused
  `frontend/src/data/` module. Model severity, workflow status, category, confirmation, owner,
  evidence, project and nullable monetary estimates as typed values. Fixtures use synthetic stable
  IDs and sample-only names. No user emails, real deal names, real evidence, or sensitive content.
- Keep query, filters, sorting and selected ID in the Findings view's narrow owner. Compute visible
  rows and summary totals from the source data rather than duplicating them in state or effects.
  Select the first result on initial view only; selecting a row updates the detail. Closing detail
  keeps it closed until the user selects a row again. If filters remove the current selection, close
  its detail rather than showing an out-of-filter record. Changing the deal clears local view state
  via a deal-keyed owner. Clearly document that view state is temporary and fixture data is not
  persisted; do not imply sample edits are saved.
- Set `enabled: true` for Findings and add a dedicated branch/component in
  `DealRoomOverviewPage`. Continue to use `?section=findings`, preserve unrelated search parameters
  and navigation state, and support direct entry, refresh, Back and Forward under both routers.
  Switching tabs must preserve the existing shell and access boundary. Evidence and Open Items remain
  disabled. Avoid an extra nested `<main>` or duplicated tab bar.
- An illustrative page has no fetch, so do not add a fake spinner, fake error, network endpoint or
  fixture fallback on a server failure. Include distinct fixture-empty and filter-empty states.
  When live findings are implemented later, define real loading/error/retry and access states then;
  do not quietly substitute sample findings for failed live requests.

## Implementation steps

1. Add synthetic fixtures under `frontend/src/fixtures/deal-room/` and typed domain helpers under
   `frontend/src/data/`. Cover multiple severities, statuses, categories, owners, missing evidence,
   unknown costs and sufficiently long text to exercise wrapping. Define aggregate and sorting
   semantics once and share them across the cards, table and detail panel.
2. Implement focused Findings view components under `frontend/src/components/deal-room/`, reusing
   existing `WorkspaceCard`, `Badge`, `Button`, `Input`, `Select`, `Avatar`, `Dialog`, `Separator`
   and ReUI data-grid as appropriate. Match the reference's layout, not its exact aesthetic or sample
   facts; retain Quarry's semantic colors, typography, surface tokens, radii and button behavior.
3. Enable and connect the tab through `DealRoomHeader.tsx` and `DealRoomOverviewPage.tsx`; key the
   Findings state by deal identity and retain the shared tab/header/sidebar. Add the explicit sample
   label and unavailable actions before exposing it to users.
4. Test selectors for filter composition, stable sorting, correct summary totals, missing and
   recurring costs, and filtered-empty behavior. Test tab/direct URL/Back/Forward navigation, row
   selection, close/reopen and responsive detail focus, disabled action explanations, no fixture
   cross-contamination between deals, and accessible names/keyboard operation. Include empty fixture
   and long text cases. Keep test-only values local or in `frontend/src/test/fixtures/`.
5. Inspect the implemented UI against the image at a comparable desktop content width and at narrow
   widths. Verify the existing theme, contrast, focus, scrolling, 200% zoom, reduced motion and
   web/desktop routing; inspect console/network for unexpected failures. Use the existing listener
   only when appropriate or record/clean up any new dev-server process. Do not claim exact pixel
   matching of the example's overall viewport since Quarry's persistent shell changes available space.
6. Update `docs/ARCHITECTURE.md` to record the now-enabled Findings tab, clearly marked illustrative
   data, available local interactions, disabled write/evidence links, and absence of persistence.
   Re-read the relevant architecture section after implementation; this plan remains design input.

## Acceptance and verification

- Selecting **Findings** and opening `?section=findings` both show the page within the existing Deal
  Room header and sidebar on web and desktop; refresh and browser history select the same tab.
- The page visibly identifies sample data. Summary cards, filters, sort, table selection, detail close
  and reopen behave consistently from the same fixture set. A detail cannot imply a real evidence
  document or project exists. New and Edit cannot claim to save anything.
- The reference's wide two-column composition, metrics and toolbar are recognizable, with Quarry's
  theme and existing components; narrower layouts remain readable and keyboard-operable. No data,
  fixture, mock, or state from one deal is represented as another deal's actual findings.
- Run focused frontend tests for the Findings helpers/view and Deal Room navigation, then from
  `frontend/`: `npm run typecheck`, `npm run check:boundaries`, `npm test`, `npm run build:web`,
  `npm run check:web-bundle`, and `npm run build:desktop-ui`. Run the bundle check only after a
  successful web build. Distinguish pre-existing failures from regressions. No backend or Tauri Rust
  checks are required if the implementation remains within this frontend-only scope.
- Review the final diff and architecture impact, run `git diff --check`, and review
  `git status --short` without disturbing existing user changes.
