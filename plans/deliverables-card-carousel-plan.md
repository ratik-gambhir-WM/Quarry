# Deliverables card carousel plan

Status: proposed

Scope: the shared React/Vite Deliverables view, the Quarry API contract and web/desktop adapters,
an Axum templates integration, and the ReUI carousel primitives needed to render live template
preview images in web and desktop builds

Primary files:

- `frontend/src/pages/DealRoomPage.tsx`
- `frontend/src/components/deal-room/DeliverablesView.tsx`
- `frontend/src/components/deal-room/DeliverablesCarousel.tsx`
- `frontend/src/data/deliverables.ts`
- `frontend/src/contracts/quarryApi.ts`
- `frontend/src/api/httpQuarryApi.ts`
- `frontend/src/api/tauriQuarryApi.ts`
- `frontend/tests/pages/DealRoomPage.test.tsx`
- `frontend/tests/components/deal-room/DeliverablesView.test.tsx`
- `frontend/tests/components/deal-room/DeliverablesCarousel.test.tsx`
- `frontend/tests/api/httpQuarryApi.test.ts`
- `frontend/tests/api/tauriQuarryApi.test.ts`
- `frontend/src/components/ui/card.tsx`
- `frontend/src/components/ui/carousel.tsx`
- `frontend/package.json`
- `frontend/package-lock.json`
- `frontend/src-tauri/tests/quarry_api/service_tests.rs`
- `backend/src/app/config.rs`
- `backend/src/app/bootstrap.rs`
- `backend/src/domains/mod.rs`
- `backend/src/domains/templates/mod.rs`
- `backend/src/domains/templates/handler.rs`
- `backend/src/domains/templates/route.rs`
- `backend/src/domains/templates/service.rs`
- `backend/src/adapters/mod.rs`
- `backend/src/adapters/diligence_studio/mod.rs`
- `backend/src/adapters/diligence_studio/client.rs`
- `backend/tests/unit/app/config_tests.rs`
- `backend/tests/unit/domains/templates/service_tests.rs`
- `backend/tests/unit/adapters/diligence_studio/client_tests.rs`
- `backend/tests/integration/http_tests.rs`
- `docs/ARCHITECTURE.md`
- `docs/adr/0003-diligence-studio-service-boundary.md`

## Outcome

Replace the Start from templates section's centered empty-state sentence with a compact card
carousel whose PNG images come from the live paginated preview API. Keep the existing empty state
for Completed Slide(s) and In-progress slides because the supplied API does not return deliverable
status or deal ownership and therefore cannot authoritatively populate those sections.

The specialized upstream service already exposes:

```http
GET http://127.0.0.1:43127/templates/previews?page=1
```

It returns at most 10 rendered PNG previews per page as browser-ready base64 `dataUrl` values,
together with dimensions, template IDs, relative individual-preview URLs, and pagination
metadata. Quarry's Axum templates domain will orchestrate this template-preview use case through
an injected Diligence Studio capability and expose a stable product-facing endpoint.
Diligence Studio remains responsible for discovering and rendering previews behind its response
contract and may expose unrelated capabilities through other endpoints.

The implementation must use each returned `dataUrl` as the card image source. It must not copy the
old `deliverable-assets/` PNGs into frontend fixtures or silently fall back to those images when
Diligence Studio is unavailable.

The first implementation will:

- preserve the 12-pixel `Deliverables` page title, the three equal-height vertical sections, their
  current headings, dividers, and the disabled Add template control aligned to the right of the
  Start from templates heading;
- leave Completed Slide(s) and In-progress slides in their current empty states;
- load every page of the template-preview response in stable API order and render the resulting
  Start from templates records in one non-looping carousel;
- show six evenly sized template cards in the first carousel viewport at desktop widths of 1280
  pixels and above, with responsive reductions at smaller widths;
- use compact 16:9 thumbnail frames with `object-contain`, preserving the whole source image even
  when its dimensions are not exactly 16:9;
- provide visible loading, empty, error, retry, and success behavior for the remote template
  catalog, without presenting a failed request as a successful empty catalog;
- provide previous/next buttons plus Embla's pointer, touch, and keyboard behavior without
  autoplay;
- work through the same React tree and `QuarryApi` contract in web and desktop;
- avoid new persistence, schema, deliverable-generation, editing, download, and upload behavior.

## Ownership and stable boundaries

- `backend/src/domains/templates/` owns the Quarry use case, route, DTOs, validation policy, and
  orchestration because the existing module is the reserved owner of reusable deliverable
  templates.
- `backend/src/adapters/diligence_studio/` owns the concrete `reqwest` integration with the broader
  Diligence Studio service through a `DiligenceStudioClient`. For this use case it implements a
  narrow preview-source interface injected into the templates service; handlers and services do
  not construct the client.
- Diligence Studio owns preview discovery and PNG production. Quarry does not duplicate that work,
  inspect source template files, or persist rendered images in this slice.
- Add the required server-side setting
  `DILIGENCE_STUDIO_API_BASE_URL=http://127.0.0.1:43127` to the ignored local `backend/.env`.
  The base URL identifies the Diligence Studio origin; individual client methods append their
  capability-specific relative endpoints. This matches the live server, which mounts the template
  router at `/templates` and its bulk previews handler at `/previews` without an `/api` prefix. The
  existing `main.rs` dotenv load makes the value available to `AppConfig::from_env`; `AppConfig`
  parses and validates it, then `bootstrap` constructs `DiligenceStudioClient` with the configured
  URL and injects the client through `TemplatePreviewSource` into `TemplateService`. Allow HTTP
  only for loopback hosts, allow HTTPS otherwise, reject credentials/query/fragment, and normalize
  one trailing slash for safe relative joining. The client and domain service must never read the
  environment directly.
- Quarry exposes `GET /api/v1/templates/previews?page=<positive integer>`. The temporary `/api`
  compatibility mount may expose the same handler through normal router composition, but new
  clients target `/api/v1` only.
- The browser and Tauri webview never call port 43127 directly. Axum performs the upstream call,
  so the feature uses the existing `VITE_API_BASE_URL`, `QUARRY_API_BASE_URL`, CORS policy, and
  generic Tauri JSON GET relay. No CSP expansion, new Tauri command, or frontend Diligence Studio
  environment variable is needed.
- The upstream response contains no human-readable title. Preserve `templateId` as the stable
  identity and accessible identifier. A pure UI formatter may replace separators with spaces for
  presentation while retaining the exact ID in the record.
- `previewUrl` is relative to the upstream service. Preserve it in the transport DTO for contract
  fidelity, but do not use it as an `<img src>` or resolve it in the frontend. `dataUrl` is the
  required card image source.
- Template records do not belong to a deal in the supplied response. Loading them at the
  Deliverables view boundary is correct; do not send or fabricate a deal ID.

### Client and domain-service design decision

Build one integration-specific `DiligenceStudioClient`, not a template-only HTTP client, generic
REST client, or arbitrary URL-and-method proxy. Keep `TemplateService` as the domain service for
the template-catalog use case.

- Reuse the shared `reqwest::Client` constructed in `bootstrap` so outbound calls share connection
  pooling and transport configuration.
- Keep the Diligence Studio base URL, relative `templates/previews` path construction, query
  encoding, upstream DTOs, response-size limits, PNG/data-URL validation, and upstream error
  mapping inside `backend/src/adapters/diligence_studio/client.rs`.
- Resolve capability paths relative to the normalized base URL. For this method, joining the
  relative `templates/previews` endpoint produces `/templates/previews?page=N`. Keep endpoint
  paths out of environment configuration so other Diligence Studio methods can append their own
  routes to the same client base URL.
- Expose only a typed `TemplatePreviewSource` interface to `TemplateService`; the domain must not
  depend on `reqwest`, raw URLs, HTTP verbs, or untyped JSON.
- When Quarry adopts another Diligence Studio capability, add another narrow domain-facing
  interface implemented by `DiligenceStudioClient`. Do not make templates own unrelated Diligence
  Studio behavior merely because both capabilities use the same upstream client.
- Split Diligence Studio endpoint groups into focused adapter submodules if the integration grows;
  retain `DiligenceStudioClient` as the configured entry point rather than growing a generic
  request method into product code.
- Do not add a reusable generic client until at least one more integration demonstrates genuinely
  identical transport policy. Existing adapters have different authentication, response, limit,
  and error requirements, so generalizing them now would weaken those boundaries rather than
  remove meaningful duplication.

## End-to-end request flow

```text
backend/.env
  -> main.rs dotenv load
  -> AppConfig::from_env
  -> bootstrap
  -> DiligenceStudioClient(base URL)

DealRoomPage
  -> runtime.api.listTemplatePreviews(page)
  -> web: httpQuarryApi OR desktop: tauriQuarryApi
  -> GET /api/v1/templates/previews?page=N
  -> Axum templates handler
  -> templates service
  -> injected TemplatePreviewSource capability
  -> DiligenceStudioClient
  -> GET http://127.0.0.1:43127/templates/previews?page=N
  -> validated paginated JSON with rendered PNG data URLs
  -> DeliverablesView -> DeliverablesCarousel -> <img src={dataUrl}>
```

This keeps catalog orchestration in Axum's templates domain, delegates preview production to
Diligence Studio, preserves the shared frontend transport boundary, and lets
desktop reuse `quarry_api_get`. The current Tauri path validator permits a query string on an
allowed `/api/v1/` path; add a regression test for this exact route.

## API and display contracts

Represent the upstream and Quarry response with the same camelCase JSON shape:

```ts
type TemplatePreviewPage = {
  pagination: {
    page: number;
    pageSize: number;
    totalItems: number;
    totalPages: number;
    hasNextPage: boolean;
    hasPreviousPage: boolean;
  };
  previews: Array<{
    templateId: string;
    contentType: "image/png";
    dataUrl: string;
    previewUrl: string;
    width: number;
    height: number;
  }>;
};
```

Add this method to `QuarryApi` and implement it in both adapters:

```ts
listTemplatePreviews(page: number): Promise<TemplatePreviewPage>;
```

Both adapters request
`/api/v1/templates/previews?page=${encodeURIComponent(String(page))}`. Keep query validation at
the Axum handler even though the UI emits only positive integers.

Add transport-independent display data in `frontend/src/data/deliverables.ts`:

```ts
type DeliverableSlide = {
  id: string;
  thumbnailAlt: string;
  thumbnailHeight: number;
  thumbnailSrc: string;
  thumbnailWidth: number;
};
```

Map every preview to one `DeliverableSlide` with:

- `id` equal to the exact `templateId`;
- `thumbnailSrc` equal to `dataUrl`;
- dimensions copied from `width` and `height`;
- `thumbnailAlt` derived from the exact template ID, for example
  `Template preview: example`.

Preserve API order across pages. Treat duplicate template IDs, an inconsistent page sequence, or
pagination that does not advance as an invalid response rather than rendering unstable React keys
or looping forever.

## Axum templates integration

Replace the comment-only `backend/src/domains/templates/mod.rs` scaffold with the smallest
coherent read-only feature:

- define typed query, pagination, preview, and response DTOs with explicit camelCase
  serialization;
- define a narrow preview-source interface at the templates boundary and implement it with the
  integration-specific `DiligenceStudioClient` under `backend/src/adapters/diligence_studio/`;
- construct the concrete adapter in `bootstrap`, inject it into `TemplateService`, and register
  the templates router through existing API composition;
- inject the shared `reqwest::Client` into `DiligenceStudioClient`; do not create a generic REST
  wrapper, expose arbitrary outbound paths, or construct a separate client per request;
- let the handler default a missing `page` query to `1`, reject `0`, negative, non-numeric, or
  otherwise invalid page values as a sanitized 400, and delegate upstream work to the service;
- have the service request the same page from its source, validate the source result, and return
  the stable Quarry DTO without taking over rendering or presentation processing;
- have the adapter resolve the relative `templates/previews` endpoint against the configured base
  URL, add `page=N` through typed query construction, and deserialize the response without
  rewriting `dataUrl`;
- map connection failure, timeout, non-success status, invalid JSON, and invalid preview payloads
  to a sanitized upstream-dependency error at the HTTP boundary. Do not expose the configured URL,
  upstream body, or internal reqwest error to clients;
- verify `contentType === "image/png"`, non-empty bounded `templateId`, positive dimensions,
  `dataUrl` beginning with `data:image/png;base64,`, valid base64 PNG bytes, and no more than 10
  previews per upstream page before returning the response;
- bound accepted upstream response and decoded image sizes using named constants and test the
  limits. Choose limits large enough for the documented 1600 × 900 previews, but do not accept an
  unbounded base64 response into memory;
- validate pagination consistency: positive page and page size, non-negative totals, response page
  matching the requested page, `previews.length <= pageSize <= 10`, and mutually consistent
  previous/next flags;
- add no database tables, repositories, cache, mutation route, generation route, or fallback
  catalog.

The integration is a read-through orchestration layer. Each Deliverables view load requests the
current upstream catalog, so it is not durable and availability depends on the local preview
service.

## Frontend request ownership and rendering states

`DealRoomPage` owns the template-preview request because it is the route-level orchestrator. Keep
the carousel presentational. Use a discriminated state rather than independent loading/error/data
flags:

```ts
type TemplatePreviewState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "success"; slides: readonly DeliverableSlide[] };
```

On initial mount and retry:

1. request page 1;
2. append its mapped previews in response order;
3. while `hasNextPage` is true, request exactly `page + 1` and append it;
4. stop when `hasNextPage` is false;
5. commit one success result after the full catalog is assembled;
6. ignore stale completion after unmount or after a newer retry starts.

Use `totalPages` and the returned page number as safety guards, not as permission to launch every
request in parallel. Sequential loading preserves catalog order, applies backpressure to base64
payloads, and prevents one failed later page from being mistaken for a complete catalog. A failure
on any page produces the error state; do not render a partial page as a complete successful
catalog.

Render the Start from templates section as follows:

| Request state | Visible result |
| --- | --- |
| Loading | Compact labelled progress treatment in the section content area |
| Error | Concise failure message and native Retry button |
| Success with zero previews | Existing centered `You have no ...` template sentence |
| Success with one or more previews | One template carousel in API order |

Keep Completed Slide(s) and In-progress slides on their existing empty messages in every state.
Do not add a local fixture fallback, cached stale data, optimistic data, or error-to-empty coercion.

## Registry adoption and collision policy

The ReUI registry is mutable. Reinspect it on the implementation date using the shadcn CLI's
supported dry-run and diff modes:

```sh
cd frontend
npx shadcn@latest add @reui/c-carousel-3 --dry-run
npx shadcn@latest add @reui/c-carousel-3 --diff
```

The 2026-09-10 inspection proposed creating `card.tsx` and `carousel.tsx`, overwriting Quarry's
customized `button.tsx`, creating a Picsum-backed example, and adding `embla-carousel-react` plus a
package named `cn`. Do not accept that payload verbatim:

1. Run `npx shadcn@latest add @reui/c-carousel-3` without `--overwrite` and reject replacement of
   `components/ui/button.tsx`.
2. Preserve Quarry's Button variants, semantic action colors, focus styles, and `icon-sm` size.
3. Retain Card and Carousel primitives only after changing generated `cn` imports to the existing
   `@/lib/utils` helper.
4. Ensure the `cn` package is absent from `package.json` and `package-lock.json`.
5. Retain `embla-carousel-react` as the only expected new direct frontend dependency and review
   its exact lockfile subtree.
6. Use `components/examples/c-carousel-3.tsx` only as source material, then remove it so no dead
   Picsum example or remote image URL ships.
7. Ensure both generated `reInit` and `select` listeners are removed during effect cleanup.

References:

- [ReUI carousel patterns](https://reui.io/components/carousel)
- [ReUI registry setup](https://reui.io/docs/registry)
- [shadcn CLI add, dry-run, and diff options](https://ui.shadcn.com/docs/cli)

## Carousel component and layout

Create `DeliverablesCarousel.tsx` with only `slides` and `sectionLabel` props. The parent owns
request-state and empty-state branching.

Each carousel should:

- set `opts={{ align: "start", loop: false, slidesToScroll: 1 }}`;
- use the accessible name `Start from templates slides`;
- map exact template IDs to `CarouselItem` elements;
- render each `dataUrl` inside Card on a neutral semantic background with a 16:9 aspect ratio,
  `object-contain`, and a restrained border or shadow using existing semantic tokens;
- set the API dimensions as explicit image `width` and `height` attributes and use
  `loading="lazy"` plus `decoding="async"`;
- expose the template ID through the image alternative and/or accessible card label without an
  unrequested caption row;
- keep unavailable navigation directions natively disabled;
- avoid autoplay, timers, infinite looping, dots, and automatic status announcements.

Use responsive item bases:

```text
default: 1 visible
sm:      2 visible
md:      3 visible
lg:      4 visible
xl:      6 visible
```

Implementation target:

- apply `basis-full sm:basis-1/2 md:basis-1/3 lg:basis-1/4 xl:basis-1/6` to each item;
- use one consistent 12-pixel gutter through matching content margin and item padding;
- keep enough internal side padding for arrow buttons and override generated off-canvas arrow
  positions so controls remain inside the page inset;
- hide arrow buttons below `sm` only if swipe and focused-carousel keyboard operation remain
  available;
- center the carousel vertically in the existing section content area;
- retain the three equal grid rows, with no outer white card container or page-level horizontal
  scrollbar.

At the current `max-w-[1440px]` content width, six items plus gutters and internal arrow space
should produce thumbnails around 200–220 pixels wide. Verify computed geometry instead of treating
that estimate as a fixed CSS requirement.

## Accessibility and interaction requirements

- Preserve one page `h1`, three `h2` headings, and three labelled `section` regions.
- Give the template carousel a unique accessible name.
- Give loading and failure states useful text; the Retry control must be a native button with a
  visible focus ring and remain disabled while its new request is pending.
- Keep Previous and Next as native buttons with visible focus rings and screen-reader text.
- Confirm ArrowLeft and ArrowRight operate the focused carousel only.
- Confirm pointer drag and touch swipe work without trapping vertical page scrolling.
- Ensure disabled arrows are not focusable through native disabled behavior.
- Preserve unique image alternatives by incorporating each exact template ID.

## Tests

Add focused coverage at every changed boundary.

Backend:

1. Parse a configured `DILIGENCE_STUDIO_API_BASE_URL`; verify a missing, empty, unsafe, or malformed
   value fails startup configuration precisely, without mutating process-global environment in
   tests.
2. Verify the page query defaults to 1 and invalid query values return 400.
3. With an ephemeral local fake upstream, verify the adapter calls
   `/templates/previews?page=1`, preserves a valid response, and handles page 2. Include a URL-join
   regression proving the client appends that endpoint to the configured service base URL.
4. Verify non-success status, connection failure, timeout, invalid JSON, oversized response,
   invalid base64/PNG data, wrong content type, excessive preview count, and inconsistent
   pagination are sanitized and never leak the upstream URL or response body.
5. Verify the registered Quarry route returns camelCase JSON under
   `/api/v1/templates/previews`.
6. Extend architecture tests if needed to enforce that handlers do not construct the upstream
   adapter and services do not read ambient configuration.

Frontend contract and orchestration:

1. Verify `httpQuarryApi` and `tauriQuarryApi` both encode the positive page query and preserve the
   paginated response and `dataUrl`.
2. Verify initial loading, multi-page accumulation in stable order, empty success, later-page
   failure, retry, duplicate-ID rejection, and stale completion after unmount or a newer retry.
3. Verify Completed and In-progress remain empty while a successful response renders only the
   template carousel.
4. Verify a failed request renders an error and Retry rather than the empty-state sentence or
   local images.
5. Verify populated records render with API dimensions, lazy/async image attributes, exact data
   URLs, stable keys, and accessible names.
6. Verify navigation labels and disabled states when Embla geometry is trustworthy in happy-dom.
   Otherwise keep semantic rendering automated and record pointer/keyboard navigation as mandatory
   browser inspection.
7. Keep the current page-level Deliverables selection and other Deal Room views unchanged.

Tauri:

- Extend the generic relay service test to prove
  `/api/v1/templates/previews?page=1` passes path validation. No new native command or
  capability permission should be introduced.

Do not assert utility-class strings as the only proof of six-wide behavior. Confirm computed card
count and geometry in a real browser.

## Verification

Start narrow, then run the affected gates.

From `frontend/`:

```sh
npm test -- tests/components/deal-room/DeliverablesView.test.tsx
npm test -- tests/components/deal-room/DeliverablesCarousel.test.tsx
npm test -- tests/pages/DealRoomPage.test.tsx
npm test -- tests/api/httpQuarryApi.test.ts tests/api/tauriQuarryApi.test.ts
npm run typecheck
npm run check:boundaries
npm test
npm run build:web
npm run check:web-bundle
npm run build:desktop-ui
```

From `backend/`:

```sh
cargo test template_preview
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

From `frontend/src-tauri/`:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
```

Do not use `cargo run` as verification because backend startup can migrate SQLite and requires
Helix. Test the integration against an ephemeral fake upstream. A live smoke test against port
43127 is optional and requires Diligence Studio plus explicitly disposable Quarry dependencies.

Inspect Deliverables in web and desktop at widths below `sm`, at `md`/`lg`, at 1280 pixels, and at
a wide viewport. Verify:

- loading, empty, error, retry, and populated template states;
- exactly six full cards at `xl`, equal widths/gutters, and no partial seventh card initially;
- API previews render in stable cross-page order without cropping, distortion, or failed data
  URLs;
- Previous/Next disabled and enabled states, mouse click, keyboard arrows, drag, and touch swipe;
- focus visibility and no focus loss when a button becomes disabled;
- no console errors, page-level horizontal overflow, clipped arrows, direct requests from the
  browser/webview to port 43127, or fixture fallback;
- identical behavior through the web and desktop transports.

After implementation, inspect `package.json` and `package-lock.json` for only the expected Embla
addition, run `git diff --check`, inspect the complete diff for secrets/generated output/unrelated
changes, and re-run `git status --short`.

## Architecture impact

Update `docs/ARCHITECTURE.md` in the implementation change to:

- describe Start from templates as an API-backed carousel with loading, empty, error/retry, and
  populated states, while Completed and In-progress remain unbacked empty states;
- add `listTemplatePreviews` to the shared frontend contract and both adapter mappings;
- document `GET /api/v1/templates/previews?page=N` and the Axum-to-Diligence-Studio
  request flow;
- document required `DILIGENCE_STUDIO_API_BASE_URL`, the local `backend/.env` value, its
  `AppConfig -> bootstrap -> DiligenceStudioClient` injection path, and URL-safety rules;
- change the templates domain from conceptual/comment-only to a read-only upstream-backed catalog
  orchestration boundary;
- retain the deliverables domain as a conceptual owner of generated and user-managed deal
  deliverables; this slice does not add deliverable records or lifecycle behavior;
- record that previews are transient base64 PNGs, not persisted Quarry artifacts, and that
  Diligence Studio is an availability dependency for this view;
- retain the limitations that there is no completed/in-progress deliverables API, template
  mutation, deliverable generation action, persistence, tenant authorization, or enabled Add
  template action.

Add `docs/adr/0003-diligence-studio-service-boundary.md` because this introduces a new upstream
service and trust boundary. Record domain ownership, the Axum-mediated topology, configuration and
URL policy, response-size and PNG validation, failure behavior, the absence of persistence, and
why browsers and the Tauri webview do not call the service directly. Record the choice of the
integration-specific `DiligenceStudioClient` with narrow domain-facing capability interfaces over
a generic REST client, while reusing the shared `reqwest::Client` as the low-level HTTP mechanism.

## Acceptance criteria

- Start from templates obtains every card image from the upstream
  `GET http://127.0.0.1:43127/templates/previews?page=N` response through Quarry's versioned Axum
  templates endpoint; rendered `<img>` elements use the returned `dataUrl` values.
- `DILIGENCE_STUDIO_API_BASE_URL=http://127.0.0.1:43127` is supplied through the ignored local
  `backend/.env`, parsed once by `AppConfig`, and injected by `bootstrap`; the client appends the
  relative path owned by each capability rather than storing endpoint paths in configuration.
- Axum's templates domain owns catalog orchestration and contract validation, while the
  broader Diligence Studio service continues to own preview discovery and PNG generation.
- Outbound access uses `DiligenceStudioClient` with the shared `reqwest::Client`; `TemplateService`
  sees only `TemplatePreviewSource`, and no generic REST client or arbitrary upstream proxy is
  introduced.
- The browser and desktop webview make no direct requests to port 43127; no CSP expansion, new
  Tauri command, or frontend Diligence Studio base URL is added.
- Pagination follows `hasNextPage` sequentially, preserves API order, terminates safely, and does
  not render partial results as a complete catalog.
- Loading, empty, error/retry, success, stale-response, malformed-response, and upstream-failure
  paths are explicit and tested.
- Completed Slide(s) and In-progress slides retain their empty states because this API cannot
  classify or scope deliverables.
- Exactly six complete, equal-width cards are visible at `xl`; smaller breakpoints show 1/2/3/4
  cards without oversized or cramped thumbnails.
- Cards preserve complete PNGs with `object-contain` in compact 16:9 frames, use API dimensions,
  and maintain consistent 12-pixel spacing.
- All templates are reachable with accessible controls, keyboard input, and swipe; the page title,
  headings, dividers, equal-height layout, and disabled Add template control remain intact.
- No `deliverable-assets/` copy, deliverables runtime fixture, remote Picsum URL, `cn` dependency,
  or failure fallback ships; Quarry's customized `button.tsx` remains unchanged.
- Web, desktop UI, Tauri, and backend checks pass, and manual inspection shows no errors, overflow,
  clipping, cropping, or transport divergence.
- `docs/ARCHITECTURE.md` accurately records the new endpoint, configuration, integration flow,
  feature maturity, and remaining limitations, and ADR 0003 records the new service boundary.
