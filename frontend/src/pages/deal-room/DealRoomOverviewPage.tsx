import { useState } from "react";
import { AddDealModal } from "../../components/hub/sidebar/AddDealModal";
import { Button } from "../../components/ui/button";
import { DealSummaryCard } from "../../components/deal-room/DealSummaryCard";
import { InsightsStrip } from "../../components/hub/InsightsStrip";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { DealRoomHeader } from "../../components/deal-room/DealRoomHeader";
import { workspaceInsights } from "../../fixtures/workspace/portfolio";
import { useDealRoom } from "../DealRoomPage";

export function DealRoomOverviewPage() {
  const [editing, setEditing] = useState(false);
  const { activeOverviewSection, deal, email, setActiveOverviewSection } = useDealRoom();
  const dealInsights = workspaceInsights.filter((insight) => insight.deal === deal.room.name);

  return (
    <WorkspaceMain
      header={<DealRoomHeader activeSection={activeOverviewSection} onActiveSectionChange={setActiveOverviewSection} />}
    >
      {editing && deal.persisted ? <AddDealModal initialDeal={deal.persisted} email={email} onClose={() => setEditing(false)} /> : null}
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
        {deal.persisted ? <Button className="self-end" variant="outline" onClick={() => setEditing(true)}>Edit deal</Button> : null}
        {activeOverviewSection === "file-summary" ? (
          <>
            <h1 className="sr-only">{deal.room.name}</h1>
            <InsightsStrip className="mt-2" contextLabel={deal.room.name} items={dealInsights} />
          </>
        ) : (
          <DealSummaryCard deal={deal.room} />
        )}
      </div>
    </WorkspaceMain>
  );
}
