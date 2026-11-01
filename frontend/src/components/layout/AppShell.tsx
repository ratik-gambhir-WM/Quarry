import { ReactNode } from "react";
import { AppFooter } from "./AppFooter";

type AppShellProps = {
  centered?: boolean;
  children: ReactNode;
  showFooter?: boolean;
};

export function AppShell({
  centered = false,
  children,
  showFooter = true,
}: AppShellProps) {
  return (
    <div className="min-h-dvh bg-background text-on-surface">
      <main
        id="main-content"
        tabIndex={-1}
        className={`relative overflow-hidden px-gutter py-margin ${
          centered
            ? `flex ${showFooter ? "min-h-[calc(100dvh-89px)]" : "min-h-dvh"} items-center justify-center`
            : showFooter
              ? "min-h-[calc(100dvh-89px)]"
              : "min-h-dvh"
        }`}
      >
        <div className="relative z-10 w-full">{children}</div>
      </main>
      {showFooter ? <AppFooter /> : null}
    </div>
  );
}
