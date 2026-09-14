import { useLocation, useNavigate } from "react-router-dom";
import { DeliverableTemplatesView } from "../../components/deal-room/DeliverableTemplatesView";
import { DeliverablesHeader } from "../../components/deal-room/DeliverablesHeader";
import { TemplatePreviewProvider } from "../../components/deal-room/TemplatePreviewStore";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";
import { getDeliverableTemplatePath, getDeliverablesPath } from "../../data/workspace";
import { useDealRoom } from "../DealRoomPage";

export function DeliverableTemplatesPage() {
  const location = useLocation();
  const navigate = useNavigate();
  const { deal, navigationState } = useDealRoom();
  return (
    <TemplatePreviewProvider requestKey={location.key}>
      <WorkspaceMain
        header={
          <DeliverablesHeader
            mode="templates"
            onBack={() => navigate(getDeliverablesPath(deal.room.id), { state: navigationState })}
          />
        }
      >
        <div className="mx-auto flex h-full w-full max-w-[1440px] flex-col">
          <DeliverableTemplatesView
            onRetry={() => navigate(location.pathname, { replace: true, state: location.state })}
            onSelectSlide={(slide) => navigate(
              getDeliverableTemplatePath(deal.room.id, slide.id),
              { state: navigationState },
            )}
          />
        </div>
      </WorkspaceMain>
    </TemplatePreviewProvider>
  );
}
