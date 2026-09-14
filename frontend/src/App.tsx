import { lazy } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { LazyPage } from "./app/LazyPage";
import { ViewTransition } from "./components/ui/ViewTransition";
import { LoginPage } from "./pages/LoginPage";
import { ThemeModeProvider } from "./hooks/useThemeMode";

const WorkspaceRoutes = lazy(() =>
  import("./app/WorkspaceRoutes").then((module) => ({ default: module.WorkspaceRoutes })),
);

function App() {
  const location = useLocation();

  return (
    <ThemeModeProvider>
      <ViewTransition
        default="none"
        name="quarry-page"
        share="auto"
      >
        <Routes location={location}>
          <Route element={<Navigate replace to="/login" />} path="/" />
          <Route element={<LoginPage />} path="/login" />
          <Route
            element={<LazyPage label="Loading workspace"><WorkspaceRoutes /></LazyPage>}
            path="/hub/*"
          />
          <Route element={<Navigate replace to="/login" />} path="*" />
        </Routes>
      </ViewTransition>
    </ThemeModeProvider>
  );
}

export default App;
