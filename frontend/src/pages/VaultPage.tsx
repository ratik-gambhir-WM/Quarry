import { ActivityStream } from "../components/hub/ActivityStream";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

export function VaultPage() {
  return (
    <WorkspaceHomeShell activeHomeSection="vault" header={<WorkspaceHeader title="Vault" />}>
      <div className="mx-auto w-full max-w-[960px] pb-10">
        <ActivityStream />
      </div>
    </WorkspaceHomeShell>
  );
}
