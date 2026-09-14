import type { ReactNode } from "react";
import { useWorkspace } from "../../app/WorkspaceProvider";
import { WorkspaceLayout } from "./WorkspaceLayout";
import { WorkspaceSidebar } from "./WorkspaceSidebar";
import type { ActiveHomeSection } from "./sidebar/sidebarTypes";

type WorkspaceHomeShellProps = {
  activeHomeSection?: ActiveHomeSection;
  children: ReactNode;
  header: ReactNode;
};

export function useWorkspaceHomeDeals() {
  return useWorkspace().deals;
}

export function WorkspaceHomeShell({ activeHomeSection = "hub", children, header }: WorkspaceHomeShellProps) {
  const { deals, dealsResource, email, initiatives, navigationState, retryDeals, tools } = useWorkspace();
  const hasDealsNotice = dealsResource.status === "error"
    || (dealsResource.status === "success" && dealsResource.source === "demo");

  return (
    <WorkspaceLayout
      header={header}
      sidebar={
        <WorkspaceSidebar
          activeHomeSection={activeHomeSection}
          deals={deals}
          email={email}
          initiatives={initiatives}
          navigationState={navigationState}
          tools={tools}
        />
      }
    >
      {dealsResource.status === "error" ? (
        <WorkspaceDealsNotice
          actionLabel="Retry"
          message={`Workspace deals could not be loaded. ${dealsResource.message}`}
          onAction={retryDeals}
          role="alert"
        />
      ) : dealsResource.status === "success" && dealsResource.source === "demo" ? (
        <WorkspaceDealsNotice message="Demo data" role="status" />
      ) : null}
      {hasDealsNotice ? <div className="mt-4">{children}</div> : children}
    </WorkspaceLayout>
  );
}

function WorkspaceDealsNotice({
  actionLabel,
  message,
  onAction,
  role,
}: {
  actionLabel?: string;
  message: string;
  onAction?: () => void;
  role: "alert" | "status";
}) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg border border-outline-variant bg-surface-container-lowest px-4 py-3 text-[13px] text-text-main" role={role}>
      <span>{message}</span>
      {actionLabel && onAction ? (
        <button className="font-semibold text-primary hover:underline" onClick={onAction} type="button">
          {actionLabel}
        </button>
      ) : null}
    </div>
  );
}
