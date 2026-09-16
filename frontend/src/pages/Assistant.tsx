import { QueryChat } from "../components/chat/QueryChat";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

export function Assistant() {
  return (
    <WorkspaceHomeShell
      activeHomeSection="assistant"
      contentMode="fill"
      header={<WorkspaceHeader title="Summarize" />}
    >
      <QueryChat />
    </WorkspaceHomeShell>
  );
}
