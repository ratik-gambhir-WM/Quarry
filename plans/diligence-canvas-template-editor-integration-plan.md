# Diligence Canvas template-editor integration plan

Status: proposed

Baseline: live Quarry, Diligence Canvas, and Diligence Studio source inspected 2026-09-13

Scope: copy the reusable Diligence Canvas source into Quarry, make it a transport-free controlled
editor, open it after a template-preview selection, and retrieve the selected template's hydrated
presentation JSON through Quarry's existing web/desktop/Axum API boundary.

Primary UI route: `/hub/deals/:dealId/deliverables/templates/:templateId`

Primary Quarry endpoint: `GET /api/v1/templates/{template_id}`

Upstream endpoint: `GET {DILIGENCE_STUDIO_API_BASE_URL}/templates/{template_id}` with the fixed
`X-App-Id: Quarry_WestMonroe` identity

## Outcome

A user can browse the existing template carousel, activate a preview image, and move to a dedicated
editor route for that template. The editor route loads the selected template through
`runtime.api.getTemplate(templateId)`, validates the returned document, and passes it to a
lazy-loaded, controlled `DiligenceCanvas` component. Canvas edits replace the route-local document
immutably through `onChange`.

The first slice deliberately supports selection, loading, viewing, and in-memory editing. It does
not add persistence, generation, export, or another PowerPoint import surface. The existing gallery
remains the only template-import UI. Until a save/export workflow is specified, leaving the editor
discards edits and the UI must describe them as local to the current view.

The copied canvas will not retain its standalone networking abstraction. Remove these public props
and the implementation they enable:

- `api?: DiligenceCanvasApi`
- `apiBaseUrl?: string`
- `brandLogoSrc?: string`
- the related `fetch`, import/export callbacks, download controls, and API operation state

Quarry owns transport through `QuarryApi`; the canvas owns rendering and editing only. The bundled
brand logo remains fixed inside the copied component. Presentation-supplied colors, fonts, geometry,
and transforms remain document data, while editor chrome should use Quarry semantic tokens where
practical.

## Verified baseline and planning consequences

| Area | Live behavior | Planning consequence |
| --- | --- | --- |
| Gallery route | `DeliverableTemplatesView` is rendered at `/hub/deals/:dealId/deliverables/templates` inside `DealRoomPage`. | Add a nested route instead of an unrelated modal or page-local boolean. Direct links, refresh, and Back behavior remain meaningful on both routers. |
| Carousel | Each preview is a non-interactive `<img>` inside a card; the delete button is the only interactive control. | Add a separate accessible selection button around/in front of the image and keep Delete as a sibling control so there are no nested buttons. |
| Gallery state | `TemplatePreviewProvider` owns paginated reads, imports, deletes, refresh, stale-result protection, and feedback. | Do not put editor-document state into this catalog store. The editor owns one selected document and its own load/retry lifecycle. |
| Quarry frontend API | `QuarryApi` already has preview, delete, and PPTX-import methods with separate HTTP and Tauri mappings. | Add one typed `getTemplate` method to the same contract and both adapters. No raw `fetch` belongs in the copied library or feature components. |
| Quarry backend | The templates vertical slice already owns route, handler, service, and the injected Diligence Studio adapter. | Extend this slice with a GET on the existing parameterized template path. Do not add another client, service locator, or generic upstream proxy. |
| Diligence client work | Four user-owned backend files are already modified: the shared client now centralizes the fixed app header and 30-second timeout, and template operations use it. | Compose the GET method with that in-flight refactor and never overwrite or revert those changes. Re-read the live diff before implementation. |
| Upstream JSON | Diligence Studio already serves a template through both `/api/v1/templates/:templateId` and `/api/v1/import/:templateId`. `find()` hydrates stored image references into data URIs. | Use `/templates/:templateId`, matching the catalog resource. Do not add an asset endpoint for the initial editor flow. |
| Upstream ownership | The inspected Diligence Studio worktree has extensive uncommitted server changes. | Ship only against a committed/version-pinned upstream contract, or explicitly record the coordinated rollout dependency. Quarry should not silently depend on a transient local worktree. |
| Canvas source | `/Users/rgambhir/slide-template-endpoints/web/src/lib/diligence-canvas` contains 52 files (~368 KiB), is not in a Git repository, and includes five colocated tests. | Copy the current source as a reviewed snapshot, retain provenance/adaptation notes, and move tests under Quarry's `frontend/tests` tree. Do not place `*.test.*` below `frontend/src`. |
| Canvas dependencies | The copy directly needs `platejs@53.3.11`, `@platejs/basic-nodes@53.0.0`, and `@platejs/basic-styles@53.0.0`; their React peers accept React 18+. | Add these exact direct dependencies with npm. Preserve Quarry's React 19 canary, `.npmrc`, and unrelated lockfile entries. |
| Bundle shape | `DealRoomPage` is currently eager, while Plate and the SVG editor are substantial additions. | Keep the editor implementation behind a dynamic import so Login, ordinary Deal Room, Deliverables, and the gallery do not download it before a template is opened. |

The older `plans/diligence-studio-template-import-plan.md` is no longer the live implementation
baseline: its import behavior is now present in code. Treat current manifests, adapters, routes,
tests, and the active backend diff as authoritative.

## Target request and state flow

```text
Preview selection button
  -> navigate to /deliverables/templates/:templateId
  -> DeliverableTemplateCanvasView starts a request and lazy-loads editor code
  -> runtime.api.getTemplate(templateId)
       web: fetch Quarry /api/v1/templates/:templateId
       desktop: existing generic Tauri JSON relay to the same Quarry path
  -> Axum templates handler -> TemplateService -> DiligenceStudioClient
  -> Diligence Studio /api/v1/templates/:templateId + X-App-Id
  -> bounded hydrated JSON response
  -> frontend runtime validation
  -> controlled DiligenceCanvas value/onChange
```

The template ID is route state only in the URL sense; it is not authorization. The upstream fixed
app ID scopes Quarry's catalog but is also not authentication or tenancy.

## Stable boundaries and explicit non-goals

- Web and desktop clients call Quarry only. Do not expose port `43127`,
  `DILIGENCE_STUDIO_API_BASE_URL`, or `X-App-Id` to the browser bundle or Tauri command arguments.
- Keep `/api/v1` as the frontend target. Quarry's automatic `/api` compatibility mount may expose
  the same Axum route, but no new client code should use it.
- Diligence Studio remains the owner of template persistence, asset persistence, hydration,
  normalization for PowerPoint export, and preview generation. Quarry does not copy template JSON
  into SQLite or Helix.
- Keep the selected document request-scoped and release it on route unmount. Do not add a global
  cache, fixture fallback, `sessionStorage`, or activity-log payload capture for presentation JSON.
- Do not add a direct asset relay initially. The selected-template response already contains
  hydrated `data:` image sources.
- Do not retain `DiligenceCanvasApi`, `createDiligenceCanvasApi`, `apiBaseUrl`, a fetch override, or
  component-owned import/export requests. Raw transport remains under `frontend/src/api`.
- Do not add another template import picker to the editor. The existing header/empty-state import
  workflow remains unchanged.
- Do not add save, autosave, versioning, collaborative editing, undo history beyond what the copied
  component already provides, deliverable creation, or PPTX export in this slice.
- Do not add a Tauri command, capability, CSP origin, or desktop environment variable. The existing
  validated generic JSON GET relay can carry the new Quarry endpoint.
- Do not modify Diligence Studio as part of this Quarry change; revalidate its versioned contract
  before implementation and coordinate separately if it has drifted.

## Product and transport contracts

### Frontend document contract

Move the copied document types and runtime parser into a Quarry-owned transport-neutral module,
for example `frontend/src/contracts/diligenceCanvas.ts`. It should remain the one frontend source of
truth for:

- `DiligenceCanvasDocument`, `DiligencePresentation`, slide, element, and text-run types;
- the supported shape/image/line/text discriminants and finite numeric constraints;
- `parseDiligenceCanvasDocument(value: unknown)`.

The copied editor and both API adapters consume that module. The canvas barrel may re-export the
types for ergonomic component use, but `quarryApi.ts` should not depend on a page component.

Extend `QuarryApi` with:

```ts
getTemplate(templateId: string): Promise<DiligenceCanvasDocument>;
```

Both adapters must first treat JSON as `unknown`, run the shared parser, and return the typed
document only after validation. Adapter or parser details are not shown in the editor error UI.

### Quarry HTTP contract

```http
GET /api/v1/templates/{template_id}
Accept: application/json

200 OK
Cache-Control: private, no-store
Content-Type: application/json

{
  "presentation": {
    "title": "...",
    "preserveElementOrder": true,
    "showBranding": false,
    "slides": []
  }
}
```

Behavior:

- validate the decoded template ID before capability lookup;
- return 404 when the upstream app-scoped template does not exist;
- return sanitized 503 when Diligence Studio is unconfigured, unavailable, times out, returns an
  invalid content type, exceeds the response limit, or returns malformed/invalid JSON;
- add no automatic retry; GET is safe to retry only through the visible editor Retry action;
- set `private, no-store` because hydrated documents can contain embedded slide images and
  diligence content;
- cap the upstream body before deserialization. Use a named template-document limit (provisionally
  50 MiB, matching the upstream export JSON ceiling) and test both `Content-Length` and streamed
  overrun paths;
- parse to a JSON value and at least validate the top-level `presentation` object before returning
  it. Preserve all unknown presentation/element fields so a Rust DTO does not silently strip
  round-trip data. The stricter frontend parser remains the renderer boundary.

The upstream call uses the configured versioned base and relative `templates/{encoded-id}` path.
The common `DiligenceStudioClient` supplies the fixed app header and timeout. Do not concatenate an
unescaped caller-controlled path or log the full upstream URL/document body.

### Error and stale-result behavior

| Condition | UI/API result |
| --- | --- |
| Template selected | Navigate once and start one document request. |
| Code chunk or document loading | Keep workspace chrome and editor header visible with a labelled skeleton/status. |
| Empty/malformed upstream document | Reject it; never mount the editor with fabricated or partial data. |
| Template deleted between gallery load and selection | Show a sanitized unable-to-open state with Back to Templates; HTTP preserves 404 even if desktop exposes only a generic message. |
| Availability/transport failure | Show one sanitized alert plus Retry and Back to Templates. |
| Retry | Start a new generation and ignore completion from earlier attempts. |
| Route changes/unmount | Late completion is a no-op. Do not claim the generic desktop GET was cancelled. |
| Canvas edit | Replace the complete document immutably and mark the view as locally changed. |
| Leave editor | Discard route-local edits; do not persist them or imply otherwise. |

## Implementation sequence

### Phase 0 — Revalidate the moving baseline

1. Run `git status --short` in Quarry and preserve the four current backend modifications plus any
   later user work.
2. Re-open the live `DiligenceStudioClient`, template adapter/service/routes, frontend contract,
   carousel, page routes, tests, and manifests before editing.
3. Re-check the external canvas directory and its imports because it has no Git revision to pin.
   Record the source name/date and the Quarry-specific removals in the copied README without
   committing an absolute developer-machine path.
4. Confirm Diligence Studio's versioned `GET /api/v1/templates/:templateId` still returns the
   hydrated `{ presentation: ... }` document for `Quarry_WestMonroe` and that its own contract is
   committed or rollout-pinned.
5. Inspect the source Plate package peer requirements against the live Quarry React canary before
   installing; do not change React, Vite, TypeScript, or Tailwind versions to accommodate the copy.

Exit: the plan still matches all three live codebases, and overlapping user-owned backend edits
have a surgical integration point.

### Phase 1 — Add the Quarry template-document vertical slice

1. Add the presentation document TypeScript types/parser and the `QuarryApi.getTemplate` method.
2. Add browser mapping to encoded `/api/v1/templates/{templateId}` using the existing JSON GET
   helper, then validate the unknown response.
3. Add the equivalent Tauri TypeScript mapping through `transport.get<unknown>` and the same
   parser. Keep web and desktop route strings identical.
4. In the Diligence Studio Rust adapter, add a typed/bounded `get_template` operation using the
   shared client's GET behavior. Validate ID, success status, JSON content type, body size,
   deserialization, and top-level shape without stripping unknown fields.
5. Add `TemplateService::get`, mapping upstream 404 to `ServiceError::NotFound` and all other
   upstream contract/availability failures to a sanitized unavailable error.
6. Add an Axum GET handler and combine GET/DELETE on `/templates/{template_id}`. Return JSON with
   `Cache-Control: private, no-store`.
7. Add adapter, service, and route tests before wiring UI. Use fake/local test routers only; do not
   call the live Diligence Studio service.

Exit: web and desktop TypeScript consumers can fetch and validate the same Quarry-owned endpoint;
404, unavailable, malformed, and oversized responses are covered without a new native command.

### Phase 2 — Copy and Quarry-adapt Diligence Canvas

1. Copy the complete production dependency closure from
   `/Users/rgambhir/slide-template-endpoints/web/src/lib/diligence-canvas` to
   `frontend/src/lib/diligence-canvas`. Keep the SVG renderer, canvas model/edits, geometry,
   normalization, Plate editor, text layout, shapes, and bundled brand asset together; do not
   cherry-pick a partial set that compiles only through accidental source-repository imports.
2. Move `contracts.ts` into the Quarry contracts layer described above and update the copied
   imports. No copied production file may import outside Quarry or use an absolute source path.
3. Remove `api.ts`, `download.ts`, and their barrel exports. Simplify `DiligenceCanvas.tsx` to the
   controlled editor surface: `value`, `onChange`, slide navigation, JSON panel, editor options,
   optional presentation behavior, title/description, and class name.
4. Remove the standalone import/export toolbar controls and all request/abort/operation state.
   Change the default description so it does not mention a DiligenceCanvas API.
5. Remove `brandLogoSrc` through the public component/SVG prop chain and let `SvgBrandFrame` use
   the copied `brand-logo.png`. Keep `showBranding` behavior unless the live source proves it is
   inseparable from the removed override.
6. Keep `resolveImageSource` only as a pure rendering hook if it remains useful and transport-free.
   The Quarry editor should not supply it for hydrated template documents.
7. Adapt only editor chrome colors/borders/text to Quarry semantic tokens where needed for the
   active light theme and retained dark palette. Do not rewrite JSON-driven slide styling.
8. Copy the five source tests into mirrored paths under `frontend/tests/lib/diligence-canvas/` and
   fix their imports. Replace the top-level export-API test with controlled value/onChange, JSON
   panel, and navigation coverage. Keep focused rich-text, text-run, and SVG-layout tests.
9. Add exact direct Plate dependencies with npm and inspect the manifest/lockfile diff for unrelated
   churn. The normal source tree is already scanned by Tailwind 4, so no `@source` rule should be
   needed unless a build proves otherwise.

Exit: the copied editor has no network client, custom API prop, API-base configuration, brand-logo
override, co-located tests, source-repository imports, or raw fetch. Its focused tests and both
TypeScript targets pass.

### Phase 3 — Add the selected-template editor route and workflow

1. Add `getDeliverableTemplatePath(dealId, templateId)` beside the existing workspace route
   helpers. Encode the template ID as one path parameter.
2. Register `/hub/deals/:dealId/deliverables/templates/:templateId` in `App.tsx`, reusing
   `DealRoomPage` with a distinct `deliverable-template-editor` initial view.
3. Extend `DealRoomPage` composition so the editor route:
   - treats the sidebar section as Deliverables;
   - uses the same full-height content mode as Deliverables/Templates;
   - renders an editor header whose Back action explicitly returns to the gallery;
   - derives the display label from the route ID rather than depending on transient location state;
   - includes the template ID in any key used to reset route-local editor state.
4. Add a small `DeliverableTemplateCanvasView` orchestrator. It owns a discriminated
   loading/error/success state, the current validated document, local-dirty state, retry
   generations, and stale/unmounted response suppression.
5. Begin the document request in the small orchestrator while dynamically importing the heavy
   `DiligenceCanvas` module. Wrap only the editor code in a labelled Suspense fallback; do not make
   Login, Deal Room, Deliverables, or the gallery statically import Plate.
6. On success, render `<DiligenceCanvas value={document} onChange={...} />`. Each edit replaces the
   entire document and updates a concise local-only status. Do not send changes to Quarry.
7. Add `onSelectSlide` to `DeliverablesCarousel` and thread it through
   `DeliverableTemplatesView`. Render the preview as a semantic button named `Open <template name>
   template`. Keep the Delete button a sibling overlay and prove it does not navigate.
8. Preserve carousel navigation, preview alt text, lazy image loading, delete/import disablement,
   confirmation, refresh, and error behavior. Selection should be disabled only when the existing
   mutually exclusive catalog operation requires it.
9. Add route tests for gallery-to-editor navigation, direct editor entry, explicit Back to
   Templates, sidebar selection, and missing deal behavior. Add component tests for loading,
   success, malformed response, sanitized error, retry, stale completion, local edits, and
   selection/delete separation.

Exit: a mouse or keyboard user can open any visible preview, load exactly its ID through Quarry,
edit the returned document, and return to the gallery without regressing existing catalog actions.

### Phase 4 — Documentation, bundle, and runtime completion gate

1. Update `docs/ARCHITECTURE.md` in the same implementation change:
   - add the nested editor route to the route map;
   - document selected-document/local-edit state ownership and lazy editor loading;
   - add `getTemplate` to `QuarryApi` and `GET /templates/{template_id}` to the API surface;
   - update the template-catalog feature inventory and request flow;
   - replace the current statement that hydrated template JSON is not relayed by Quarry;
   - document the response limit, `no-store` behavior, fixed app scope, and no-auth limitation;
   - state that editor changes are ephemeral and export/save are not implemented;
   - add the new parser/editor/route tests to the verification narrative.
2. Rebuild web and desktop UI and inspect emitted chunks. Confirm Plate/canvas code is absent from
   the initial, ordinary Deal Room, and gallery request graphs and is loaded only for the editor.
3. Exercise the workflow in both browser and desktop UI when practical. Use a fake or explicitly
   disposable Diligence Studio/Quarry environment for live integration; do not run Axum against
   valuable SQLite data or start Diligence Studio against its default local catalog merely as a
   check.
4. Inspect keyboard/focus behavior, text editing, drag/resize/delete interactions, loading/error
   feedback, reduced motion, active theme, narrow/wide layouts, browser console, and network/IPC
   paths. Stop only processes started for this verification and confirm their ports are released.
5. Finish with `git diff --check`, final diff review, and `git status --short`, distinguishing the
   pre-existing backend edits from this implementation.

## Planned file changes

| Area | Expected files |
| --- | --- |
| Canvas document contract | `frontend/src/contracts/diligenceCanvas.ts`, `frontend/src/contracts/quarryApi.ts` |
| Browser/desktop mappings | `frontend/src/api/httpQuarryApi.ts`, `frontend/src/api/tauriQuarryApi.ts` |
| Canvas copy | `frontend/src/lib/diligence-canvas/**` including `assets/brand-logo.png`, excluding copied API/download modules and co-located tests |
| Editor orchestration | `frontend/src/components/deal-room/DeliverableTemplateCanvasView.tsx` |
| Gallery selection | `frontend/src/components/deal-room/DeliverablesCarousel.tsx`, `frontend/src/components/deal-room/DeliverableTemplatesView.tsx` |
| Header/route composition | `frontend/src/components/deal-room/DeliverablesHeader.tsx`, `frontend/src/pages/DealRoomPage.tsx`, `frontend/src/App.tsx`, `frontend/src/data/workspace.ts` |
| Frontend dependencies | `frontend/package.json`, `frontend/package-lock.json` |
| Frontend tests | mirrored canvas tests under `frontend/tests/lib/diligence-canvas/`; adapter, carousel, view, header, page, and editor-view tests under existing `frontend/tests/**` paths |
| Upstream adapter | `backend/src/adapters/diligence_studio/templates.rs` and its mirrored tests; `client.rs` only if the live shared-helper seam truly requires a surgical addition |
| Axum template slice | `backend/src/domains/templates/{service,handler,route}.rs` and mirrored service tests |
| Backend HTTP coverage | `backend/tests/integration/http_tests.rs` |
| Canonical architecture | `docs/ARCHITECTURE.md` |

No Diligence Studio files, Quarry SQLite/Helix schema, fixture data, Tauri Rust source, capability
files, CSP, or environment schema should change for this slice.

## Verification matrix

Use actual added test names/paths if they differ from these planned names.

From `frontend/`:

```sh
npm test -- tests/lib/diligence-canvas
npm test -- tests/api/httpQuarryApi.test.ts tests/api/tauriQuarryApi.test.ts
npm test -- tests/components/deal-room/DeliverablesCarousel.test.tsx tests/components/deal-room/DeliverableTemplatesView.test.tsx tests/components/deal-room/DeliverableTemplateCanvasView.test.tsx tests/pages/DealRoomPage.test.tsx
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

Also inspect the installed dependency graph after the lockfile change:

```sh
npm ls platejs @platejs/basic-nodes @platejs/basic-styles react react-dom
```

From `backend/`:

```sh
cargo test template_document
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

No Tauri Rust gate is required if the generic GET relay is unchanged. If implementation evidence
requires native relay changes, stop and revise the scope/security review before editing, then run
the full Tauri format, Clippy, tests, desktop typecheck, boundary check, and desktop UI build.

## Acceptance criteria

- Activating a preview image navigates to the selected template's nested editor route and sends
  exactly that template ID through `QuarryApi.getTemplate`.
- Delete remains independent: activating Delete confirms/deletes and never opens the editor.
- Web and desktop use the same `/api/v1/templates/{encoded-id}` mapping; neither contacts Diligence
  Studio directly.
- Axum sends the fixed `Quarry_WestMonroe` app identity upstream, bounds/validates the response,
  preserves 404, sanitizes other failures, and sets `private, no-store` on success.
- Hydrated image data renders without a separate asset request.
- The canvas copy has no custom API/base-URL/brand-logo props, fetch override, import/export
  transport, or tests under `frontend/src`.
- The editor is controlled, emits immutable replacement documents, and clearly identifies edits
  as ephemeral.
- Loading, malformed, unavailable, retry, stale/unmounted, and success states are covered.
- Preview listing, pagination, import, delete, refresh, carousel navigation, and direct gallery
  routing retain their current behavior.
- Plate/canvas code is loaded only when the editor is opened, and both production UI builds and
  the web-bundle boundary check pass.
- `docs/ARCHITECTURE.md` describes the implemented route, API/data flow, response limits,
  trust boundary, tests, and local-only editing limitation.
- Final diff/status review shows no unrelated changes, generated output, secrets, source-machine
  paths, or accidental dependency/version churn.

## Follow-up boundary

PPTX export should be planned separately. A complete cross-platform export requires a Quarry-owned
binary response contract, Diligence Studio `POST /api/v1/export` relay with JSON/body/output limits,
browser download handling, and a deliberate Tauri binary-save capability. Do not preserve the
canvas's standalone `api` prop or direct browser API client as a shortcut for that future work.
