# Provider parity fixtures

The unchanged TypeScript integrations and the reference checkout's installed
OpenAI SDK generated `reference.json`. No network calls are made: the classifier
uses the SDK's Responses parser with canned HTTP response bodies, and the
embedder uses a canned client response. The generator writes only here.

Regenerate from the repository root, replacing `REFERENCE_CHECKOUT` with the
absolute checkout path:

```sh
node --import REFERENCE_CHECKOUT/node_modules/tsx/dist/esm/index.mjs \
  server/tests/fixtures/providers/generate-reference.mjs REFERENCE_CHECKOUT/server
cargo test --manifest-path server/Cargo.toml --lib providers
```

The fixtures cover exact schema/request bodies, eager validation of every output
text, refusal and incomplete-response precedence, malformed response shapes,
first-output selection, and embedding-vector validation. A local loopback mock
test verifies actual HTTP requests and the absence of retry requests.
