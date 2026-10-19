import { useState } from "react";
import { useWorkspace } from "../app/WorkspaceProvider";
import { QueryChatThread } from "../components/chat/QueryChat";
import { QueryChatRuntimeProvider } from "../components/chat/QueryChatRuntimeProvider";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

export function Assistant() {
  const [scope, setScope] = useState<"all" | "freeform">("all");
  const [contextTruncated, setContextTruncated] = useState(false);
  const { email = "" } = useWorkspace();

  return (
    <QueryChatRuntimeProvider
      key={`${email}:${scope}`}
      scope={scope}
      onContextTruncated={setContextTruncated}
      userEmail={email}
    >
      <WorkspaceHomeShell
        activeHomeSection="assistant"
        contentMode="fill"
        header={<><WorkspaceHeader title="Assistant" /><label className="flex items-center gap-2 text-sm">Chat scope<select className="rounded-md border border-border bg-background p-2" value={scope} onChange={(e) => setScope(e.currentTarget.value === "freeform" ? "freeform" : "all")}><option value="all">All chats</option><option value="freeform">Freeform chats</option></select></label></>}
        sidebarMode="assistant"
      >
        <QueryChatThread contextTruncated={contextTruncated} />
      </WorkspaceHomeShell>
    </QueryChatRuntimeProvider>
  );
}
