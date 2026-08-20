import { createContext, useContext } from "react";
import type { ReactNode } from "react";
import { workspaceInitiatives, workspaceTools, type WorkspaceDeal } from "../../data/workspace";
import { useWorkspaceDeals } from "../../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../../hooks/useWorkspaceSession";
import { WorkspaceLayout } from "./WorkspaceLayout";
import { WorkspaceSidebar } from "./WorkspaceSidebar";

type WorkspaceHomeShellProps = {
  activeHomeSection?: "account" | "hub" | "logs" | "summarize" | "vault";
  children: ReactNode;
  header: ReactNode;
};

const WorkspaceDealsContext = createContext<WorkspaceDeal[]>([]);

export function useWorkspaceHomeDeals() {
  return useContext(WorkspaceDealsContext);
}

export function WorkspaceHomeShell({ activeHomeSection = "hub", children, header }: WorkspaceHomeShellProps) {
  const { email, navigationState } = useWorkspaceSession();
  const { deals } = useWorkspaceDeals();

  return (
    <WorkspaceDealsContext.Provider value={deals}>
      <WorkspaceLayout
        header={header}
        sidebar={
          <WorkspaceSidebar
            activeHomeSection={activeHomeSection}
            deals={deals}
            email={email}
            initiatives={workspaceInitiatives}
            navigationState={navigationState}
            tools={workspaceTools}
          />
        }
      >
        {children}
      </WorkspaceLayout>
    </WorkspaceDealsContext.Provider>
  );
}
