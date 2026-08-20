import { ReactNode } from "react";
import { AppFooter } from "./AppFooter";

type AppShellProps = {
  centered?: boolean;
  children: ReactNode;
  showFooter?: boolean;
};

export function AppShell({ centered = false, children, showFooter = true }: AppShellProps) {
  return (
    <div className="min-h-screen bg-background text-on-surface">
      <main
        className={`relative overflow-hidden px-gutter py-margin ${
          centered
            ? `flex ${showFooter ? "min-h-[calc(100vh-89px)]" : "min-h-screen"} items-center justify-center`
            : showFooter
              ? "min-h-[calc(100vh-89px)]"
              : "min-h-screen"
        }`}
      >
        <div className="workspace-ambient pointer-events-none absolute inset-0 overflow-hidden">
          <div className="absolute right-[-10%] top-[-12%] h-[38rem] w-[38rem] rounded-full bg-surface-container-high/90 blur-3xl" />
          <div className="absolute bottom-[-12%] left-[-8%] h-[28rem] w-[28rem] rounded-full bg-primary-fixed/30 blur-3xl" />
          <div className="absolute left-1/2 top-20 h-48 w-48 -translate-x-1/2 rounded-full bg-tertiary-fixed/30 blur-3xl" />
        </div>
        <div className="relative z-10 w-full">{children}</div>
      </main>
      {showFooter ? <AppFooter /> : null}
    </div>
  );
}
