# Runtime Markdown prompt catalog plan

## Goal

Move every production-owned, fixed AI prompt out of Rust and TypeScript string
constants into a version-controlled Markdown catalog. The backend will load and
validate the catalog during application bootstrap, inject the resulting
immutable prompt set into the services that use it, and fail startup with a
safe, actionable error if a required prompt asset is absent or malformed.

This plan deliberately does **not** turn user request fields into files:
`QueryModelInput.prompt`, `RunAssistantThreadInput.prompt`, and their optional
`systemInstructions` remain request-scoped user input with the existing limits
and override behavior. Hub fixture suggestions also remain fixtures, because
they are not sent to a model.

## Current inventory and target ownership

The migration covers the fixed/default prompt text currently in:

| Current owner | Runtime use | Target catalog entry |
| --- | --- | --- |
| `adapters/openai/prompts.rs` | Generic Responses fallback prompt and fallback instructions | `system/research_agent.md` sections `default-response` and `default-instructions` |
| `adapters/openai/helix_prompt.rs` | `helix_query` CLI example input | `company_research.md` section `helix-query-example` |
| `domains/deals/extraction.rs` | Deal-question extraction system instruction and the variable-backed request template | `diligence_review.md` sections `deal-extraction-system` and `deal-extraction-request` |
| `domains/summaries/prompt.rs` | API and CLI summary instructions plus basic, technology-diligence, and product/application request fragments | `document_summary.md` sections `summary-system`, `cli-summary-system`, `basic-summary-request`, `technology-diligence`, and `product-application-deep-dive` |
| `domains/documents/formats/image_prompt.rs` | Retrieval-oriented image description instruction | `system/research_agent.md` section `image-description` |
| `domains/assistant/chat/service.rs` | Default ephemeral and persisted chat instruction | `system/research_agent.md` section `assistant-chat-default` |
| `frontend/src/components/chat/queryChatSuggestions.ts` | Four assistant starter-button prompts | `company_research.md` sections `starter-deal-risks`, `starter-diligence-questions`, `starter-executive-brief`, and `starter-investment-thesis` |

The four requested catalog files will therefore be the complete initial
structure:

```text
backend/prompts/
├── document_summary.md
├── company_research.md
├── diligence_review.md
└── system/
    └── research_agent.md
```

`backend/prompts/` keeps server-consumed operational assets adjacent to the
Cargo package that must ship them. It also avoids depending on the repository
root being the working directory: normal backend execution starts from
`backend/`.

## Catalog format and stable contract

1. Define a small, documented Markdown contract before moving any text. Each
   file begins with YAML front matter containing at least `schemaVersion`,
   `title`, and `owner`. Each prompt is a level-two heading whose explicit
   `id` is the stable runtime lookup key, followed by its exact prompt body in
   a fenced `text` block. The loader returns only the fenced body, not the
   Markdown heading, metadata, or fence delimiters.
2. Preserve the present model-visible wording, whitespace normalization, and
   ordering on the initial move. Dynamic templates use named `{{variables}}`
   only for values already interpolated today; Markdown remains the authoring
   representation, not an invitation to execute arbitrary template logic.
3. Give the six dynamic fields in the deal-extraction request template and
   the summary manifest/root/skipped-file fragments explicit allowed variable
   lists. Reject unknown, missing, or unrendered placeholders. Escape or
   delimit inserted values so document filenames and deal metadata cannot
   alter the surrounding instruction structure.
4. Keep prompt identity separate from user data. Prompt IDs are a closed Rust
   enum or equivalent typed constants owned by the new catalog module; routes
   and clients never select a file path or prompt ID. Do not put secrets,
   customer document content, user prompts, or runtime-generated manifests in
   the Markdown files.

## Implementation steps

1. Add the four Markdown files under `backend/prompts/`, preserving every
   fixed prompt and template fragment from the inventory above. Include a
   short file-level purpose statement and a section-level comment for the
   intended use, but keep comments outside the fenced prompt text so they are
   never sent to a provider.
2. Introduce a transport-free `shared::prompts` module with:
   - typed prompt IDs and the expected file/section mapping;
   - a `PromptCatalog` value holding validated, immutable prompt strings;
   - a Markdown parser/validator that enforces the catalog schema, unique
     IDs, permitted variables, nonempty prompt bodies, and the complete
     required-ID set; and
   - focused rendering helpers for the two existing dynamic workflows.

   Use a deliberately small parser or a maintained Markdown/front-matter
   dependency chosen from the existing Cargo ecosystem only if needed. Do not
   parse Markdown by brittle line offsets or expose a general-purpose template
   engine.
3. Add a non-secret `PromptConfig` to `AppConfig` with an explicit
   `QUARRY_PROMPTS_DIR` override and a default of `prompts`. Reject a blank
   value. In bootstrap, resolve that path once, read and validate the required
   files before assembling services, and map failures to a new sanitized
   bootstrap error that names the prompt asset/ID but never prints its body.
   Resolve relative paths from the backend process working directory as the
   existing backend runtime does; document that packaged deployments must copy
   `backend/prompts/` alongside the executable or set the override.
4. Extend `assemble_api` to construct one `Arc<PromptCatalog>` and inject it
   through constructors into `OpenAiClient`, `AssistantChatService`,
   `DealService`, `SummaryService`, and the image-description path. Preserve
   the existing dependency direction: bootstrap constructs infrastructure and
   services; handlers keep transport validation; services do not read the
   filesystem, environment, or `AppState`.
5. Replace `DEFAULT_RESPONSES_PROMPT`, `DEFAULT_SYSTEM_INSTRUCTIONS`,
   `HELIX_QUERY_EXAMPLE_PROMPT`, the deal-extraction constant/template,
   summary constants/builders, `IMAGE_DESCRIPTION_PROMPT`, and
   `DEFAULT_CHAT_INSTRUCTIONS` with typed catalog lookups. Keep current
   fallback and override precedence unchanged: explicit caller instructions
   win; otherwise assistant chat receives `assistant-chat-default`; generic
   OpenAI calls retain their existing default response/instruction behavior.
6. Update `parse_document` and `helix_query` to load the same configured
   catalog (or a small shared bootstrap helper) rather than reintroducing
   string constants or assuming a repository-root CWD. Ensure their
   user-visible failures identify an unavailable prompt catalog cleanly.
7. Remove `frontend/src/components/chat/queryChatSuggestions.ts`'s embedded
   prompt strings. Decide the delivery boundary explicitly during
   implementation: add a small read-only, versioned backend catalog endpoint
   that returns only the four approved starter IDs/title/label/prompt values,
   then update the handwritten TypeScript contract plus web and desktop
   adapters and cache/display state. Do not import backend Markdown directly
   into Vite or duplicate the strings in frontend fallback data. Treat this
   endpoint as a public-contract addition and maintain `/api/v1` targeting.
8. Delete the superseded prompt modules/constants only after all production
   and CLI consumers use the catalog. Retain existing prompt-builder APIs only
   where they now delegate to catalog rendering, so call sites remain narrow
   and testable.

## Verification and rollout

1. Add unit tests for catalog discovery, front-matter/section parsing,
   duplicate or missing IDs, empty fenced bodies, unsupported/missing
   variables, safe rendering of punctuation/newlines in runtime values, and
   error redaction. Snapshot or exact-string assertions should prove that each
   migrated provider request is byte-for-byte equivalent in semantics to the
   current initial catalog.
2. Extend service and OpenAI request tests to assert the existing fallback,
   override, deal extraction, summary, image-description, and CLI paths
   receive the intended catalog entries. Add bootstrap/config tests for the
   default directory, override, unavailable catalog, and malformed catalog.
3. For the starter-prompt endpoint, add Axum route tests and web/desktop
   adapter/component tests proving the four buttons render and send the
   server-provided prompt, while loading and endpoint-failure states remain
   accessible and visible.
4. Run the affected backend gates from `backend/`: focused catalog/config/
   service tests, `cargo fmt --all -- --check`, `cargo check --locked
   --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, and
   `cargo test --locked --all-targets`. Do not use `cargo run` as verification,
   because it can migrate local SQLite and requires Helix.
5. Run the affected frontend gates from `frontend/`: the focused chat test,
   `npm run typecheck`, `npm run check:boundaries`, `npm test`, `npm run
   build:web`, `npm run check:web-bundle`, and `npm run build:desktop-ui`.
   Manually inspect the assistant starter buttons on the web and desktop
   routes, including loading/error behavior, keyboard use, focus, and both
   themes.
6. Update `docs/ARCHITECTURE.md` with the new prompt-catalog runtime boundary,
   bootstrap/configuration behavior, packaged-asset requirement, client
   endpoint, and the fact that prompt bodies are redacted from logs. Re-check
   the documented OpenAI default-instructions statement so it names the
   catalog source rather than a Rust literal.
7. Before handoff, run `git diff --check`, inspect the final diff and prompt
   files for accidental secrets or document data, and re-run `git status
   --short` without disturbing the existing user changes.

## Acceptance criteria

- No production-owned fixed prompt remains embedded as a Rust or TypeScript
  literal; each has one catalog ID and a Markdown source of truth.
- Every backend consumer receives prompt content from the validated runtime
  catalog, and changing a deployed catalog requires an intentional process
  restart rather than a Rust rebuild.
- User-supplied prompts/instructions retain their present validation and
  precedence, and no caller can choose arbitrary prompt files or paths.
- The shared frontend does not bundle or read server filesystem assets; its
  starter prompts come through the documented API contract in both web and
  desktop runtimes.
- Missing, malformed, or unsafe catalog assets fail deterministically without
  logging prompt bodies, secrets, or user document content.
