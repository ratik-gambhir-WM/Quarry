import { DealTimelineView } from "../../components/deal-room/DealTimelineView";
import { DealMembersPanel } from "../../components/deal-room/DealMembersPanel";
import { DealActivity } from "../../components/deal-room/DealActivity";
import { DealDocumentSearch } from "../../components/deal-room/DealDocumentSearch";
import { DealResourcesDialog } from "../../components/deal-room/DealResourcesDialog";
import { WorkspaceCard } from "../../components/hub/WorkspaceCard";
import { useState } from "react";
import { AddDealModal } from "../../components/hub/sidebar/AddDealModal";
import { Button } from "../../components/ui/stealth/button";
import { DealSummaryCard } from "../../components/deal-room/DealSummaryCard";
import { InsightsStrip } from "../../components/hub/InsightsStrip";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { DealRoomHeader } from "../../components/deal-room/DealRoomHeader";
import { workspaceInsights } from "../../fixtures/workspace/portfolio";
import { useDealRoom } from "../DealRoomPage";

export function DealRoomOverviewPage() {
  const [addingResources, setAddingResources] = useState(false);
  const [editing, setEditing] = useState(false);
  const {
    activeOverviewSection,
    deal,
    email,
    setActiveOverviewSection,
    timelineItems,
    setTimelineItems,
  } = useDealRoom();
  const dealInsights = workspaceInsights.filter(
    (insight) => insight.deal === deal.room.name,
  );

  return (
    <WorkspaceMain
      header={
        <DealRoomHeader
          activeSection={activeOverviewSection}
          onActiveSectionChange={setActiveOverviewSection}
        />
      }
    >
      {editing && deal.persisted ? (
        <AddDealModal
          initialDeal={deal.persisted}
          email={email}
          onClose={() => setEditing(false)}
        />
      ) : null}
      {addingResources && deal.persisted ? (
        <DealResourcesDialog
          deal={deal}
          onClose={() => setAddingResources(false)}
        />
      ) : null}
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
        {activeOverviewSection === "activity" ? (
          deal.persisted ? (
            <DealActivity key={deal.room.id} dealId={deal.room.id} />
          ) : (
            <DealTimelineView
              deal={deal.room}
              events={timelineItems}
              onEventsChange={setTimelineItems}
            />
          )
        ) : activeOverviewSection === "site-visits" ? (
          <DealTimelineView
            title="Site Visits"
            deal={deal.room}
            events={timelineItems}
            onEventsChange={setTimelineItems}
          />
        ) : activeOverviewSection === "search" ? (
          <DealDocumentSearch key={deal.room.id} dealId={deal.room.id} />
        ) : activeOverviewSection === "settings" ? (
          <WorkspaceCard interactive={false} className="p-6 sm:p-7">
            <h1 className="type-h3 text-text-main">Deal settings</h1>
            {deal.persisted ? (
              <div className="mt-8">
                <DealMembersPanel deal={deal.persisted} />
              </div>
            ) : null}
            <div className="mt-8 border-t border-border pt-6">
              <p className="mt-2 text-sm text-muted">
                Manage deal details, companies, roles, and scope.
              </p>
              {deal.persisted?.capabilities?.edit ? (
                <Button
                  className="mt-5"
                  variant="secondary"
                  onClick={() => setEditing(true)}
                >
                  Edit deal
                </Button>
              ) : (
                <p className="mt-4 text-sm text-muted">
                  Settings are available for saved deals.
                </p>
              )}
            </div>
          </WorkspaceCard>
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
          <DealSummaryCard
            deal={deal.room}
            onAddResources={
              deal.persisted?.capabilities?.edit
                ? () => setAddingResources(true)
                : undefined
            }
          />
        )}
      </div>
    </WorkspaceMain>
  );
}
