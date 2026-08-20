import type { ReactNode } from "react";

type WorkspaceLayoutProps = {
  children: ReactNode;
  header?: ReactNode;
  sidebar: ReactNode;
};

export function WorkspaceLayout({ children, header, sidebar }: WorkspaceLayoutProps) {
  return (
    <div className="relative h-screen overflow-hidden bg-background text-on-surface">
      <div className="relative z-10 flex h-full min-h-0">
        {sidebar}

        <main className="flex min-w-0 flex-1 flex-col">
          {header ? (
            <div className="flex h-16 shrink-0 items-center border-b border-outline-variant/70 px-8">{header}</div>
          ) : null}
          <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-y-auto px-8 py-8">{children}</div>
        </main>
      </div>
    </div>
  );
}
