import { DealSummaryCard } from "../../components/deal-room/DealSummaryCard";
import { InsightsStrip } from "../../components/hub/InsightsStrip";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { DealRoomHeader } from "../../components/deal-room/DealRoomHeader";
import { workspaceInsights } from "../../fixtures/workspace/portfolio";
import { useDealRoom } from "../DealRoomPage";

export function DealRoomOverviewPage() {
  const { activeOverviewSection, deal, setActiveOverviewSection } = useDealRoom();
  const dealInsights = workspaceInsights.filter((insight) => insight.deal === deal.room.name);

  return (
    <WorkspaceMain
      header={<DealRoomHeader activeSection={activeOverviewSection} onActiveSectionChange={setActiveOverviewSection} />}
    >
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
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
