import type { ReactNode } from "react";
import { useWorkspace } from "../../app/WorkspaceProvider";
import { WorkspaceLayout } from "./WorkspaceLayout";
import { WorkspaceSidebar } from "./WorkspaceSidebar";
import type { ActiveHomeSection } from "./sidebar/sidebarTypes";

type WorkspaceHomeShellProps = {
  activeHomeSection?: ActiveHomeSection;
  children: ReactNode;
  contentMode?: "fill" | "scroll";
  header: ReactNode;
};

export function useWorkspaceHomeDeals() {
  return useWorkspace().deals;
}

export function WorkspaceHomeShell({
  activeHomeSection = "hub",
  children,
  contentMode = "scroll",
  header,
}: WorkspaceHomeShellProps) {
  const { deals, dealsResource, email, initiatives, navigationState, retryDeals, tools } = useWorkspace();
  const hasDealsNotice = dealsResource.status === "error"
    || (dealsResource.status === "success" && dealsResource.source === "demo");

  return (
    <WorkspaceLayout
      contentMode={contentMode}
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
      {contentMode === "fill" ? (
        <div className="flex h-full min-h-0 flex-col">
          {hasDealsNotice ? (
            <div className="shrink-0 px-4 pt-4">
              <WorkspaceDealsResourceNotice
                dealsResource={dealsResource}
                onRetry={retryDeals}
              />
            </div>
          ) : null}
          <div className={`min-h-0 flex-1 ${hasDealsNotice ? "pt-4" : ""}`}>{children}</div>
        </div>
      ) : (
        <>
          <WorkspaceDealsResourceNotice dealsResource={dealsResource} onRetry={retryDeals} />
          {hasDealsNotice ? <div className="mt-4">{children}</div> : children}
        </>
      )}
    </WorkspaceLayout>
  );
}

function WorkspaceDealsResourceNotice({
  dealsResource,
  onRetry,
}: {
  dealsResource: ReturnType<typeof useWorkspace>["dealsResource"];
  onRetry: () => void;
}) {
  return dealsResource.status === "error" ? (
    <WorkspaceDealsNotice
      actionLabel="Retry"
      message={`Workspace deals could not be loaded. ${dealsResource.message}`}
      onAction={onRetry}
      role="alert"
    />
  ) : dealsResource.status === "success" && dealsResource.source === "demo" ? (
    <WorkspaceDealsNotice message="Demo data" role="status" />
  ) : null;
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
