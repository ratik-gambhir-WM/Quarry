# Key dependencies and source locations

This is a map of Quarry's main libraries, their upstream documentation, and the code that uses
them. Repository links point to integration code or locally maintained components, not installed
package contents. Exact versions and enabled features come from the manifests and lockfiles:

| Build root | Declared dependencies | Resolved versions |
| --- | --- | --- |
| Backend | [Cargo.toml](../backend/Cargo.toml) | [Cargo.lock](../backend/Cargo.lock) |
| Shared frontend | [package.json](../frontend/package.json) | [package-lock.json](../frontend/package-lock.json) |
| Desktop shell | [Cargo.toml](../frontend/src-tauri/Cargo.toml) | [Cargo.lock](../frontend/src-tauri/Cargo.lock) |

The lists below cover the main architectural dependencies, rather than every direct or transitive
package. Update this map when adopting, replacing, or moving one of these integrations. See
[Getting Started](../Getting%20Started.md) for installation and [architecture](ARCHITECTURE.md) for
runtime boundaries and configuration. Upstream documentation may describe newer releases; match
its APIs to the versions resolved in Quarry's lockfiles.

## Backend

| Dependency and upstream link | Quarry use | Main source locations |
| --- | --- | --- |
| [Axum](https://docs.rs/axum/), [Tower](https://docs.rs/tower/), [tower-http](https://docs.rs/tower-http/) | HTTP routing, extractors, middleware, CORS and request limits | [app/http/](../backend/src/app/http/); domain-owned `handler.rs` and `route.rs` under [domains/](../backend/src/domains/) |
| [Tokio](https://docs.rs/tokio/), [futures-util](https://docs.rs/futures-util/) | Async execution, channels, bounded concurrent work and streams | [main.rs](../backend/src/main.rs), [bootstrap.rs](../backend/src/app/bootstrap.rs), [domains/](../backend/src/domains/) |
| [Rig](https://rig.rs/docs) (`rig-core`, imported as `rig`) | OpenAI Responses requests, streaming, structured output and embeddings | [adapters/openai/](../backend/src/adapters/openai/), [template providers](../backend/src/adapters/slide_templates/providers.rs) |
| [reqwest](https://docs.rs/reqwest/) | HTTP transport for other server integrations; Rig owns its own bundled provider transport | [WM AI client](../backend/src/adapters/wm_ai/client.rs), [SharePoint adapter](../backend/src/adapters/sharepoint/) |
| [SQLx](https://docs.rs/sqlx/) | Async PostgreSQL connections and transactions behind Quarry's typed query builders | [adapters/postgres/](../backend/src/adapters/postgres/), [schema.sql](../backend/src/app/schema.sql), [migrations.rs](../backend/src/app/migrations.rs) |
| [pgvector Rust client](https://github.com/pgvector/pgvector-rust), [pgvector extension](https://github.com/pgvector/pgvector) | PostgreSQL vector encoding and document/notebook cosine retrieval | [vector.rs](../backend/src/adapters/postgres/vector.rs), [document search repository](../backend/src/domains/documents/store/search_repository.rs), [notebook search repository](../backend/src/domains/notebooks/search_repository.rs) |
| [HelixDB Rust client](https://github.com/HelixDB/helix-db/tree/main/sdks/rust), [HelixDB docs](https://docs.helix-db.com/) | Embedded-template graph/vector projections; documents and notebooks use PostgreSQL | [adapters/helix/](../backend/src/adapters/helix/), [template graph](../backend/src/domains/templates/slide_templates/graph.rs) |
| [Serde](https://serde.rs/), [serde_json](https://docs.rs/serde_json/) | Typed JSON requests, responses and stored documents | Domain-owned models under [domains/](../backend/src/domains/) |
| [Argon2](https://docs.rs/argon2/), [sha2](https://docs.rs/sha2/) | Password hashing, session-token digests and content identities | [identity service](../backend/src/domains/identity/service.rs), [shared/ids.rs](../backend/src/shared/ids.rs) |
| [tracing](https://docs.rs/tracing/), [tracing-subscriber](https://docs.rs/tracing-subscriber/) | Structured diagnostics and logging configuration | [main.rs](../backend/src/main.rs); instrumentation across [backend/src/](../backend/src/) |
| [Quarry's docx-rust fork](https://github.com/ratik-gambhir-WM/docx-rust-quarry) | Patched DOCX parsing; private Cargo Git dependency pinned to a commit | [DOCX parser](../backend/src/domains/documents/formats/docx.rs), [Cargo.toml](../backend/Cargo.toml), [CI dependency setup](../.github/actions/setup-docx-dependency/) |
| [pdf-extract](https://docs.rs/pdf-extract/), [lopdf](https://docs.rs/lopdf/) | PDF text extraction and PDF document operations | [PDF parser](../backend/src/domains/documents/formats/pdf.rs), [document viewing](../backend/src/domains/documents/viewing/) |
| [calamine](https://docs.rs/calamine/), [pptx-to-md](https://docs.rs/pptx-to-md/) | Spreadsheet and PowerPoint text extraction for document ingestion | [spreadsheet.rs](../backend/src/domains/documents/formats/spreadsheet.rs), [powerpoint.rs](../backend/src/domains/documents/formats/powerpoint.rs) |
| [image](https://docs.rs/image/) | Image decoding, validation and preview rendering | [image parser](../backend/src/domains/documents/formats/image.rs), [document viewing](../backend/src/domains/documents/viewing/) |
| [zip](https://docs.rs/zip/), [roxmltree](https://docs.rs/roxmltree/) | Bounded Office-package reading and OOXML parsing | [shared/office_zip.rs](../backend/src/shared/office_zip.rs), [template PPTX implementation](../backend/src/domains/templates/slide_templates/pptx/) |
| [tiktoken-rs](https://docs.rs/tiktoken-rs/) | Token counting and token-bounded document chunks | [text_chunking.rs](../backend/src/domains/documents/formats/text_chunking.rs) |

Rig's Agent and AgentRunner are not currently integrated. Quarry owns product orchestration in
its services and workers. The Rust SharePoint adapter is retained but is not wired into product
bootstrap. The private DOCX fork requires SSH read access; see the
[dependency security patch](ARCHITECTURE.md#docx-dependency-security-patch) for provenance and CI setup.

## Shared frontend

| Dependency and upstream link | Quarry use | Main source locations |
| --- | --- | --- |
| [React](https://react.dev/) and React DOM | Shared browser and desktop React application | [main.tsx](../frontend/src/main.tsx), [App.tsx](../frontend/src/App.tsx), [components/](../frontend/src/components/) |
| [Vite](https://vite.dev/guide/), [TypeScript](https://www.typescriptlang.org/docs/) | Builds, strict typechecking and web/desktop alias selection | [vite.config.ts](../frontend/vite.config.ts), [tsconfig.web.json](../frontend/tsconfig.web.json), [tsconfig.desktop.json](../frontend/tsconfig.desktop.json) |
| [React Router](https://reactrouter.com/) (`react-router-dom`) | Shared routes with browser-history routing on web and hash routing on desktop | [app/](../frontend/src/app/), [platform/Router.web.tsx](../frontend/src/platform/Router.web.tsx), [platform/Router.desktop.tsx](../frontend/src/platform/Router.desktop.tsx) |
| [Tailwind CSS](https://tailwindcss.com/docs) | Utility styling and shared semantic theme tokens | [index.css](../frontend/src/index.css), [vite.config.ts](../frontend/vite.config.ts) |
| [shadcn/ui](https://ui.shadcn.com/docs) | Locally owned standard controls and disclosures; CLI/registry configuration | [components/ui/](../frontend/src/components/ui/), [components.json](../frontend/components.json), [lib/utils.ts](../frontend/src/lib/utils.ts) |
| [Radix UI](https://www.radix-ui.com/primitives), [Base UI](https://base-ui.com/react/overview/quick-start), [Ark UI](https://ark-ui.com/) | Interaction and accessibility primitives behind shared controls | [components/ui/](../frontend/src/components/ui/), including [stealth/](../frontend/src/components/ui/stealth/) |
| [Lucide](https://lucide.dev/guide/packages/lucide-react) | Product icons | [components/](../frontend/src/components/) |
| [Motion](https://motion.dev/docs/react) | Component transitions, animated controls and reduced-motion paths | [components/ui/](../frontend/src/components/ui/), [interface motion guide](interface-motion.md) |
| [dnd-kit](https://docs.dndkit.com/) | Drag-and-drop interactions in shared boards | [components/kanban-board/](../frontend/src/components/kanban-board/) |
| [TanStack Table](https://tanstack.com/table/latest/docs/overview) | Headless table state behind shared data-grid components | [components/reui/data-grid/](../frontend/src/components/reui/data-grid/), [FileReviewTable.tsx](../frontend/src/components/data-room/FileReviewTable.tsx) |
| [BlockNote](https://www.blocknotejs.org/docs) (`@blocknote/core`, `@blocknote/react`, `@blocknote/shadcn`) | Native Notebook editor and its shadcn control integration | [components/notebook/](../frontend/src/components/notebook/), especially [NotebookEditor.tsx](../frontend/src/components/notebook/NotebookEditor.tsx) and [notebookSchema.ts](../frontend/src/components/notebook/notebookSchema.ts) |
| [Plate](https://platejs.org/docs) (`platejs`, `@platejs/*`) | Diligence Canvas rich-text editing | [lib/diligence-canvas/internal/editor/](../frontend/src/lib/diligence-canvas/internal/editor/) |
| [EmbedPDF](https://www.embedpdf.com/docs) (`@embedpdf/*`) | Data Room PDF rendering, viewer plugins and in-memory annotations | [components/extend/](../frontend/src/components/extend/), [DocumentPreviewPanel.tsx](../frontend/src/components/data-room/DocumentPreviewPanel.tsx) |
| [React-PDF](https://github.com/wojtekmaj/react-pdf), [PDF.js](https://mozilla.github.io/pdf.js/) | Earlier reusable PDF viewer and PDF thumbnail utilities | [components/pdf-viewer/](../frontend/src/components/pdf-viewer/), [lib/pdf-thumbnail-utils.ts](../frontend/src/lib/pdf-thumbnail-utils.ts) |
| [pdf-lib](https://pdf-lib.js.org/) | Client-side PDF operations used by the PDF editor panels | [components/extend/pdf-editor-panels.tsx](../frontend/src/components/extend/pdf-editor-panels.tsx) |
| [react-markdown](https://github.com/remarkjs/react-markdown), [remark-gfm](https://github.com/remarkjs/remark-gfm) | Markdown and GitHub-flavored Markdown rendering | [GlobalVaultPage.tsx](../frontend/src/pages/GlobalVaultPage.tsx) |
| [Vitest](https://vitest.dev/guide/), [Testing Library](https://testing-library.com/docs/react-testing-library/intro/), [happy-dom](https://github.com/capricorn86/happy-dom) | Unit/component tests and browser-like test environments | [tests/](../frontend/tests/), shared fixtures in [src/test/](../frontend/src/test/) |

shadcn components are source files maintained in Quarry, not an opaque runtime component package.
Edit/reuse `components/ui/` and preserve Quarry's tokens. Direct Radix, Base UI and Ark UI imports
stay inside that shared layer. [components.json](../frontend/components.json) also lists configured
registries; a configured registry does not imply all its components are installed. Main retained
registry integrations are:

| UI source and upstream link | Quarry use | Local source |
| --- | --- | --- |
| [Stealth](https://www.stealth.pm/) | Shared controls adapted to Quarry's theme and component contracts | [components/ui/stealth/](../frontend/src/components/ui/stealth/) and [components/ui/](../frontend/src/components/ui/) |
| [ReUI](https://reui.io/) | Shared data-grid and related interface components | [components/reui/](../frontend/src/components/reui/) |
| [Extend UI](https://ui.extend.ai/) | PDF editor interface integrated with EmbedPDF | [components/extend/](../frontend/src/components/extend/) |

The Assistant's [AgentRuntime.tsx](../frontend/src/components/chat/AgentRuntime.tsx) and the
[Diligence Canvas integration](../frontend/src/lib/diligence-canvas/) are Quarry-owned code.
React's canary selection and npm peer-dependency policy remain governed by the live manifest,
lockfile and existing `.npmrc`; this guide does not prescribe upgrades.

## Desktop boundary and external executables

| Dependency and upstream link | Quarry use | Main source locations |
| --- | --- | --- |
| [Tauri 2](https://v2.tauri.app/) (`tauri`, `@tauri-apps/api`, CLI) | Native shell, validated IPC and HTTP/SSE gateway to Axum | [src-tauri/src/](../frontend/src-tauri/src/), [capabilities/](../frontend/src-tauri/capabilities/), [runtime.desktop.ts](../frontend/src/platform/runtime.desktop.ts) |
| [LibreOffice](https://www.libreoffice.org/) | External Office-to-PDF conversion executable | [adapters/office/](../backend/src/adapters/office/) |
| [Chromium DevTools Protocol](https://chromedevtools.github.io/devtools-protocol/), [tungstenite](https://docs.rs/tungstenite/) | Installed Chromium browser controlled over WebSocket for template preview capture | [adapters/slide_templates/preview/](../backend/src/adapters/slide_templates/preview/) |

PostgreSQL, its pgvector extension, Helix, LibreOffice and Chromium have runtime setup requirements
beyond installing Rust/npm packages. See [Getting Started](../Getting%20Started.md),
[PostgreSQL storage](postgresql-storage.md) and the
[architecture configuration reference](ARCHITECTURE.md#11-configuration).
