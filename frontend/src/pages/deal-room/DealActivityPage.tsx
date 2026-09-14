import { DealTimelineView } from "../../components/deal-room/DealTimelineView";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { useDealRoom } from "../DealRoomPage";

export function DealActivityPage() {
  const { deal, setTimelineItems, timelineItems } = useDealRoom();
  return (
    <WorkspaceMain>
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
        <DealTimelineView deal={deal.room} events={timelineItems} onEventsChange={setTimelineItems} />
      </div>
    </WorkspaceMain>
  );
}
