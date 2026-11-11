import { UserProvider, useUser } from "./auth/UserProvider";
import { Component, createRef, lazy, Suspense } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { LazyPage } from "./app/LazyPage";
import { ViewTransition } from "./components/ui/ViewTransition";
import { AppShell } from "./components/layout/AppShell";
import { QuarryButton } from "./components/ui/QuarryButton";
import { ThemeModeProvider } from "./hooks/useThemeMode";
import { PageMetadata } from "./app/PageMetadata";
import { NotFoundPage } from "./pages/NotFoundPage";
import { useMotionInput } from "./hooks/useMotionInput";

const LoginPage = lazy(() =>
  import("./pages/LoginPage").then((module) => ({ default: module.LoginPage })),
);
const WorkspaceRoutes = lazy(() =>
  import("./app/WorkspaceRoutes").then((module) => ({
    default: module.WorkspaceRoutes,
  })),
);

class AuthPage extends Component<{ register?: boolean }, { failed: boolean }> {
  state = { failed: false };
  private errorHeading = createRef<HTMLHeadingElement>();

  static getDerivedStateFromError() {
    return { failed: true };
  }

  componentDidCatch() {
    this.errorHeading.current?.focus();
  }

  render() {
    const { register = false } = this.props;
    if (this.state.failed) {
      return (
        <AppShell centered showFooter={false}>
          <section className="glass-panel mx-auto w-full max-w-[400px] space-y-md rounded-xl p-lg sm:p-xl">
            <div className="space-y-sm" role="alert">
              <h1 className="type-h1 text-on-surface" ref={this.errorHeading} tabIndex={-1}>
                {register ? "Registration couldn’t load" : "Sign in couldn’t load"}
              </h1>
              <p className="type-body-sm text-on-surface-variant">
                Check your connection, then reload this page to try again.
              </p>
            </div>
            <QuarryButton onClick={() => window.location.reload()}>
              Reload page
            </QuarryButton>
          </section>
        </AppShell>
      );
    }
    return (
      <Suspense
        fallback={
          <AppShell centered showFooter={false}>
            <p role="status">
              {register ? "Loading registration…" : "Loading sign in…"}
            </p>
          </AppShell>
        }
      >
        <LoginPage register={register} />
      </Suspense>
    );
  }
}

function ProductApp() {
  useMotionInput();
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
          <Route element={<AuthPage key="login" />} path="/login" />
          <Route
            element={<AuthPage key="register" register />}
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
