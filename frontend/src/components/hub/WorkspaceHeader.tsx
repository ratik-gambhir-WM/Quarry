import type { ReactNode } from "react";

type WorkspaceHeaderProps = {
  actions?: ReactNode;
  afterTitle?: ReactNode;
  breadcrumbs?: ReactNode;
  title: string;
};

export function WorkspaceHeader({ actions, afterTitle, breadcrumbs, title }: WorkspaceHeaderProps) {
  return (
    <header className="flex min-w-0 flex-1 items-center justify-between gap-4">
      <div className="flex min-w-0 items-center gap-2">
        {breadcrumbs ? <div className="min-w-0 text-[12px] text-muted">{breadcrumbs}</div> : null}
        <h1 className="truncate text-[14px] font-medium leading-5 tracking-[-0.01em] text-text-main [font-family:var(--font-heading)]">
          {title}
        </h1>
        {afterTitle}
      </div>
      {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
    </header>
  );
}
