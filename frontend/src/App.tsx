import { UserProvider, useUser } from "./auth/UserProvider";
import { lazy } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { LazyPage } from "./app/LazyPage";
import { ViewTransition } from "./components/ui/ViewTransition";
import { LoginPage } from "./pages/LoginPage";
import { ThemeModeProvider } from "./hooks/useThemeMode";
import { PageMetadata } from "./app/PageMetadata";
import { NotFoundPage } from "./pages/NotFoundPage";

const WorkspaceRoutes = lazy(() =>
  import("./app/WorkspaceRoutes").then((module) => ({
    default: module.WorkspaceRoutes,
  })),
);

function ProductApp() {
  const user = useUser();
  const location = useLocation();

  return (
    <ThemeModeProvider>
      <PageMetadata />
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          event.preventDefault();
          document.getElementById("main-content")?.focus();
        }}
      >
        Skip to main content
      </a>
      <ViewTransition default="none" name="quarry-page" share="auto">
        <Routes location={location}>
          <Route element={<Navigate replace to="/login" />} path="/" />
          <Route element={<LoginPage key="login" />} path="/login" />
          <Route
            element={<LoginPage key="register" register />}
            path="/register"
          />
          <Route
            element={
              user.user ? (
                <LazyPage label="Loading workspace">
                  <WorkspaceRoutes key={user.generation} />
                </LazyPage>
              ) : (
                <Navigate
                  replace
                  to="/login"
                  state={{ from: location.pathname + location.search }}
                />
              )
            }
            path="/hub/*"
          />
          <Route element={<NotFoundPage />} path="*" />
        </Routes>
      </ViewTransition>
    </ThemeModeProvider>
  );
}

export default function App() {
  return (
    <UserProvider>
      <ProductApp />
    </UserProvider>
  );
}
