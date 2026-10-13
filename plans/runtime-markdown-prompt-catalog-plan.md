# Runtime Markdown prompt catalog plan

## Status and goal

This is a proposed implementation plan, not the current product contract. The
live code and [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) remain the
sources of truth until this plan is implemented.

Move every production-owned, fixed model prompt out of Rust and TypeScript
string constants into a version-controlled Markdown catalog. The backend will
load and validate the catalog at the beginning of application bootstrap,
inject the resulting immutable prompt set into the consumers that need it, and
fail startup with a safe, actionable error if a required prompt asset is absent
or malformed.

This plan deliberately does **not** turn request fields into files:
`QueryModelInput.prompt`, `RunAssistantThreadInput.prompt`, and their optional
`systemInstructions` remain request-scoped user input with their current
validation and override behavior. Assistant starter suggestions are intentionally
out of scope: the product does not offer starter prompts. Other Hub and Vault fixture
records are unrelated and remain fixtures. The developer-only sample input in `backend/src/bin/embedding.rs`
is embedding content rather than a model instruction; it stays out of the
catalog, although that binary will still construct a catalog-aware OpenAI
client.

## Current inventory and target ownership

The migration covers these current fixed prompt owners:

| Current owner | Runtime use | Target catalog entry |
| --- | --- | --- |
| `backend/src/adapters/openai/prompts.rs` | Generic Responses fallback prompt and fallback instructions | `system/research_agent.md`: `default-response`, `default-instructions` |
| `backend/src/adapters/openai/helix_prompt.rs` | `helix_query response` CLI example input | `company_research.md`: `helix-query-example` |
| `backend/src/domains/deals/extraction.rs` | Deal-question extraction system instruction and its variable-backed request template | `diligence_review.md`: `deal-extraction-system`, `deal-extraction-request` |
| `backend/src/domains/summaries/prompt.rs` | API and CLI summary instructions plus basic, technology-diligence, and product/application request fragments | `document_summary.md`: `summary-system`, `cli-summary-system`, `basic-summary-request`, `technology-diligence`, `product-application-deep-dive` |
| `backend/src/domains/documents/formats/image_prompt.rs` | Retrieval-oriented image description instruction used by both legacy image parsing and document ingestion | `system/research_agent.md`: `image-description` |
| `backend/src/domains/assistant/chat/service.rs` | Default ephemeral and persisted chat instruction | `system/research_agent.md`: `assistant-chat-default` |

The initial catalog stays adjacent to the Cargo package that consumes its
server-owned assets:

```text
backend/prompts/
├── document_summary.md
├── company_research.md
├── diligence_review.md
└── system/
    └── research_agent.md
```

The default directory is relative to the backend process working directory,
which is `backend/` for normal local commands. Packaged deployments must copy
these assets beside the backend executable's configured working directory or
set the explicit configuration override described below.

## Catalog format and stable contract

1. Define and document a deliberately small Markdown contract before moving
   text. Each file has YAML front matter with at least `schemaVersion`,
   `title`, and `owner`. Each prompt has a level-two heading with an explicit,
   stable `id` and exactly one fenced `text` body. A regular prompt lookup
   returns only that body—not headings, front matter, comments, or fence
   delimiters.
2. Preserve present model-visible wording, whitespace normalization, and
   ordering during the initial move. Dynamic entries use named
   `{{variables}}` only for values interpolated today; Markdown is an
   authoring format, not a general-purpose template engine.
3. Give `deal-extraction-request` its seven allowed variables:
   `deal_id`, `deal_name`, `transaction_type`, `target_company`,
   `primary_buyer`, `deal_sponsor`, and `attached_file_manifest`. Give
   `basic-summary-request` its explicit root, document-manifest, and
   skipped-files variables. Reject unknown, missing, duplicate, or unrendered
   placeholders. Render user- and document-derived values as clearly
   delimited data so they cannot alter the surrounding instruction structure.
4. Keep prompt identity separate from user data. Prompt IDs are a closed Rust
   enum or equivalent typed constants in the catalog module. Clients and
   routes never choose a file path or arbitrary prompt ID. Do not place
   secrets, customer document content, user prompts, or generated manifests in
   Markdown assets.

## Implementation steps

1. Add the Markdown files under `backend/prompts/`, preserving every fixed
   prompt and template fragment in the inventory. Include brief file-level
   purpose text and non-model comments outside fenced bodies; do not duplicate
   any prompt body in Rust or TypeScript.
2. Add a transport-free `shared::prompts` module containing:
   - typed prompt IDs and the required file/entry mapping;
   - an immutable `PromptCatalog` holding validated prompt bodies;
   - a parser/validator for schema metadata, required/unique IDs, nonempty
     fenced bodies, and permitted variables; and
   - narrow render helpers for deal extraction and document summary requests.

   Use a deliberately small parser or a maintained Markdown/front-matter
   dependency if needed. Do not parse by brittle line offsets or introduce a
   general template engine.
3. Add non-secret `PromptConfig` to `AppConfig` in
   `backend/src/app/config.rs`, with `QUARRY_PROMPTS_DIR` defaulting to
   `prompts`; reject a blank override. At the start of `bootstrap`, before
   SQLite is opened/migrated or Helix is initialized, resolve and validate the
   catalog once. Add a sanitized `BootstrapError` that identifies an asset or
   prompt ID but never logs prompt bodies. Update configuration documentation
   and packaged-asset guidance in `docs/ARCHITECTURE.md` in the implementation
   change.
4. Change `assemble_api` in `backend/src/app/bootstrap.rs` to accept the one
   `Arc<PromptCatalog>` created by bootstrap. Pass only the needed immutable
   capability to `OpenAiClient`, `AssistantChatService`, `DealService`,
   `SummaryService`, and the two image-description paths
   (`formats::image::describe_image` and `DocumentIngestionService`). Keep the
   current dependency direction: bootstrap constructs clients and services;
   handlers keep transport validation; domains do not read the filesystem,
   ambient environment, or shared request state.
5. Replace `DEFAULT_RESPONSES_PROMPT`, `DEFAULT_SYSTEM_INSTRUCTIONS`,
   `HELIX_QUERY_EXAMPLE_PROMPT`, the deal-extraction literal/template, summary
   constants/builders, `IMAGE_DESCRIPTION_PROMPT`, and
   `DEFAULT_CHAT_INSTRUCTIONS` with typed catalog lookups or rendering
   helpers. Preserve precedence exactly: explicit caller instructions win;
   otherwise assistant chat uses `assistant-chat-default`; generic OpenAI
   calls retain their existing fallback response and instructions.
6. Update every `OpenAiClient` construction site and affected test helper to
   supply a validated catalog. This includes bootstrap, `parse_document`,
   `helix_query`, `embedding`, and OpenAI unit-test constructors. The two CLI
   commands that consume prompts must load the configured catalog through the
   same configuration/loader boundary and report catalog failures without
   printing bodies. `embedding` needs the catalog-aware client constructor but
   does not add an entry for its sample content.
7. Delete superseded prompt modules/constants only after every production and
   CLI call site uses the catalog.
   Keep narrow public summary helpers only where they delegate to typed catalog
   rendering, so consumers remain small and testable.

## Verification and rollout

1. Add catalog unit tests for discovery; front-matter/entry parsing; duplicate,
   missing, empty, and malformed entries; unsupported/missing/unrendered
   variables; safe rendering of punctuation and newlines;
   and error redaction. Exact-string tests should prove the initial migrated
   provider requests retain current model-visible semantics.
2. Update current OpenAI request tests, deal-service tests, summary tests,
   image/ingestion tests, assistant-chat service tests, CLI tests, and
   `backend/tests/integration/support.rs` for catalog injection. Add
   `AppConfig` tests for the default directory and rejected blank override,
   plus bootstrap tests that prove catalog failure occurs before infrastructure
   initialization.
4. Run affected backend checks from `backend/`:

   ```sh
   cargo fmt --all -- --check
   cargo check --locked --all-targets
   cargo clippy --locked --all-targets -- -D warnings
   cargo test --locked --all-targets
   ```

   Do not use `cargo run` as verification: it can migrate local SQLite and
   needs Helix.
5. Run affected frontend checks from `frontend/`:

   ```sh
   npm run typecheck
   npm run check:boundaries
   npm test
   npm run build:web
   npm run check:web-bundle
   npm run check:bundle-size:web
   npm run build:desktop-ui
   npm run check:bundle-size:desktop
   ```

   Manually inspect the Assistant composer in web and desktop builds,
   including keyboard behavior, focus, and both themes.
6. Update `docs/ARCHITECTURE.md` in the implementation change for the
   catalog runtime boundary, bootstrap/configuration order, packaged assets,
   and prompt-body redaction. Replace its current
   statement that assistant defaults come from a Rust literal with the
   catalog source.
7. Before handoff, run `git diff --check`, inspect Markdown assets for
   accidental secrets or document data, and re-run `git status --short`
   without disturbing existing work.

## Acceptance criteria

- No production-owned fixed model prompt remains a Rust or TypeScript literal;
  each has one typed catalog source. The product has no Assistant starter prompts.
- Bootstrap validates the full catalog before opening local infrastructure,
  and every server/CLI OpenAI client receives the same immutable catalog.
- Changing a deployed catalog requires an intentional process restart, not a
  Rust rebuild or a frontend deployment.
- User-supplied prompts and instruction overrides retain their current
  validation and precedence; no caller can select an arbitrary asset, path, or
  prompt ID.
- The shared frontend never bundles or reads server filesystem assets.
- Missing, malformed, or unsafe catalog assets fail deterministically without
  logging prompt bodies, secrets, or user document content.
