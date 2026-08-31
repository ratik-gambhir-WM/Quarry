import type { ReactNode } from "react";

type WorkspaceLayoutProps = {
  children: ReactNode;
  header?: ReactNode;
  sidebar: ReactNode;
};

export function WorkspaceLayout({ children, header, sidebar }: WorkspaceLayoutProps) {
  return (
    <div className="workspace-shell relative h-screen overflow-hidden text-on-surface">
      <div className="relative z-10 flex h-full min-h-0">
        {sidebar}

        <main className="workspace-main-surface m-2 flex min-w-0 flex-1 flex-col overflow-hidden lg:ml-0">
          {header ? (
            <div className="workspace-main-rail flex h-10 shrink-0 items-center border-b px-6 sm:px-8">{header}</div>
          ) : null}
          <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-y-auto px-8 py-8">{children}</div>
        </main>
      </div>
    </div>
  );
}
