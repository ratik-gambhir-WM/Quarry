import { useState } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import {
  DealRoomHeader,
  type DealRoomOverviewSection,
} from "../components/deal-room/DealRoomHeader";
import { DealSummaryCard } from "../components/deal-room/DealSummaryCard";
import { DealTimelineView } from "../components/deal-room/DealTimelineView";
import { UnderConstructionView } from "../components/deal-room/UnderConstructionView";
import { InsightsStrip } from "../components/hub/InsightsStrip";
import { WorkspaceLayout } from "../components/hub/WorkspaceLayout";
import { WorkspaceSidebar } from "../components/hub/WorkspaceSidebar";
import { WorkspacePageSkeleton } from "../components/ui/WorkspacePageSkeleton";
import type { DealExtractionLocationState } from "../data/dealExtraction";
import { buildWorkspaceDealFromExtractionResult } from "../data/dealExtraction";
import { workspaceInsights } from "../fixtures/workspace/portfolio";
import type { DealTimelineItem, WorkspaceDeal, WorkspaceLocationState } from "../data/workspace";
import { useWorkspaceDeals } from "../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

type ActiveDealView = "deal-room" | "diligence-graph" | "site-visits" | "deliverables" | "synthesis-canvas" | "timeline";

export function DealRoomPage() {
  const { dealId } = useParams();
  const location = useLocation();
  const { deals: persistedDeals, loaded } = useWorkspaceDeals();
  const extractionState = location.state as DealExtractionLocationState | null;
  const extractionResult = extractionState?.result;
  const extractedDeal =
    extractionResult && extractionResult.deal.dealId === dealId
      ? buildWorkspaceDealFromExtractionResult(extractionResult, extractionState?.sowSourceName)
      : undefined;
  const deal = extractedDeal ?? persistedDeals.find((workspaceDeal) => workspaceDeal.room.id === dealId);
  const { email, navigationState } = useWorkspaceSession();
  const deals = extractedDeal
    ? [extractedDeal, ...persistedDeals.filter((workspaceDeal) => workspaceDeal.room.id !== extractedDeal.room.id)]
    : persistedDeals;
  const dealNavigationState = extractionResult
    ? ({
        ...navigationState,
        result: extractionResult,
        sowSourceName: extractionState?.sowSourceName,
      } satisfies DealExtractionLocationState)
    : navigationState;

  if (!deal && loaded) {
    return <Navigate replace to="/hub" />;
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

type DealRoomWorkspaceProps = {
  deal: WorkspaceDeal;
  deals: WorkspaceDeal[];
  email?: string;
  navigationState?: WorkspaceLocationState;
};

function DealRoomWorkspace({ deal, deals, email, navigationState }: DealRoomWorkspaceProps) {
  const [activeDealView, setActiveDealView] = useState<ActiveDealView>("deal-room");
  const [activeOverviewSection, setActiveOverviewSection] = useState<DealRoomOverviewSection>("overview");
  const [timelineItems, setTimelineItems] = useState<DealTimelineItem[]>(() => deal.room.timeline);
  const dealInsights = workspaceInsights.filter((insight) => insight.deal === deal.room.name);

  return (
    <WorkspaceLayout
      header={activeDealView === "deal-room" ? (
        <DealRoomHeader
          activeSection={activeOverviewSection}
          onActiveSectionChange={setActiveOverviewSection}
        />
      ) : undefined}
      sidebar={
        <WorkspaceSidebar
          activeDealId={deal.room.id}
          activeSection={activeDealView}
          deals={deals}
          email={email}
          mode="deal-room"
          navigationState={navigationState}
          onDealRoomSectionChange={setActiveDealView}
        />
      }
    >
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
        {activeDealView === "timeline" ? (
          <DealTimelineView deal={deal.room} events={timelineItems} onEventsChange={setTimelineItems} />
        ) : activeDealView === "diligence-graph" ? (
          <UnderConstructionView
            description="Evidence relationships and dependency mapping for this deal."
            icon="graph"
            title="Diligence Graph"
          />
        ) : activeDealView === "site-visits" ? (
          <UnderConstructionView
            description="Planning templates and visit notes for diligence fieldwork."
            icon="person"
            title="Site Visits"
          />
        ) : activeDealView === "deliverables" ? (
          <UnderConstructionView
            description="Organize and track the materials prepared for this deal."
            icon="listAlt"
            title="Deliverables"
          />
        ) : activeDealView === "synthesis-canvas" ? (
          <UnderConstructionView
            description="A working canvas for combining findings, risks, and recommendations."
            icon="grid"
            title="Synthesis Canvas"
          />
        ) : activeOverviewSection === "file-summary" ? (
          <>
            <h1 className="sr-only">{deal.room.name}</h1>
            <InsightsStrip
              className="mt-2"
              contextLabel={deal.room.name}
              items={dealInsights}
            />
          </>
        ) : (
          <DealSummaryCard deal={deal.room} />
        )}
      </div>
    </WorkspaceLayout>
  );
}
