import { lazy, Suspense } from "react";
import type { ReactNode } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { ViewTransition } from "./components/ui/ViewTransition";
import { WorkspacePageSkeleton } from "./components/ui/WorkspacePageSkeleton";
import { HubPage } from "./pages/HubPage";
import { DealRoomPage } from "./pages/DealRoomPage";
import { LoginPage } from "./pages/LoginPage";
import { AccountPage } from "./pages/AccountPage";
import { ThemeModeProvider } from "./hooks/useThemeMode";

const DataRoomPage = lazy(() =>
  import("./pages/DataRoomPage").then((module) => ({ default: module.DataRoomPage })),
);
const GlobalVaultPage = lazy(() =>
  import("./pages/GlobalVaultPage").then((module) => ({ default: module.GlobalVaultPage })),
);
const VaultPage = lazy(() =>
  import("./pages/VaultPage").then((module) => ({ default: module.VaultPage })),
);
const SummarizePage = lazy(() =>
  import("./pages/SummarizePage").then((module) => ({ default: module.SummarizePage })),
);
const LogsPage = lazy(() =>
  import("./pages/LogsPage").then((module) => ({ default: module.LogsPage })),
);
const Deals = lazy(() =>
  import("./pages/Deals").then((module) => ({ default: module.Deals })),
);

function App() {
  const location = useLocation();

  return (
    <ThemeModeProvider>
      <ViewTransition
        default="none"
        key={location.pathname}
        name="quarry-page"
        share="auto"
      >
        <Routes location={location}>
          <Route element={<Navigate replace to="/login" />} path="/" />
          <Route element={<LoginPage />} path="/login" />
          <Route element={<HubPage />} path="/hub" />
          <Route element={<AccountPage />} path="/hub/account" />
          <Route
            element={<LazyPage label="Loading Global Vault"><GlobalVaultPage /></LazyPage>}
            path="/hub/vault"
          />
          <Route
            element={<LazyPage label="Loading Vault"><VaultPage /></LazyPage>}
            path="/hub/initiatives/vault"
          />
          <Route
            element={<LazyPage label="Loading Explore"><SummarizePage /></LazyPage>}
            path="/hub/summarize"
          />
          <Route
            element={<LazyPage label="Loading logs"><LogsPage /></LazyPage>}
            path="/hub/logs"
          />
          <Route
            element={<LazyPage label="Loading deals"><Deals /></LazyPage>}
            path="/hub/deals"
          />
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId" />
          <Route
            element={<LazyPage label="Loading data room"><DataRoomPage /></LazyPage>}
            path="/hub/deals/:dealId/data-room"
          />
          <Route element={<Navigate replace to="/login" />} path="*" />
        </Routes>
      </ViewTransition>
    </ThemeModeProvider>
  );
}

function LazyPage({ children, label }: { children: ReactNode; label: string }) {
  return (
    <Suspense
      fallback={
        <ViewTransition default="none" exit="slide-down">
          <WorkspacePageSkeleton label={label} />
        </ViewTransition>
      }
    >
      <ViewTransition default="none" enter="slide-up">
        {children}
      </ViewTransition>
    </Suspense>
  );
}

export default App;
