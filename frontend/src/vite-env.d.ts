/// <reference types="vite/client" />
/// <reference types="react/canary" />

interface ImportMetaEnv {
  readonly VITE_WORKSPACE_DATA_SOURCE?: "api" | "demo";
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
