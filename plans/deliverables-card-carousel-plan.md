# Deliverables card carousel plan

Status: proposed

Revalidated: 2026-09-11 against the live Quarry working tree and the local Diligence Studio
preview endpoint

Scope: the shared React/Vite Deliverables view, the Quarry API contract and web/desktop adapters,
an Axum templates integration, and the ReUI carousel primitives needed to render live template
preview images in web and desktop builds

Primary files:

- `frontend/src/components/deal-room/DeliverablesView.tsx`
- `frontend/src/components/deal-room/DeliverablesCarousel.tsx`
- `frontend/src/data/deliverables.ts`
- `frontend/src/contracts/quarryApi.ts`
- `frontend/src/api/httpQuarryApi.ts`
- `frontend/src/api/tauriQuarryApi.ts`
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
- `backend/src/adapters/diligence_studio/templates.rs`
- `backend/tests/unit/app/config_tests.rs`
- `backend/tests/unit/domains/templates/service_tests.rs`
- `backend/tests/unit/adapters/diligence_studio/client_tests.rs`
- `backend/tests/unit/adapters/diligence_studio/templates_tests.rs`
- `backend/tests/integration/http_tests.rs`
- `docs/ARCHITECTURE.md`

## Current-state findings

- Quarry has no implementation of this feature yet. `DeliverablesView.tsx` renders three static
  empty sections, `backend/src/domains/templates/mod.rs` is comment-only, and the templates module
  is not registered in `backend/src/domains/mod.rs` or `app/bootstrap.rs`.
- `DealRoomPage.tsx` already mounts `DeliverablesView` only while the Deliverables workspace view
  is selected. It does not need a new request, state, or prop-passing responsibility for this
  slice.
- `QuarryApi`, the HTTP adapter, and the Tauri adapter have no template-preview operation. The
  generic Tauri JSON GET relay already accepts a versioned path with a query string, and the Tauri
  CSP already permits `data:` images, so only a regression test is needed in the native crate.
- Quarry has no Card or Carousel primitive and no Embla dependency. The current ReUI block still
  resolves the shared `card`, `carousel`, and `button` registry items; the resolved Card/Carousel
  payloads still request the unwanted `cn` package and the Carousel payload still omits cleanup
  for its `reInit` listener.
- Diligence Studio currently serves the bulk endpoint at
  `/api/v1/templates/previews?page=N`. The observed catalog contains 16 items over two pages, and
  its images are not all 1600 x 900; limits must be byte- and pixel-budget based rather than tied
  to one nominal dimension or current item count.

## Outcome

Replace the Start from templates section's centered empty-state sentence with a compact card
carousel whose PNG images come from the live paginated preview API. Keep the existing empty state
for Completed Slide(s) and In-progress slides because the supplied API does not return deliverable
status or deal ownership and therefore cannot authoritatively populate those sections.

The specialized upstream service already exposes:

```http
GET http://127.0.0.1:43127/api/v1/templates/previews?page=1
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
- render a compact carousel-shaped skeleton while previews are loading, then provide explicit
  empty, error/retry, and success behavior for the remote template catalog without presenting a
  failed request as a successful empty catalog;
- provide previous/next buttons plus Embla's pointer, touch, and keyboard behavior without
  autoplay;
- work through the same React tree and `QuarryApi` contract in web and desktop;
- avoid new persistence, schema, deliverable-generation, editing, download, and upload behavior.

## Ownership and stable boundaries

- `backend/src/domains/templates/` owns the Quarry use case, route, DTOs, validation policy, and
  orchestration because the existing module is the reserved owner of reusable deliverable
  templates.
- `backend/src/adapters/diligence_studio/` owns one concrete, integration-wide
  `DiligenceStudioClient`. It is not a template-specific client: `client.rs` owns the configured
  base client and shared transport policy, while capability modules such as `templates.rs` add
  typed endpoint methods. The same cloneable client may be injected into services in other domains
  as Diligence Studio gains capabilities. No adapter module imports a product domain.
- Diligence Studio owns preview discovery and PNG production. Quarry does not duplicate that work,
  inspect source template files, or persist rendered images in this slice.
- Add optional server-side `DILIGENCE_STUDIO_API_BASE_URL` configuration. For local development,
  the operator supplies `http://127.0.0.1:43127/api/v1` in the ignored `backend/.env`; the
  implementation must not read, print, or modify that local file unless configuration work is
  separately authorized. A missing value leaves the capability unconfigured and does not prevent
  the rest of Quarry from starting; the template-preview route returns a sanitized 503 until the
  value is supplied. A present but invalid value fails startup configuration.
- The base URL identifies Diligence Studio's shared versioned API boundary. Individual typed client
  methods append capability-specific relative endpoints. This matches the live server, which
  mounts all routers under `/api/v1`, the template router at `/templates`, and its bulk previews
  handler at `/previews`. The existing `main.rs` dotenv load makes the value available to
  `AppConfig::from_env`; `AppConfig` parses and validates it, then `bootstrap` constructs one
  optional `Arc<DiligenceStudioClient>` and injects a clone into `TemplateService`. Allow HTTP only
  for `localhost` or IP loopback hosts, allow HTTPS otherwise, reject credentials/query/fragment,
  require the normalized path to end in `/api/v1`, and normalize exactly one trailing slash for
  safe relative joining. Clients and domain services never read the environment directly.
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

### Shared integration client and domain-service design decision

Build one integration-specific `DiligenceStudioClient`, not a template-only HTTP client, generic
REST client, or arbitrary URL-and-method proxy. Construct it once and share it across domain
services as new Diligence Studio capabilities are adopted. Keep `TemplateService` as the domain
service for the template-catalog use case.

- Reuse the shared `reqwest::Client` constructed in `bootstrap` so outbound calls share connection
  pooling and transport configuration.
- Keep the Diligence Studio base URL and shared transport state in
  `backend/src/adapters/diligence_studio/client.rs`. Keep the relative `templates/previews` path,
  query encoding, upstream DTOs, response-size limits, PNG/data-URL validation, and endpoint error
  mapping in `backend/src/adapters/diligence_studio/templates.rs`.
- Resolve capability paths relative to the normalized `/api/v1/` base URL. Do not use a leading
  slash with `Url::join`, because that would discard the configured version prefix. For this
  method, joining relative `templates/previews` produces `/api/v1/templates/previews?page=N`.
  Keep capability endpoint paths out of environment configuration so other Diligence Studio
  methods can append their own routes to the same versioned client base URL.
- Inject the typed `DiligenceStudioClient` directly into `TemplateService`, following Quarry's
  current service-to-client composition pattern. The domain may call its typed preview method but
  must not depend on `reqwest`, raw URLs, HTTP verbs, untyped JSON, or unrelated Diligence Studio
  endpoint methods.
- When Quarry adopts another Diligence Studio capability, add another focused adapter submodule and
  inject the same client into the owning domain service. Do not make templates own unrelated
  Diligence Studio behavior merely because the methods share one configured client.
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
  -> mounts DeliverablesView only when selected
  -> DeliverablesView calls runtime.api.listTemplatePreviews(page)
  -> web: httpQuarryApi OR desktop: tauriQuarryApi
  -> GET /api/v1/templates/previews?page=N
  -> Axum templates handler
  -> templates service
  -> injected shared DiligenceStudioClient
  -> typed templates capability method
  -> GET http://127.0.0.1:43127/api/v1/templates/previews?page=N
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
- expose the template-preview wire types and method from the templates capability submodule of the
  integration-wide `DiligenceStudioClient`; keep the domain response types owned by templates and
  map the validated adapter result at the service boundary;
- parse optional Diligence Studio configuration in `AppConfig`, construct one optional shared
  adapter in `bootstrap`, inject it into `TemplateService`, and register the templates router
  through existing API composition;
- inject the shared `reqwest::Client` into `DiligenceStudioClient`; do not create a generic REST
  wrapper, expose arbitrary outbound paths, or construct a separate client per request;
- let the handler default a missing `page` query to `1`, reject `0`, negative, non-numeric, or
  otherwise invalid page values as a sanitized 400, and delegate upstream work to the service;
- have the service return a sanitized 503 when the shared client is unconfigured, request the same
  page when configured, and return the stable Quarry DTO without taking over rendering or
  presentation processing;
- have the adapter resolve the relative `templates/previews` endpoint against the configured base
  URL, add `page=N` through typed query construction, and deserialize the response without
  rewriting `dataUrl`;
- apply a Diligence Studio request timeout shorter than Quarry's global 120-second request timeout;
  enforce the response cap while streaming instead of calling an unbounded `response.bytes()`;
- log internal context server-side, then map connection failure, timeout, non-success status,
  invalid JSON, and invalid preview payloads to a stable sanitized 503. Do not expose the
  configured URL, upstream body, or internal reqwest error to clients;
- verify `contentType === "image/png"`, non-empty bounded `templateId`, positive dimensions,
  `dataUrl` beginning with `data:image/png;base64,`, valid base64 PNG bytes whose decoded dimensions
  match the declared width/height, and no more than 10 previews per upstream page before returning
  the response. Accept `previewUrl` only as a safe relative or origin-relative path (including the
  upstream's current leading-slash form), with no scheme, authority, credentials, query, or
  fragment;
- bound accepted upstream response bytes, per-image decoded bytes, dimensions, and total pixels
  using named constants and test the limits. Choose limits with headroom above the currently
  observed multi-megabyte page and variable image dimensions; do not key them to one nominal
  1600 x 900 size or accept an unbounded base64 response into memory;
- validate pagination consistency: positive page and page size, non-negative totals, response page
  matching the requested page, `previews.length <= pageSize <= 10`, `totalPages` matching the
  ceiling implied by `totalItems/pageSize`, and mutually consistent previous/next flags. Define the
  empty-catalog case explicitly as page 1, zero items/pages, no previews, and both flags false;
- apply named maximum catalog/page-count guards so a syntactically consistent but hostile
  pagination response cannot make the frontend issue an effectively unbounded sequence of
  requests;
- add no database tables, repositories, cache, mutation route, generation route, or fallback
  catalog.

The integration is a read-through orchestration layer. Each Deliverables view load requests the
current upstream catalog, so it is not durable and availability depends on the local preview
service. An absent or unavailable Diligence Studio instance degrades this section to its explicit
error/retry state without taking down unrelated Quarry routes.

## Frontend request ownership and rendering states

`DeliverablesView` owns the template-preview request. The current `DealRoomPage` conditionally
mounts this view only when the Deliverables workspace section is selected, so view-level ownership
avoids downloading the multi-megabyte base64 catalog for every Deal Room visit and avoids adding
pass-through props to the page. Keep `DeliverablesCarousel` presentational. Use a discriminated
state rather than independent loading/error/data flags:

```ts
type TemplatePreviewState =
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "success"; slides: readonly DeliverableSlide[] };
```

On `DeliverablesView` mount and retry:

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
catalog. Require page numbers to advance by exactly one, keep `pageSize`, `totalItems`, and
`totalPages` stable across the load, reject duplicate template IDs across pages, enforce the same
maximum page/catalog counts exposed by the product contract, and verify the final accumulated item
count matches `totalItems` before committing success.

Render the Start from templates section as follows:

| Request state | Visible result |
| --- | --- |
| Loading | A labelled, non-interactive skeleton row shaped like the responsive 16:9 template-card carousel |
| Error | Concise failure message and native Retry button |
| Success with zero previews | Existing centered `You have no ...` template sentence |
| Success with one or more previews | One template carousel in API order |

Keep Completed Slide(s) and In-progress slides on their existing empty messages in every state.
Do not add a local fixture fallback, cached stale data, optimistic data, or error-to-empty coercion.

Use the existing `frontend/src/components/ui/skeleton.tsx` primitive. Render only the number of
skeleton cards that can fit at the current breakpoint (1/2/3/4/6), keep their dimensions and
12-pixel gutters aligned with the loaded cards, hide them from the accessibility tree, and expose
one concise `role="status"` label such as `Loading template previews`. Do not render disabled
carousel arrows during loading or animate beyond the existing Skeleton treatment. Because the
current shared Skeleton pulse is not covered by Quarry's view-transition-only reduced-motion CSS,
apply `motion-reduce:animate-none` to these feature skeletons without broadening this slice into a
global Skeleton refactor.

## Registry adoption and collision policy

The ReUI registry is mutable. Reinspect it on the implementation date using the shadcn CLI's
supported dry-run and diff modes:

```sh
cd frontend
npx --no-install shadcn add @reui/c-carousel-3 --dry-run
npx --no-install shadcn add @reui/c-carousel-3 --view
npx --no-install shadcn add @reui/c-carousel-3 --diff
```

The 2026-09-11 inspection still proposes creating `card.tsx` and `carousel.tsx`, replacing
Quarry's customized `button.tsx`, creating a Picsum-backed example, and adding
`embla-carousel-react` plus a package named `cn`. Do not run the mutating registry install or
accept that payload verbatim:

1. Use the supported `--dry-run`, `--view`, and `--diff` modes to inspect the live registry payload
   without writing files. If the registry is unavailable, use the already documented payload as a
   design reference and repeat inspection before implementation handoff.
2. Add only the adapted local Card and Carousel primitives needed by the feature. Preserve
   Quarry's existing `button.tsx` unchanged, including its variants, semantic action colors, focus
   styles, and `icon-sm` size.
3. Change registry `cn` imports to the existing `@/lib/utils` helper and replace the registry's
   icon placeholder with Quarry's existing `Icon`/Lucide conventions.
4. Install `embla-carousel-react` directly with npm as the only expected new frontend dependency;
   ensure the `cn` package is absent from `package.json` and `package-lock.json`, and review the
   exact Embla lockfile subtree.
5. Do not add `components/examples/c-carousel-3.tsx`; the remote Picsum example is reference
   material only and must not ship.
6. Fix the upstream listener-cleanup omission so both `reInit` and `select` listeners are removed
   during effect cleanup.

References:

- [ReUI carousel patterns](https://reui.io/components/carousel)
- [ReUI registry setup](https://reui.io/docs/registry)
- [shadcn CLI add, dry-run, view, and diff options](https://ui.shadcn.com/docs/cli)

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
- keep compact Previous and Next buttons rendered at every breakpoint so keyboard access does not
  disappear on small screens; touch/pointer swipe remains an additional interaction, not the only
  mobile navigation path;
- center the carousel vertically in the existing section content area;
- retain the three equal grid rows, with no outer white card container or page-level horizontal
  scrollbar.

Tailwind's `xl` breakpoint is based on viewport width, while Quarry's expanded `w-72` sidebar and
workspace padding reduce the actual carousel width. At a 1280-pixel viewport, six cards will be
materially narrower than 200 pixels; at wider or collapsed-sidebar layouts they may reach the
200–220-pixel range. Treat six complete equal-width cards with no clipped seventh card or page
overflow as the contract, and record actual computed geometry for expanded and collapsed sidebar
states instead of asserting one universal pixel estimate.

## Accessibility and interaction requirements

- Preserve one page `h1`, three `h2` headings, and three labelled `section` regions.
- Give the template carousel a unique accessible name.
- Give the skeleton loading state a single status label and give the failure state useful visible
  text; the Retry control must be a native button with a visible focus ring and prevent duplicate
  submission. Once retry begins, replace the error controls with the loading skeleton.
- Keep Previous and Next as native buttons with visible focus rings and screen-reader text.
- Confirm ArrowLeft and ArrowRight operate the focused carousel only.
- Confirm pointer drag and touch swipe work without trapping vertical page scrolling.
- Ensure disabled arrows are not focusable through native disabled behavior.
- Give each slide a position label and preserve unique image alternatives by incorporating each
  exact template ID.

## Tests

Add focused coverage at every changed boundary.

Backend:

1. Parse optional `DILIGENCE_STUDIO_API_BASE_URL` from `AppConfig::from_values`; verify missing or
   blank values leave the capability unconfigured, while unsafe, malformed, unversioned, or
   incorrectly versioned present values fail startup configuration precisely. Do not mutate
   process-global environment in tests.
2. Verify the page query defaults to 1 and invalid query values return 400.
3. With an ephemeral local fake upstream, verify the adapter calls
   `/api/v1/templates/previews?page=1`, preserves a valid response, and handles page 2. Include a
   URL-join regression proving the client preserves the version prefix while appending the
   capability endpoint to the configured service base URL.
4. Verify an unconfigured client returns 503. Verify non-success status, connection failure,
   timeout, invalid JSON, oversized streamed response, invalid base64/PNG data, wrong content type,
   declared/decoded dimension mismatch, unsafe preview URL, excessive preview count, unreasonable
   dimensions/pixel count, excessive catalog/page count, and inconsistent pagination are sanitized
   and never leak the upstream URL or response body.
5. Verify the registered Quarry route returns camelCase JSON under
   `/api/v1/templates/previews`.
6. Extend the infrastructure-constructor allowlist test for `DiligenceStudioClient::new` and verify
   it is constructed only in `bootstrap`. Preserve the existing rule that adapters do not import
   domains and domain request layers do not import adapters.

Frontend contract and orchestration:

1. Verify `httpQuarryApi` and `tauriQuarryApi` both encode the positive page query and preserve the
   paginated response and `dataUrl`.
2. Verify the initial loading skeleton structure, hidden placeholder semantics, and single status
   label; then verify multi-page accumulation in stable order, empty success, later-page failure,
   retry, changing pagination metadata, excessive page count, final total mismatch, duplicate-ID
   rejection, and stale completion after unmount or a newer retry. Treat breakpoint-specific
   skeleton visibility as browser geometry rather than pretending happy-dom computes Tailwind
   breakpoints.
3. Verify Completed and In-progress remain empty while a successful response renders only the
   template carousel.
4. Verify a failed request renders an error and Retry rather than the empty-state sentence or
   local images.
5. Verify populated records render with API dimensions, lazy/async image attributes, exact data
   URLs, stable keys, and accessible names.
6. Verify navigation labels and disabled states when Embla geometry is trustworthy in happy-dom.
   Otherwise keep semantic rendering automated and record pointer/keyboard navigation as mandatory
   browser inspection.
7. Keep the current page-level Deliverables selection and other Deal Room views unchanged; no
   `DealRoomPage` implementation change is expected.

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
a wide viewport, with the sidebar both expanded and collapsed where it affects geometry. Verify:

- the loading skeleton matches the populated carousel's responsive footprint without layout shift,
  then empty, error, retry, and populated template states;
- exactly six full cards at `xl`, equal widths/gutters, and no partial seventh card initially;
- API previews render in stable cross-page order without cropping, distortion, or failed data
  URLs;
- Previous/Next disabled and enabled states, mouse click, keyboard arrows, drag, and touch swipe;
- focus visibility, no focus loss when a button becomes disabled, and a static skeleton under
  `prefers-reduced-motion: reduce`;
- semantic contrast in the normal light theme and a forced `data-theme="dark"` inspection while
  Quarry's dark-theme picker remains disabled;
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
- document optional `DILIGENCE_STUDIO_API_BASE_URL`, the recommended local `backend/.env` value,
  its unconfigured-capability behavior, its
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

Record the new upstream service and trust boundary in `docs/ARCHITECTURE.md` and
`docs/DOMAIN_MODEL.md`: domain ownership, the Axum-mediated topology, configuration and URL policy,
response-size and PNG validation, failure behavior, the absence of persistence, and why browsers
and the Tauri webview do not call the service directly. Record the choice of the integration-wide
`DiligenceStudioClient` with focused typed capability methods over a generic REST client or
template-only client, while reusing the shared `reqwest::Client` as the low-level HTTP mechanism.

## Acceptance criteria

- Start from templates obtains every card image from the upstream
  `GET http://127.0.0.1:43127/api/v1/templates/previews?page=N` response through Quarry's
  versioned Axum templates endpoint; rendered `<img>` elements use the returned `dataUrl` values.
- When supplied, `DILIGENCE_STUDIO_API_BASE_URL=http://127.0.0.1:43127/api/v1` is read from the
  ignored local `backend/.env`, parsed once by `AppConfig`, and injected by `bootstrap`; the client
  appends the relative path owned by each capability without dropping or duplicating the version
  prefix. When absent, Quarry still starts and the preview endpoint returns a sanitized 503.
- Axum's templates domain owns catalog orchestration and contract validation, while the
  broader Diligence Studio service continues to own preview discovery and PNG generation.
- Outbound access uses one integration-wide `DiligenceStudioClient` with the shared
  `reqwest::Client`; `TemplateService` calls only its typed template-preview method, the same client
  can later be injected into other owning domains, and no generic REST client or arbitrary
  upstream proxy is introduced.
- The browser and desktop webview make no direct requests to port 43127; no CSP expansion, new
  Tauri command, or frontend Diligence Studio base URL is added.
- Pagination follows `hasNextPage` sequentially, preserves API order, terminates safely, and does
  not render partial results as a complete catalog.
- A responsive 16:9 card skeleton is visible while loading; empty, error/retry, success,
  stale-response, malformed-response, unconfigured-capability, and upstream-failure paths are
  explicit and tested.
- Completed Slide(s) and In-progress slides retain their empty states because this API cannot
  classify or scope deliverables.
- Exactly six complete, equal-width cards are visible at `xl`; smaller breakpoints show 1/2/3/4
  cards, and measured sizes are recorded for the current expanded and collapsed sidebar rather
  than enforcing a contradictory universal 200-pixel minimum.
- Cards preserve complete PNGs with `object-contain` in compact 16:9 frames, use API dimensions,
  and maintain consistent 12-pixel spacing.
- All templates are reachable with accessible controls, keyboard input, and swipe; the page title,
  headings, dividers, equal-height layout, and disabled Add template control remain intact.
- No `deliverable-assets/` copy, deliverables runtime fixture, remote Picsum URL, `cn` dependency,
  or failure fallback ships; Quarry's customized `button.tsx` remains unchanged.
- Web, desktop UI, Tauri, and backend checks pass, and manual inspection shows no errors, overflow,
  clipping, cropping, or transport divergence.
- `docs/ARCHITECTURE.md` and `docs/DOMAIN_MODEL.md` accurately record the new endpoint,
  configuration, integration flow, service boundary, feature maturity, and remaining limitations.
