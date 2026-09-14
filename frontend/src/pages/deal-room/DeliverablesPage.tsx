import { DeliverablesView } from "../../components/deal-room/DeliverablesView";
import { DeliverablesHeader } from "../../components/deal-room/DeliverablesHeader";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { useNavigate } from "react-router-dom";
import { getDeliverableTemplatesPath } from "../../data/workspace";
import { useDealRoom } from "../DealRoomPage";

export function DeliverablesPage() {
  const { deal, navigationState } = useDealRoom();
  const navigate = useNavigate();
  return (
    <WorkspaceMain
      header={
        <DeliverablesHeader
          mode="deliverables"
          onViewTemplates={() => navigate(getDeliverableTemplatesPath(deal.room.id), { state: navigationState })}
        />
      }
    >
      <div className="mx-auto flex h-full w-full max-w-[1440px] flex-col"><DeliverablesView /></div>
    </WorkspaceMain>
  );
}
