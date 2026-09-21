import { useState } from "react";
import { QueryChatThread } from "../components/chat/QueryChat";
import { QueryChatRuntimeProvider } from "../components/chat/QueryChatRuntimeProvider";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

export function Assistant() {
  const [contextTruncated, setContextTruncated] = useState(false);

  return (
    <QueryChatRuntimeProvider onContextTruncated={setContextTruncated}>
      <WorkspaceHomeShell
        activeHomeSection="assistant"
        contentMode="fill"
        header={<WorkspaceHeader title="Summarize" />}
        sidebarMode="assistant"
      >
        <QueryChatThread contextTruncated={contextTruncated} />
      </WorkspaceHomeShell>
    </QueryChatRuntimeProvider>
  );
}
