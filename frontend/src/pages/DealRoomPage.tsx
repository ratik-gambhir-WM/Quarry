import { useState } from "react";
import { Navigate, Outlet, useLocation, useOutletContext, useParams } from "react-router-dom";
import { useWorkspace } from "../app/WorkspaceProvider";
import type { DealRoomOverviewSection } from "../components/deal-room/DealRoomHeader";
import { WorkspaceShell } from "../components/hub/WorkspaceLayout";
import { WorkspaceSidebar } from "../components/hub/WorkspaceSidebar";
import type { ActiveDealSection } from "../components/hub/sidebar/sidebarTypes";
import { WorkspacePageSkeleton } from "../components/ui/WorkspacePageSkeleton";
import type { DealExtractionLocationState } from "../data/dealExtraction";
import { buildWorkspaceDealFromExtractionResult } from "../data/dealExtraction";
import {
  type DealTimelineItem,
  type WorkspaceDeal,
  type WorkspaceLocationState,
} from "../data/workspace";

export type DealRoomOutletContext = {
  activeOverviewSection: DealRoomOverviewSection;
  deal: WorkspaceDeal;
  deals: WorkspaceDeal[];
  email?: string;
  navigationState?: WorkspaceLocationState;
  setActiveOverviewSection: (section: DealRoomOverviewSection) => void;
  setTimelineItems: (items: DealTimelineItem[]) => void;
  timelineItems: DealTimelineItem[];
};

export function useDealRoom() {
  return useOutletContext<DealRoomOutletContext>();
}

export function DealRoomPage() {
  const { dealId } = useParams();
  const location = useLocation();
  const { deals: activeDeals, dealsResource, email, navigationState, retryDeals } = useWorkspace();
  const extractionState = location.state as DealExtractionLocationState | null;
  const extractionResult = extractionState?.result;
  const extractedDeal = extractionResult && extractionResult.deal.dealId === dealId
    ? buildWorkspaceDealFromExtractionResult(extractionResult, extractionState?.sowSourceName)
    : undefined;
  const deal = extractedDeal ?? activeDeals.find((workspaceDeal) => workspaceDeal.room.id === dealId);
  const deals = extractedDeal
    ? [extractedDeal, ...activeDeals.filter((workspaceDeal) => workspaceDeal.room.id !== extractedDeal.room.id)]
    : activeDeals;
  const dealNavigationState = extractionResult
    ? ({
        ...navigationState,
        result: extractionResult,
        sowSourceName: extractionState?.sowSourceName,
      } satisfies DealExtractionLocationState)
    : navigationState;

  if (!deal && dealsResource.status === "error") {
    return (
      <section className="flex min-h-screen items-center justify-center p-8" role="alert">
        <div className="max-w-lg rounded-xl border border-outline-variant bg-surface-container-lowest p-6 text-center">
          <h1 className="text-lg font-semibold text-text-main">Deal unavailable</h1>
          <p className="mt-2 text-sm text-muted">{dealsResource.message}</p>
          <button className="mt-4 rounded-full bg-action px-4 py-2 text-sm font-semibold text-on-action" onClick={retryDeals} type="button">
            Retry
          </button>
        </div>
      </section>
    );
  }

  if (!deal && dealsResource.status === "success") {
    return <Navigate replace state={navigationState} to="/hub" />;
  }

  if (!deal) {
    return <WorkspacePageSkeleton label="Loading deal" />;
  }

  return (
    <DealRoomWorkspace
      deal={deal}
      deals={deals}
      email={email}
      key={deal.room.id}
      navigationState={dealNavigationState}
    />
  );
}

function DealRoomWorkspace({
  deal,
  deals,
  email,
  navigationState,
}: {
  deal: WorkspaceDeal;
  deals: WorkspaceDeal[];
  email?: string;
  navigationState?: WorkspaceLocationState;
}) {
  const location = useLocation();
  const [activeOverviewSection, setActiveOverviewSection] = useState<DealRoomOverviewSection>("overview");
  const [timelineItems, setTimelineItems] = useState<DealTimelineItem[]>(() => deal.room.timeline);

  const childPath = getDealRoomChildPath(location.pathname, deal.room.id);
  const activeSection = getActiveDealSection(childPath);
  const context: DealRoomOutletContext = {
    activeOverviewSection,
    deal,
    deals,
    email,
    navigationState,
    setActiveOverviewSection,
    setTimelineItems,
    timelineItems,
  };
  return (
    <WorkspaceShell
      sidebar={
        <WorkspaceSidebar
          activeDealId={deal.room.id}
          activeSection={activeSection}
          deals={deals}
          email={email}
          mode="deal-room"
          navigationState={navigationState}
        />
      }
    >
      <Outlet context={context} />
    </WorkspaceShell>
  );
}

function getDealRoomChildPath(pathname: string, dealId: string) {
  return pathname.slice(`/hub/deals/${encodeURIComponent(dealId)}`.length);
}

function getActiveDealSection(childPath: string): ActiveDealSection {
  if (childPath.startsWith("/activity")) return "activity";
  if (childPath.startsWith("/data-room")) return "data-room";
  if (childPath.startsWith("/analysis")) return "analysis";
  if (childPath.startsWith("/deliverables")) return "deliverables";
  return "deal-room";
}
