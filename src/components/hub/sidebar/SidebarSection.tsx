import { ReactNode } from "react";

type SidebarSectionProps = {
  action?: ReactNode;
  children: ReactNode;
  title: string;
};

export function SidebarSection({ action, children, title }: SidebarSectionProps) {
  return (
    <section className="mt-4 space-y-1.5">
      <div className="flex items-center justify-between px-3">
        <h2 className="text-[12px] font-medium leading-5 tracking-normal text-sidebar-muted">{title}</h2>
        {action}
      </div>
      <nav className="space-y-1">{children}</nav>
    </section>
  );
}
