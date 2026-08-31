import { lazy, Suspense } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
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

const routeFallback = <div className="min-h-screen bg-background" />;

function App() {
  return (
    <ThemeModeProvider>
      <Routes>
        <Route element={<Navigate replace to="/login" />} path="/" />
        <Route element={<LoginPage />} path="/login" />
        <Route element={<HubPage />} path="/hub" />
        <Route element={<AccountPage />} path="/hub/account" />
        <Route
          element={<Suspense fallback={routeFallback}><GlobalVaultPage /></Suspense>}
          path="/hub/vault"
        />
        <Route
          element={<Suspense fallback={routeFallback}><VaultPage /></Suspense>}
          path="/hub/initiatives/vault"
        />
        <Route
          element={<Suspense fallback={routeFallback}><SummarizePage /></Suspense>}
          path="/hub/summarize"
        />
        <Route
          element={<Suspense fallback={routeFallback}><LogsPage /></Suspense>}
          path="/hub/logs"
        />
        <Route element={<DealRoomPage />} path="/hub/deals/:dealId" />
        <Route
          element={
            <Suspense fallback={routeFallback}>
              <DataRoomPage />
            </Suspense>
          }
          path="/hub/deals/:dealId/data-room"
        />
        <Route element={<Navigate replace to="/login" />} path="*" />
      </Routes>
    </ThemeModeProvider>
  );
}

export default App;
