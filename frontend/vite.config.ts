import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  const target = mode === "desktop" ? "desktop" : "web";

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        "@quarry/router": new URL(`./src/platform/Router.${target}.tsx`, import.meta.url).pathname,
        "@quarry/runtime": new URL(`./src/platform/runtime.${target}.ts`, import.meta.url).pathname,
      },
    },
    clearScreen: false,
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
