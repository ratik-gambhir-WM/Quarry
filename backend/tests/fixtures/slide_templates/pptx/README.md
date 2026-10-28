# PowerPoint parity fixtures

These golden JSON files were supplied with the Rust source and generated against the
TypeScript converter. Quarry keeps the comparison fixtures and a generator that can
be run deliberately against its retained `diligence-studio-server` implementation.
Production Rust has no TypeScript conversion dependency.

From the Quarry repository root, with Node dependencies already installed:

```sh
node --import "$HOME/diligence-studio-server/node_modules/tsx/dist/esm/index.mjs" \
  backend/tests/fixtures/slide_templates/pptx/generate-reference.mjs "$HOME/diligence-studio-server"
```

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
