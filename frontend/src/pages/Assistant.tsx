import { useState } from "react";
import { useLocation } from "react-router-dom";
import { useWorkspace } from "../app/WorkspaceProvider";
import { QueryChatThread } from "../components/chat/QueryChat";
import { QueryChatRuntimeProvider } from "../components/chat/QueryChatRuntimeProvider";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";

export function Assistant() {
  const [scope, setScope] = useState<"all" | "freeform">("all");
  const [contextTruncated, setContextTruncated] = useState(false);
  const { email = "" } = useWorkspace();
  const { state }: { state: unknown } = useLocation();
  const initialDraft =
    state && typeof state === "object" && "assistantDraft" in state &&
    typeof state.assistantDraft === "string" ? state.assistantDraft : "";

  return (
    <QueryChatRuntimeProvider
      key={`${email}:${scope}`}
      scope={scope}
      onContextTruncated={setContextTruncated}
      userEmail={email}
      initialDraft={initialDraft}
    >
      <WorkspaceHomeShell
        activeHomeSection="assistant"
        contentMode="fill"
        header={
          <>
            <WorkspaceHeader title="Assistant" />
            <label className="flex shrink-0 items-center gap-2 text-[12px] text-muted">
              <span className="sr-only sm:not-sr-only">Chat scope</span>
              <select
                className="h-8 max-w-40 rounded-lg border border-border bg-surface-container-lowest px-2 text-[16px] text-text-main sm:text-[13px]"
                value={scope}
                onChange={(e) =>
                  setScope(
                    e.currentTarget.value === "freeform" ? "freeform" : "all",
                  )
                }
              >
                <option value="all">All chats</option>
                <option value="freeform">Freeform chats</option>
              </select>
            </label>
          </>
        }
        sidebarMode="assistant"
      >
        <QueryChatThread contextTruncated={contextTruncated} />
      </WorkspaceHomeShell>
    </QueryChatRuntimeProvider>
  );
}
