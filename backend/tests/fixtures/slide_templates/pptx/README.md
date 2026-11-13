# PowerPoint parity fixtures

These golden JSON files were supplied with the Rust source and generated against the
original TypeScript reference converter. They are retained comparison data; production Rust
has no TypeScript conversion dependency or separate template service.

The optional `generate-reference.mjs` utility accepts an explicitly supplied read-only reference
checkout. Regenerate only when deliberately updating the retained baseline; no external checkout
is required for normal builds or tests.

Run the parity suite from `backend/` with
`cargo test --locked slide_templates_pptx_parity_tests`. Regeneration overwrites the
reference fixtures and should be reviewed as a separate baseline update.

The fixtures cover styled runs and paragraphs, rounded rectangles, connectors,
visibility, off-slide content, table cell IDs/styles and automatic row sizing,
inherited decorations and placeholder geometry,
master visibility, nested rotation/reflection/nonuniform scaling, unsupported
content warnings, empty slides, and generated-deck round trips. Tests compare
complete compact JSON field sets and values, treating integer and floating-point
encodings as the same JSON number.
