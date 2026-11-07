# Quarry

Quarry is a diligence workspace for teams evaluating businesses and working through complex
projects. It brings deal information, source documents, notes, AI conversations, and presentation
templates into one place, helping teams move from reviewing material to developing a point of view.

Quarry runs in a browser and as a desktop application, with a shared experience across both.
It is under active development: core workflows are usable, while some views still contain
sample content or unfinished interactions.

## What you can do

### Organize deals and project context

Create and browse deals, search the portfolio, and switch between list and board views. Each
deal has a workspace for its overview, key questions, resource links, team access, and activity.
Keep supporting material close to the questions the team is trying to answer.

### Review documents in the Data Room

Upload PDFs, Word documents, spreadsheets, presentations, and images. Browse files, follow
processing progress, and preview supported documents inside the workspace. Team membership
controls access, and uploads retain attribution and activity history.

Search uploaded passages with keyword search or, when enabled by the server, hybrid search
that combines exact words with related meaning. Results identify Keyword and Semantic matches,
offer source filters, and can jump to valid pages in the currently open document.

The file-review grid supports browsing and filtering, but its review findings are currently
illustrative sample content rather than generated analysis of uploaded files.

### Work with an AI assistant

Create separate conversations, return to saved chats, attach files to the current turn, and
read responses as they arrive. Stop, retry, and copy responses as you work. AI capabilities
require server-side provider configuration. The Assistant does not yet retrieve or cite Data Room
evidence automatically.

### Capture working notes

Write and explicitly save notes in the Notebook, reopen saved work, and receive feedback on
save progress and revision conflicts. Notes persist across sessions; shared editing, attachments,
and offline work are future additions.

### Explore presentation templates

Import PowerPoint slides or decks into a private template catalog, browse generated previews,
open templates in the presentation editor, and export the current presentation as a PowerPoint
file. Editor changes are currently kept only while that view is open; saving deliverables is
still upcoming. Preview and AI enrichment features depend on the configured local tools and providers.

## A typical workflow

1. Create a deal and collect its key questions and resource links.
2. Upload source material to the Data Room and review relevant passages.
3. Capture observations in the Notebook and use separate Assistant conversations to explore ideas.
4. Use presentation templates to develop and export supporting material.

These workflows are being connected incrementally. Moving evidence into findings, saved
deliverables, and reusable institutional knowledge remains part of the product direction.

## Upcoming features and direction

The following areas describe intended next steps, with no committed release dates or ordering:

- **Evidence and findings:** connect source passages to findings, open questions, and diligence outputs.
- **Saved deliverables:** persist presentation edits and manage deliverable creation and history.
- **Connected knowledge:** develop the Global and Initiative Vaults beyond staging and sample views,
  and make saved work easier to find across the workspace.
- **Deeper document search:** open results across documents, highlight exact terms in previews,
  improve relevance, and support larger collections efficiently.
- **SharePoint integration:** connect external document sources to the Data Room; the current
  connection interface is not a completed import workflow.
- **Notebook organization and collaboration:** add folders, broader search, attachments, and shared work.
- **Production readiness:** strengthen organization tenancy, account recovery, identity lifecycle,
  operational controls, and deployment automation.

## Project status

Quarry is a development-stage application. Account registration, sign-in, sessions, and per-user
and deal membership checks are implemented. Production organization tenancy and identity lifecycle
remain incomplete. Some Hub, Vault, review, and deal-workspace views are samples or placeholders;
AI output still requires human review.

## Get started

For dependencies, local setup, configuration, development commands, builds, testing, and
troubleshooting, read [Getting Started](Getting%20Started.md).

For deeper technical context, see the [architecture](docs/ARCHITECTURE.md),
[domain model](docs/DOMAIN_MODEL.md), and [contributor guidance](AGENTS.md).
