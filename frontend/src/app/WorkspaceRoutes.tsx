import { lazy } from "react";
import type { ReactNode } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { LazyContent, LazyPage } from "./LazyPage";
import { WorkspaceProvider } from "./WorkspaceProvider";

const HubPage = lazy(() => import("../pages/HubPage").then((module) => ({ default: module.HubPage })));
const AccountPage = lazy(() => import("../pages/AccountPage").then((module) => ({ default: module.AccountPage })));
const GlobalVaultPage = lazy(() => import("../pages/GlobalVaultPage").then((module) => ({ default: module.GlobalVaultPage })));
const VaultPage = lazy(() => import("../pages/VaultPage").then((module) => ({ default: module.VaultPage })));
const SummarizePage = lazy(() => import("../pages/SummarizePage").then((module) => ({ default: module.SummarizePage })));
const LogsPage = lazy(() => import("../pages/LogsPage").then((module) => ({ default: module.LogsPage })));
const Deals = lazy(() => import("../pages/Deals").then((module) => ({ default: module.Deals })));
const DealRoomPage = lazy(() => import("../pages/DealRoomPage").then((module) => ({ default: module.DealRoomPage })));
const DealRoomOverviewPage = lazy(() => import("../pages/deal-room/DealRoomOverviewPage").then((module) => ({ default: module.DealRoomOverviewPage })));
const DealActivityPage = lazy(() => import("../pages/deal-room/DealActivityPage").then((module) => ({ default: module.DealActivityPage })));
const DealAnalysisPage = lazy(() => import("../pages/deal-room/DealAnalysisPage").then((module) => ({ default: module.DealAnalysisPage })));
const DataRoomPage = lazy(() => import("../pages/DataRoomPage").then((module) => ({ default: module.DataRoomPage })));
const DeliverablesPage = lazy(() => import("../pages/deal-room/DeliverablesPage").then((module) => ({ default: module.DeliverablesPage })));
const DeliverableTemplatesPage = lazy(() => import("../pages/deal-room/DeliverableTemplatesPage").then((module) => ({ default: module.DeliverableTemplatesPage })));
const DeliverableTemplateEditorPage = lazy(() => import("../pages/deal-room/DeliverableTemplateEditorPage").then((module) => ({ default: module.DeliverableTemplateEditorPage })));

function page(label: string, child: ReactNode) {
  return <LazyPage label={label}>{child}</LazyPage>;
}

function dealPage(label: string, child: ReactNode) {
  return <LazyContent label={label}>{child}</LazyContent>;
}

export function WorkspaceRoutes() {
  return (
    <WorkspaceProvider>
      <Routes>
        <Route index element={page("Loading Hub", <HubPage />)} />
        <Route element={page("Loading Account", <AccountPage />)} path="account" />
        <Route element={page("Loading Global Vault", <GlobalVaultPage />)} path="vault" />
        <Route element={page("Loading Vault", <VaultPage />)} path="initiatives/vault" />
        <Route element={page("Loading Assistant", <SummarizePage />)} path="summarize" />
        <Route element={page("Loading logs", <LogsPage />)} path="logs" />
        <Route element={page("Loading deals", <Deals />)} path="deals" />
        <Route element={page("Loading deal", <DealRoomPage />)} path="deals/:dealId">
          <Route index element={dealPage("Loading deal overview", <DealRoomOverviewPage />)} />
          <Route element={dealPage("Loading deal activity", <DealActivityPage />)} path="activity" />
          <Route element={dealPage("Loading data room", <DataRoomPage />)} path="data-room" />
          <Route element={dealPage("Loading analysis", <DealAnalysisPage />)} path="analysis" />
          <Route element={dealPage("Loading deliverables", <DeliverablesPage />)} path="deliverables" />
          <Route element={dealPage("Loading templates", <DeliverableTemplatesPage />)} path="deliverables/templates" />
          <Route element={dealPage("Loading template editor", <DeliverableTemplateEditorPage />)} path="deliverables/templates/:templateId" />
        </Route>
        <Route element={<Navigate replace to="/login" />} path="*" />
      </Routes>
    </WorkspaceProvider>
  );
}
