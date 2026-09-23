import { lazy, Suspense } from "react";
import { Route, Routes } from "react-router-dom";

import App from "@/App";

const TemplatePreviewRenderPage = lazy(() =>
  import("@/pages/TemplatePreviewRenderPage").then((module) => ({
    default: module.TemplatePreviewRenderPage,
  })),
);

export function RootRoutes() {
  return (
    <Routes>
      <Route
        element={(
          <Suspense fallback={null}>
            <TemplatePreviewRenderPage />
          </Suspense>
        )}
        path="/_internal/template-preview"
      />
      <Route element={<App />} path="*" />
    </Routes>
  );
}
