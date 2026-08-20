import { useEffect, useState } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import { DealRoomHeader, DealRoomOverview } from "../components/deal-room/DealRoomHeader";
import { DealSummaryCard } from "../components/deal-room/DealSummaryCard";
import { DealTimelineView } from "../components/deal-room/DealTimelineView";
import { UnderConstructionView } from "../components/deal-room/UnderConstructionView";
import { InsightsStrip } from "../components/hub/InsightsStrip";
import { WorkspaceLayout } from "../components/hub/WorkspaceLayout";
import { WorkspaceSidebar } from "../components/hub/WorkspaceSidebar";
import type { DealExtractionLocationState } from "../data/dealExtraction";
import { buildWorkspaceDealFromExtractionResult } from "../data/dealExtraction";
import { workspaceInsights } from "../data/workspace";
import type { DealTimelineItem } from "../data/workspace";
import { useWorkspaceDeals } from "../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

type ActiveDealView = "deal-room" | "diligence-graph" | "site-visits" | "synthesis-canvas" | "timeline";

export function DealRoomPage() {
  const { dealId } = useParams();
  const location = useLocation();
  const { deals: persistedDeals, loaded } = useWorkspaceDeals();
  const extractionResult = (location.state as DealExtractionLocationState | null)?.result;
  const extractedDeal =
    extractionResult && String(extractionResult.deal.id) === dealId
      ? buildWorkspaceDealFromExtractionResult(extractionResult)
      : undefined;
  const deal = extractedDeal ?? persistedDeals.find((workspaceDeal) => workspaceDeal.room.id === dealId);
  const { email, navigationState } = useWorkspaceSession();
  const [activeDealView, setActiveDealView] = useState<ActiveDealView>("deal-room");
  const [timelineItems, setTimelineItems] = useState<DealTimelineItem[]>([]);
  const dealInsights = workspaceInsights.filter((insight) => insight.deal === deal?.room.name);
  const deals = extractedDeal
    ? [extractedDeal, ...persistedDeals.filter((workspaceDeal) => workspaceDeal.room.id !== extractedDeal.room.id)]
    : persistedDeals;
  const dealNavigationState = extractionResult
    ? ({
        ...navigationState,
        result: extractionResult,
      } satisfies DealExtractionLocationState)
    : navigationState;

  useEffect(() => {
    setTimelineItems(deal?.room.timeline ?? []);
  }, [deal?.room.id, deal?.room.timeline]);

  if (!deal && loaded) {
    return <Navigate replace to="/hub" />;
  }

  if (!deal) {
    return <div className="flex min-h-screen items-center justify-center bg-background text-muted">Loading deal…</div>;
  }

  return (
    <WorkspaceLayout
      header={<DealRoomHeader />}
      sidebar={
        <WorkspaceSidebar
          activeDealId={deal.room.id}
          activeSection={activeDealView}
          deals={deals}
          email={email}
          mode="deal-room"
          navigationState={dealNavigationState}
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
        ) : activeDealView === "synthesis-canvas" ? (
          <UnderConstructionView
            description="A working canvas for combining findings, risks, and recommendations."
            icon="grid"
            title="Synthesis Canvas"
          />
        ) : (
          <>
            <DealRoomOverview description={deal.room.summary} subtitle={deal.room.overviewSubtitle} />

            <div className="grid grid-cols-12 gap-6">
              <DealSummaryCard deal={deal.room} />
              <InsightsStrip
                className="col-span-12 mt-2"
                contextLabel={deal.room.name}
                items={dealInsights}
              />
            </div>
          </>
        )}
      </div>
    </WorkspaceLayout>
  );
}
