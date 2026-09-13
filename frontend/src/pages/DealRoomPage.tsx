import { useState } from "react";
import { Navigate, useLocation, useNavigate, useParams } from "react-router-dom";
import {
  DealRoomHeader,
  type DealRoomOverviewSection,
} from "../components/deal-room/DealRoomHeader";
import { DeliverablesHeader } from "../components/deal-room/DeliverablesHeader";
import { TemplatePreviewProvider } from "../components/deal-room/TemplatePreviewStore";
import { DeliverableTemplatesView } from "../components/deal-room/DeliverableTemplatesView";
import { DeliverablesView } from "../components/deal-room/DeliverablesView";
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
import {
  getDealRoomPath,
  getDeliverablesPath,
  getDeliverableTemplatesPath,
  type DealTimelineItem,
  type WorkspaceDeal,
  type WorkspaceLocationState,
} from "../data/workspace";
import { useWorkspaceDeals } from "../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

type ActiveDealView = "deal-room" | "deliverable-templates" | "diligence-graph" | "site-visits" | "deliverables" | "synthesis-canvas" | "timeline";

type DealRoomPageProps = {
  initialView?: Extract<ActiveDealView, "deal-room" | "deliverable-templates" | "deliverables">;
};

type DealRoomLocationState = DealExtractionLocationState & {
  dealView?: Exclude<ActiveDealView, "deliverable-templates">;
};

export function DealRoomPage({ initialView: routeView = "deal-room" }: DealRoomPageProps) {
  const { dealId } = useParams();
  const location = useLocation();
  const { deals: persistedDeals, loaded } = useWorkspaceDeals();
  const extractionState = location.state as DealRoomLocationState | null;
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
  const initialView = routeView === "deal-room"
    ? extractionState?.dealView ?? routeView
    : routeView;

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
      initialView={initialView}
      key={`${deal.room.id}:${initialView}`}
      navigationState={dealNavigationState}
      requestKey={location.key}
    />
  );
}

type DealRoomWorkspaceProps = {
  deal: WorkspaceDeal;
  deals: WorkspaceDeal[];
  email?: string;
  initialView: ActiveDealView;
  navigationState?: WorkspaceLocationState;
  requestKey: string;
};

function DealRoomWorkspace({ deal, deals, email, initialView, navigationState, requestKey }: DealRoomWorkspaceProps) {
  const navigate = useNavigate();
  const [activeDealView, setActiveDealView] = useState<ActiveDealView>(initialView);
  const [activeOverviewSection, setActiveOverviewSection] = useState<DealRoomOverviewSection>("overview");
  const [timelineItems, setTimelineItems] = useState<DealTimelineItem[]>(() => deal.room.timeline);
  const dealInsights = workspaceInsights.filter((insight) => insight.deal === deal.room.name);
  const openDeliverables = () => navigate(getDeliverablesPath(deal.room.id), { state: navigationState });
  const openDeliverableTemplates = () => navigate(getDeliverableTemplatesPath(deal.room.id), { state: navigationState });

  const header = activeDealView === "deal-room" ? (
    <DealRoomHeader
      activeSection={activeOverviewSection}
      onActiveSectionChange={setActiveOverviewSection}
    />
  ) : activeDealView === "deliverables" ? (
    <DeliverablesHeader mode="deliverables" onViewTemplates={openDeliverableTemplates} />
  ) : activeDealView === "deliverable-templates" ? (
    <DeliverablesHeader mode="templates" onBack={openDeliverables} />
  ) : undefined;

  return (
    <TemplatePreviewProvider requestKey={requestKey}>
      <WorkspaceLayout
        header={header}
        sidebar={
          <WorkspaceSidebar
            activeDealId={deal.room.id}
            activeSection={activeDealView === "deliverable-templates" ? "deliverables" : activeDealView}
            deals={deals}
            email={email}
            mode="deal-room"
            navigationState={navigationState}
            onDealRoomSectionChange={(section) => {
              setActiveDealView(section);
              if (section !== "deal-room" && initialView !== "deal-room") {
                navigate(getDealRoomPath(deal.room.id), {
                  state: { ...navigationState, dealView: section } satisfies DealRoomLocationState,
                });
              }
            }}
          />
        }
      >
        <div
          className={`mx-auto flex w-full max-w-[1440px] flex-col ${
            activeDealView === "deliverables" || activeDealView === "deliverable-templates" ? "h-full" : "gap-6 pb-10"
          }`}
        >
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
          ) : activeDealView === "deliverable-templates" ? (
            <DeliverableTemplatesView
              onRetry={() => navigate(getDeliverableTemplatesPath(deal.room.id), {
                replace: true,
                state: navigationState,
              })}
            />
          ) : activeDealView === "deliverables" ? (
            <DeliverablesView />
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
    </TemplatePreviewProvider>
  );
}
