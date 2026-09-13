import type { ReactNode } from "react";

type WorkspaceLayoutProps = {
  children: ReactNode;
  contentClassName?: string;
  header?: ReactNode;
  sidebar: ReactNode;
};

export function WorkspaceLayout({ children, contentClassName = "px-8 py-8", header, sidebar }: WorkspaceLayoutProps) {
  return (
    <WorkspaceShell sidebar={sidebar}>
      <WorkspaceMain contentClassName={contentClassName} header={header}>{children}</WorkspaceMain>
    </WorkspaceShell>
  );
}

export function WorkspaceShell({ children, sidebar }: { children: ReactNode; sidebar: ReactNode }) {
  return (
    <div className="workspace-shell relative h-screen overflow-hidden text-on-surface">
      <div className="workspace-shell-content relative z-10 flex h-full min-h-0">
        <div className="contents" data-workspace-primary-sidebar>{sidebar}</div>
        {children}
      </div>
    </div>
  );
}

export function WorkspaceMain({
  children,
  contentClassName = "px-8 py-8",
  header,
}: {
  children: ReactNode;
  contentClassName?: string;
  header?: ReactNode;
}) {
  return (
    <main className="workspace-main-surface m-2 flex min-w-0 flex-1 flex-col overflow-hidden lg:ml-0">
      {header ? (
        <div className="workspace-main-rail flex h-10 shrink-0 items-center border-b px-6 sm:px-8">{header}</div>
      ) : null}
      <div className={`workspace-scrollbar-hidden min-h-0 flex-1 overflow-y-auto ${contentClassName}`}>{children}</div>
    </main>
  );
}
