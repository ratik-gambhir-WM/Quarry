/// <reference types="vite/client" />
/// <reference types="react/canary" />

interface ImportMetaEnv {
  readonly VITE_WORKSPACE_DATA_SOURCE?: "api" | "demo";
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

interface Window {
  __TTS_MERMAID_TEMPLATE_PREVIEW_INPUT__?: import("./contracts/diligenceCanvas").JsonValue;
}
