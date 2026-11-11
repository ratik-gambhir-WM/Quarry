import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const target = mode === "desktop" ? "desktop" : "web";

  return {
    plugins: [
      react(),
      tailwindcss(),
      {
        name: "quarry-bundle-target",
        generateBundle() {
          this.emitFile({
            fileName: ".vite/bundle-target.json",
            source: `${JSON.stringify({ target })}\n`,
            type: "asset",
          });
        },
      },
    ],
    resolve: {
      alias: {
        "@": new URL("./src", import.meta.url).pathname,
        "@quarry/router": new URL(
          `./src/platform/Router.${target}.tsx`,
          import.meta.url,
        ).pathname,
        "@quarry/runtime": new URL(
          `./src/platform/runtime.${target}.ts`,
          import.meta.url,
        ).pathname,
      },
    },
    clearScreen: false,
    build: {
      manifest: true,
      rollupOptions: {
        output: {
          // Package chunks must not absorb shared React/UI dependencies and eagerly load PDF code.
          onlyExplicitManualChunks: true,
          manualChunks(id) {
            // Keep the engine's leaf dependencies here to avoid an import cycle
            // through BlockNote's schema chunk.
            if (
              id.includes("/node_modules/prosemirror-") ||
              /\/node_modules\/(orderedmap|rope-sequence|w3c-keyname)\//.test(id)
            ) {
              return "prosemirror";
            }
            const embedPdfMarker = "/node_modules/@embedpdf/";
            const embedPdfIndex = id.indexOf(embedPdfMarker);
            if (embedPdfIndex === -1) {
              return undefined;
            }
            const packageName = id
              .slice(embedPdfIndex + embedPdfMarker.length)
              .split("/")[0];
            return packageName ? `embedpdf-${packageName}` : undefined;
          },
        },
      },
    },
    server: {
      port: 1420,
      strictPort: true,
      proxy: {
        "/api": {
          target: "http://127.0.0.1:3001",
          changeOrigin: true,
        },
      },
      watch: {
        ignored: ["**/src-tauri/**"],
      },
    },
  };
});
