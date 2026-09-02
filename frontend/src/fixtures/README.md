# Runtime fixtures

This tree owns shipped mock, demo, and placeholder product records used by the shared UI.
Consumers import fixture files directly so removing this directory exposes every remaining
fixture-backed product path through TypeScript import failures.

Fixtures are grouped by the feature that presents them:

- `workspace/`: portfolio deals, insights, navigation, and profile placeholders
- `hub/`: suggested content, activity, and AI search prompts
- `data-room/`: report/editor and local document-search results
- `deal-room/`: deal-room-only views such as site visits
- `sidebar/`: demo sidebar spaces
- `diligence/`: retained diligence-tree fixture data

Keep domain types, selectors, and server-response mappers in `src/data`. Tests that need one-off
values should define them in the test; shared test-only fixtures belong in `src/test/fixtures`.
