import { UnderConstructionView } from "../../components/deal-room/UnderConstructionView";
import { WorkspaceMain } from "../../components/hub/WorkspaceLayout";

export function DealAnalysisPage() {
  return (
    <WorkspaceMain>
      <div className="mx-auto flex w-full max-w-[1440px] flex-col gap-6 pb-10">
        <UnderConstructionView
          description="Analysis tools and findings for this deal."
          icon="person"
          title="Analysis"
        />
      </div>
    </WorkspaceMain>
  );
}
