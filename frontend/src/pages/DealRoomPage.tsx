import { DealAccessBoundary } from "../components/deal-room/DealAccessBoundary";
import { runtime } from "@quarry/runtime";
import { useMemo, useState, useSyncExternalStore } from "react";
import { createRequestResource } from "../data/requests/requestResource";
import {
  Navigate,
  Outlet,
  useLocation,
  useOutletContext,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { useWorkspace } from "../app/WorkspaceProvider";
import {
  getDealRoomSection,
  type DealRoomOverviewSection,
} from "../components/deal-room/DealRoomHeader";
import { WorkspaceShell } from "../components/hub/WorkspaceLayout";
import { DataRoomToolsProvider } from "../components/data-room/DataRoomToolsProvider";
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
  const {
    deals: activeDeals,
    dealsResource,
    email,
    navigationState,
    retryDeals,
  } = useWorkspace();
  const extractionState = location.state as DealExtractionLocationState | null;
  const extractionResult = extractionState?.result;
  const extractedDeal =
    extractionResult && extractionResult.deal.dealId === dealId
      ? buildWorkspaceDealFromExtractionResult(
          extractionResult,
          extractionState?.sowSourceName,
        )
      : undefined;
  const cachedDeals =
    dealsResource.status === "success" ? dealsResource.deals : activeDeals;
  const deal =
    cachedDeals.find((workspaceDeal) => workspaceDeal.room.id === dealId) ??
    extractedDeal;
  const deals =
    extractedDeal && !activeDeals.some((d) => d.room.id === dealId)
      ? [
          extractedDeal,
          ...activeDeals.filter(
            (workspaceDeal) => workspaceDeal.room.id !== extractedDeal.room.id,
          ),
        ]
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
      <section
        className="flex min-h-screen items-center justify-center p-8"
        role="alert"
      >
        <div className="max-w-[32rem] rounded-xl border border-outline-variant bg-surface-container-lowest p-6 text-center">
          <h1 className="text-lg font-semibold text-text-main">
            Deal unavailable
          </h1>
          <p className="mt-2 text-sm text-muted">{dealsResource.message}</p>
          <button
            className="mt-4 rounded-full bg-action px-4 py-2 text-sm font-semibold text-on-action"
            onClick={retryDeals}
            type="button"
          >
            Retry
          </button>
        </div>
      </section>
    );
  }

  if (!deal && dealsResource.status === "success") {
    return dealsResource.source === "server" && dealId ? (
      <DealRoomLookup key={`${email ?? ""}:${dealId}`} id={dealId} />
    ) : (
      <Navigate replace state={navigationState} to="/hub" />
    );
  }

  if (!deal) {
    return <WorkspacePageSkeleton label="Loading deal" />;
  }

  if (
    deal.persisted &&
    dealsResource.status === "success" &&
    dealsResource.source === "server"
  ) {
    return (
      <DealAccessBoundary key={`${email ?? ""}:${deal.room.id}`} deal={deal}>
        {(current) => (
          <DealRoomWorkspace
            deal={current}
            deals={deals}
            email={email}
            navigationState={dealNavigationState}
          />
        )}
      </DealAccessBoundary>
    );
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

function DealRoomLookup({ id }: { id: string }) {
  const { cacheDealResult } = useWorkspace();
  const resource = useMemo(
    () =>
      createRequestResource(() => runtime.api.getDeal(id), {
        onSuccess: cacheDealResult,
      }),
    [cacheDealResult, id],
  );
  const result = useSyncExternalStore(
    resource.subscribe,
    resource.getSnapshot,
    resource.getSnapshot,
  );
  if (result.status === "error")
    return (
      <section role="alert" className="p-8">
        <h1>Deal unavailable</h1>
        <p>{result.message}</p>
        <button type="button" onClick={resource.refresh}>
          Retry deal
        </button>
      </section>
    );

  return <WorkspacePageSkeleton label="Loading deal" />;
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
  const [searchParams, setSearchParams] = useSearchParams();
  const activeOverviewSection = getDealRoomSection(searchParams.get("section"));
  function setActiveOverviewSection(section: DealRoomOverviewSection) {
    setSearchParams(
      (previous) => {
        const next = new URLSearchParams(previous);
        if (section === "overview") next.delete("section");
        else next.set("section", section);
        return next;
      },
      { state: location.state },
    );
  }
  const [timelineItems, setTimelineItems] = useState<DealTimelineItem[]>(
    () => deal.room.timeline,
  );

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
  const shell = (
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
  return (
    <DataRoomToolsProvider
      active={activeSection === "data-room"}
      dealId={deal.room.id}
      email={email}
      key={`${email ?? ""}:${deal.room.id}`}
    >
      {shell}
    </DataRoomToolsProvider>
  );
}

function getDealRoomChildPath(pathname: string, dealId: string) {
  return pathname.slice(`/hub/deals/${encodeURIComponent(dealId)}`.length);
}

function getActiveDealSection(childPath: string): ActiveDealSection {
  if (childPath.startsWith("/data-room")) return "data-room";
  if (childPath.startsWith("/analysis")) return "analysis";
  if (childPath.startsWith("/deliverables")) return "deliverables";
  return "deal-room";
}
