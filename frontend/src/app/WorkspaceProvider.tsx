import { createContext, useContext } from "react";
import type { ReactNode } from "react";
import type { WorkspaceDeal, WorkspaceLocationState, WorkspaceSidebarTool } from "../data/workspace";
import { workspaceInitiatives, workspaceTools } from "../fixtures/workspace/navigation";
import {
  useWorkspaceDeals,
  type WorkspaceDealsResource,
} from "../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

type WorkspaceContextValue = {
  deals: WorkspaceDeal[];
  dealsResource: WorkspaceDealsResource;
  email?: string;
  initiatives: WorkspaceSidebarTool[];
  navigationState?: WorkspaceLocationState;
  retryDeals: () => void;
  tools: WorkspaceSidebarTool[];
};

const WorkspaceContext = createContext<WorkspaceContextValue | null>(null);

export function WorkspaceProvider({ children, dataSource }: { children: ReactNode; dataSource?: string }) {
  const { email, navigationState } = useWorkspaceSession();
  const { resource, retry } = useWorkspaceDeals(dataSource);

  return (
    <WorkspaceContext.Provider
      value={{
        deals: resource.status === "success" ? resource.deals : [],
        dealsResource: resource,
        email,
        initiatives: workspaceInitiatives,
        navigationState,
        retryDeals: retry,
        tools: workspaceTools,
      }}
    >
      {children}
    </WorkspaceContext.Provider>
  );
}

export function useWorkspace() {
  const context = useContext(WorkspaceContext);
  if (!context) {
    throw new Error("Workspace state must be used within WorkspaceProvider.");
  }
  return context;
}
