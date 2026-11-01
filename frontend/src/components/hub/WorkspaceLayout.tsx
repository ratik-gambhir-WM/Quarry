import type { ReactNode } from "react";
import { MobileWorkspaceNavigation } from "./MobileWorkspaceNavigation";

type WorkspaceLayoutProps = {
  children: ReactNode;
  contentClassName?: string;
  contentMode?: "fill" | "scroll";
  header?: ReactNode;
  sidebar: ReactNode;
};

export function WorkspaceLayout({
  children,
  contentClassName,
  contentMode = "scroll",
  header,
  sidebar,
}: WorkspaceLayoutProps) {
  return (
    <WorkspaceShell sidebar={sidebar}>
      <WorkspaceMain
        contentClassName={contentClassName}
        contentMode={contentMode}
        header={header}
      >
        {children}
      </WorkspaceMain>
    </WorkspaceShell>
  );
}

export function WorkspaceShell({
  children,
  sidebar,
}: {
  children: ReactNode;
  sidebar: ReactNode;
}) {
  return (
    <div className="workspace-shell relative flex h-dvh flex-col overflow-hidden text-on-surface">
      <div className="workspace-shell-content relative z-10 flex min-h-0 flex-1">
        <div className="contents" data-workspace-primary-sidebar>
          {sidebar}
        </div>
        {children}
      </div>
      <MobileWorkspaceNavigation />
    </div>
  );
}

export function WorkspaceMain({
  children,
  contentClassName,
  contentMode = "scroll",
  header,
}: {
  children: ReactNode;
  contentClassName?: string;
  contentMode?: "fill" | "scroll";
  header?: ReactNode;
}) {
  const resolvedContentClassName =
    contentMode === "fill"
      ? `min-h-0 flex-1 overflow-hidden p-0 ${contentClassName ?? ""}`
      : `min-h-0 flex-1 overflow-y-auto overscroll-contain ${contentClassName ?? "px-4 py-6 sm:px-8 sm:py-8"}`;

  return (
    <main id="main-content" tabIndex={-1} className="workspace-main-surface m-2 flex min-w-0 flex-1 flex-col overflow-hidden lg:ml-0">
      {header ? (
        <div className="workspace-main-rail flex min-h-10 shrink-0 items-center border-b px-4 py-1 sm:px-8">
          {header}
        </div>
      ) : null}
      <div className={resolvedContentClassName}>{children}</div>
    </main>
  );
}
