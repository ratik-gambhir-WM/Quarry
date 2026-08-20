# Shared Quarry frontend

`src/` is the only maintained React source tree for both artifacts.

- `npm run dev:web` / `npm run build:web` select the browser runtime and `BrowserRouter`.
- `npm run dev:desktop` / `npm run build:desktop-ui` select the Tauri runtime and `HashRouter`.
- `npm run typecheck` checks the shared UI once against each platform contract. The target-specific
  configs map `@quarry/runtime` and `@quarry/router` to the same files selected by Vite.
- Shared components import `@quarry/runtime`; they do not import raw HTTP or Tauri modules.
- Tauri imports are confined to `src/platform/runtime.desktop.ts`.

See the repository root README and ADR 0001 for configuration and boundary details.
