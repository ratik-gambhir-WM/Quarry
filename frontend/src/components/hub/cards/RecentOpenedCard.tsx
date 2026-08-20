import { Icon } from "../../ui/Icon";
import { WorkspaceCard } from "../WorkspaceCard";

type RecentFile = {
  deal: string;
  icon: "doc" | "pdf" | "sheet";
  time: string;
  title: string;
  tone: "accent" | "error" | "primary";
};

type RecentOpenedCardProps = {
  items: readonly RecentFile[];
  layout?: "rail" | "wide";
};

export function RecentOpenedCard({ items, layout = "rail" }: RecentOpenedCardProps) {
  if (layout === "wide") {
    return (
      <WorkspaceCard className="relative col-span-12 flex min-h-[430px] flex-col overflow-hidden p-8 lg:col-span-8">
        <div className="absolute -right-16 -top-14 h-64 w-64 rounded-full bg-primary/6 blur-3xl [html[data-theme=dark]_&]:opacity-0" />

        <div className="relative z-10 flex h-full flex-col">
          <div className="mb-7 flex items-center justify-between">
            <div className="flex items-center gap-4">
              <div className="flex h-16 w-16 items-center justify-center rounded-[1.4rem] bg-primary/10 text-primary">
                <Icon className="h-8 w-8" name="folderOpen" />
              </div>
              <div>
                <h2 className="type-h2 text-text-main">Recently Opened</h2>
                <p className="text-[12px] font-medium uppercase tracking-[0.12em] text-muted">
                  {items.length} Recent Documents
                </p>
              </div>
            </div>

            <button className="text-muted transition hover:text-text-main" type="button">
              <Icon className="h-5 w-5" name="filter" />
            </button>
          </div>

          <div className="grid flex-1 grid-cols-1 gap-4 sm:grid-cols-2">
            {items.map((item) => (
              <RecentFileTile item={item} key={item.title} />
            ))}
          </div>

          <div className="mt-8 border-t border-white/55 pt-6">
            <button className="inline-flex items-center gap-3 text-xs font-bold uppercase tracking-[0.18em] text-primary transition hover:gap-4">
              View All Updates
              <Icon className="h-4 w-4" name="arrowRight" />
            </button>
          </div>
        </div>
      </WorkspaceCard>
    );
  }

  return (
    <WorkspaceCard className="col-span-12 flex min-h-[640px] flex-col p-6 lg:col-span-4 lg:row-span-2">
      <div className="mb-6 flex items-center justify-between">
        <h2 className="type-h3 text-text-main">Recently Opened</h2>
        <button className="text-muted transition hover:text-text-main" type="button">
          <Icon className="h-5 w-5" name="filter" />
        </button>
      </div>

      <div className="workspace-scrollbar-hidden flex-1 overflow-y-auto pr-2">
        <div className="space-y-4">
          {items.map((item) => (
            <RecentFileRow item={item} key={item.title} />
          ))}
        </div>
      </div>

      <button className="mt-5 rounded-xl border border-white/80 bg-white/55 py-3 text-sm font-semibold text-text-main shadow-sm transition hover:bg-white">
        View All Updates
      </button>
    </WorkspaceCard>
  );
}

function RecentFileIcon({ item }: { item: RecentFile }) {
  const toneClassName =
    item.tone === "error"
      ? "bg-error/10 text-error"
      : item.tone === "accent"
        ? "bg-accent/10 text-accent"
        : "bg-primary/10 text-primary";

  return (
    <div className={`flex h-16 w-16 shrink-0 items-center justify-center rounded-2xl ${toneClassName}`}>
      <Icon className="h-8 w-8" name={item.icon} />
    </div>
  );
}

function RecentFileRow({ item }: { item: RecentFile }) {
  return (
    <div className="group flex cursor-pointer items-center gap-4 rounded-xl p-2 transition hover:bg-white/40">
      <RecentFileIcon item={item} />
      <div className="min-w-0 flex-1">
        <p className="truncate text-[17px] font-semibold text-text-main">{item.title}</p>
        <p className="truncate text-[12px] font-medium uppercase tracking-[0.1em] text-muted">
          {item.deal} • {item.time}
        </p>
      </div>
    </div>
  );
}

function RecentFileTile({ item }: { item: RecentFile }) {
  return (
    <article className="group flex min-h-[118px] cursor-pointer items-center gap-4 rounded-[1.35rem] border border-white/60 bg-white/40 p-5 transition hover:bg-white/60">
      <RecentFileIcon item={item} />
      <div className="min-w-0 flex-1">
        <p className="line-clamp-2 text-[17px] font-semibold leading-snug text-text-main">{item.title}</p>
        <p className="mt-2 truncate text-[12px] font-medium uppercase tracking-[0.1em] text-muted">
          {item.deal} • {item.time}
        </p>
      </div>
      <Icon className="h-4 w-4 shrink-0 text-muted opacity-0 transition group-hover:opacity-100" name="openInNew" />
    </article>
  );
}
