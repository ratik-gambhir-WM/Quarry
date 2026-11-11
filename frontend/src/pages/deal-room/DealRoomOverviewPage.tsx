import { DealTimelineView } from "../../components/deal-room/DealTimelineView";
import { DealMembersPanel } from "../../components/deal-room/DealMembersPanel";
import { DealActivity } from "../../components/deal-room/DealActivity";
import { DealDocumentSearch } from "../../components/deal-room/DealDocumentSearch";
import { DealResourcesDialog } from "../../components/deal-room/DealResourcesDialog";
import { useState } from "react";
import { AddDealModal } from "../../components/hub/sidebar/AddDealModal";
import { Button } from "../../components/ui/stealth/button";
import { DealSummaryCard } from "../../components/deal-room/DealSummaryCard";
import { InsightsStrip } from "../../components/hub/InsightsStrip";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { DealRoomHeader } from "../../components/deal-room/DealRoomHeader";
import { workspaceInsights } from "../../fixtures/workspace/portfolio";
import { useDealRoom } from "../DealRoomPage";
import { useContentTransition } from "../../hooks/useContentTransition";

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
  const contentRef = useContentTransition(activeOverviewSection);
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
      <div ref={contentRef} className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
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
          <section className="deal-settings-page">
            <header className="deal-settings-page-header">
              <div>
                <p className="deal-settings-eyebrow">Deal room</p>
                <h1>Deal settings</h1>
                <p className="deal-settings-description">Your team, their access, and the details behind this deal.</p>
              </div>
              {deal.persisted?.capabilities?.edit ? (
                <Button variant="secondary" onClick={() => setEditing(true)}>Edit deal</Button>
              ) : null}
            </header>
            {deal.persisted ? <DealMembersPanel key={deal.room.id} deal={deal.persisted} /> : (
              <p className="deal-roster-card p-6 text-sm text-muted">Settings are available for saved deals.</p>
            )}
          </section>
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
