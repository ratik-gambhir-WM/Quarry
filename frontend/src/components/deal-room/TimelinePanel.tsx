import type { ReactNode } from "react";
import { cn } from "../../lib/utils";
import { Icon } from "../ui/Icon";

type TimelinePanelHeaderProps = {
  className?: string;
  title: string;
};

type TimelineEntryProps = {
  children: ReactNode;
  marker: ReactNode;
};

export function TimelinePanelHeader({ className, title }: TimelinePanelHeaderProps) {
  return (
    <div className={cn("mb-7 flex items-center justify-between", className)}>
      <h2 className="type-h2 text-text-main">{title}</h2>
      <button
        aria-label={`More ${title.toLocaleLowerCase()} actions`}
        className="inline-flex h-10 w-10 items-center justify-center rounded-full transition hover:bg-white/50"
        type="button"
      >
        <Icon className="h-5 w-5 text-muted" name="more" />
      </button>
    </div>
  );
}

export function TimelineList({ children }: { children: ReactNode }) {
  return (
    <div className="workspace-scrollbar-hidden relative flex-1 overflow-y-auto pr-2">
      <div className="absolute bottom-6 left-4 top-4 w-px bg-muted/30" />
      <div className="relative z-10 space-y-7">{children}</div>
    </div>
  );
}

export function TimelineEntry({ children, marker }: TimelineEntryProps) {
  return (
    <article className="flex gap-4">
      {marker}
      <div className="space-y-2 pb-1">{children}</div>
    </article>
  );
}

export function TimelineMarker({ children }: { children: ReactNode }) {
  return (
    <div className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-white/90 bg-white/85 shadow-[0_8px_18px_rgba(7,1,84,0.06)]">
      {children}
    </div>
  );
}

export function TimelinePanelAction({ children }: { children: ReactNode }) {
  return (
    <button
      className="mt-5 rounded-2xl border border-white/85 bg-white/55 px-4 py-3 text-[15px] font-semibold text-text-main transition hover:bg-white/75"
      type="button"
    >
      {children}
    </button>
  );
}
