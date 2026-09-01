import type { ReactNode } from "react";
import { workspaceInsights } from "../../data/workspace";
import { Icon } from "../ui/Icon";
import { WorkspaceCard } from "./WorkspaceCard";

const tasks = [
  {
    checked: true,
    label: "Finalize Q3 Financial Extract for Project Alpha",
    tag: { tone: "error" as const, value: "High Priority" },
  },
  {
    checked: false,
    label: "Review legal disclosures for Logistics Merger",
    tag: { tone: "success" as const, value: "Due Today" },
  },
  {
    checked: false,
    label: "Approve analyst transcript summary: CEO Site Visit",
    tag: { tone: "muted" as const, value: "Alpha" },
  },
  {
    checked: false,
    label: "Initialize Data Room for Project Gamma",
    tag: { tone: "icon" as const, value: "more" as const },
  },
] as const;

const recentFiles = [
  {
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "2m ago",
    title: "Q3 Financial Report.pdf",
    tone: "error" as const,
  },
  {
    deal: "Project Beta",
    icon: "doc" as const,
    time: "1h ago",
    title: "Meeting Minutes - Legal Review.doc",
    tone: "accent" as const,
  },
  {
    deal: "Logistics Merger",
    icon: "sheet" as const,
    time: "3h ago",
    title: "Logistics_Due_Diligence.xlsx",
    tone: "primary" as const,
  },
  {
    deal: "Project Alpha",
    icon: "pdf" as const,
    time: "Yesterday",
    title: "Environmental Impact Study.pdf",
    tone: "error" as const,
  },
] as const;

const activityFilters = ["All", "Tasks", "Docs", "Insights"] as const;

export function ActivityStream() {
  return (
    <section aria-labelledby="activity-stream-heading">
      <div className="flex items-center justify-between gap-4 border-b border-white/55 pb-3">
        <h2
          className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted"
          id="activity-stream-heading"
        >
          Activity Stream
        </h2>

        <div className="flex items-center gap-1">
          {activityFilters.map((filter, index) => (
            <button
              aria-pressed={index === 0}
              className={
                "rounded-sm px-2 py-1 text-[10px] font-bold uppercase tracking-[0.08em] transition " +
                (index === 0
                  ? "bg-primary text-white"
                  : "bg-surface-container-low text-muted hover:bg-surface-container-high")
              }
              key={filter}
              type="button"
            >
              {filter}
            </button>
          ))}
        </div>
      </div>

      <div className="mt-4 flex flex-col gap-4">
        <StreamTaskCard item={tasks[0]} />
        <StreamInsightCard item={workspaceInsights[0]} />
        <StreamFileCard item={recentFiles[1]} />
        <StreamTaskCard item={tasks[1]} />
        <StreamInsightCard item={workspaceInsights[1]} />
        <StreamFileCard item={recentFiles[0]} />
        <StreamTaskCard item={tasks[2]} />
        <StreamFileCard item={recentFiles[2]} />
        <StreamTaskCard item={tasks[3]} />
      </div>
    </section>
  );
}

function StreamTaskCard({ item }: { item: (typeof tasks)[number] }) {
  const toneClassName =
    item.tag.tone === "error"
      ? "bg-error"
      : item.tag.tone === "success"
        ? "bg-primary"
        : item.tag.tone === "muted"
          ? "bg-muted"
          : "bg-accent";

  return (
    <StreamCard toneClassName={toneClassName}>

      <div className="flex items-center justify-between gap-4 pl-2">
        <div className="flex min-w-0 items-center gap-2 text-[11px] font-bold uppercase tracking-[0.16em] text-muted">
          <Icon className="h-4 w-4 shrink-0 text-accent" name="checkCircle" />
          <span className="truncate">{item.tag.tone === "error" ? "Critical Task" : "Task"}</span>
          {item.tag.tone !== "error" ? <span>•</span> : null}
          {item.tag.tone !== "error" ? <span className="truncate">{item.tag.value}</span> : null}
        </div>

        {item.tag.tone === "icon" ? (
          <Icon className="h-4 w-4 shrink-0 text-muted" name="more" />
        ) : (
          <span
            className={
              "shrink-0 rounded-sm px-2 py-1 text-[10px] font-bold uppercase tracking-[0.08em] " +
              (item.tag.tone === "error" ? "bg-error/10 text-error" : "bg-primary/10 text-primary")
            }
          >
            {item.tag.value}
          </span>
        )}
      </div>

      <div className="mt-3 flex items-start gap-3 pl-2">
        <button
          aria-label={item.checked ? "Completed task" : "Incomplete task"}
          className={
            "mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition " +
            (item.checked
              ? "border-primary bg-primary text-white"
              : "border-outline-variant bg-white/70 text-transparent")
          }
          type="button"
        >
          <Icon className="h-3 w-3" name="check" />
        </button>
        <p className="min-w-0 flex-1 text-[16px] font-semibold leading-snug text-text-main">{item.label}</p>
      </div>

      <p className="mt-1 pl-10 text-[12px] text-muted">Assigned to: JD, AT</p>
    </StreamCard>
  );
}

function StreamInsightCard({ item }: { item: (typeof workspaceInsights)[number] }) {
  return (
    <StreamCard toneClassName={item.toneClassName}>

      <div className="flex items-center justify-between gap-4 pl-2">
        <div className="flex min-w-0 items-center gap-2 text-[11px] font-bold uppercase tracking-[0.16em] text-muted">
          <Icon className="h-4 w-4 shrink-0 text-muted" name="bookmark" />
          <span className="truncate">Recent Insight • {item.deal}</span>
        </div>
        <span className="shrink-0 rounded-sm bg-surface-container-low px-2 py-1 text-[10px] font-bold uppercase tracking-[0.08em] text-muted">
          {item.category}
        </span>
      </div>

      <p className="mt-4 pl-2 font-heading text-[1.08rem] leading-snug text-text-main">{item.quote}</p>

      <div className="mt-4 flex items-center gap-2 rounded-md bg-surface-container-low px-3 py-2 text-[11px] font-medium uppercase tracking-[0.1em] text-muted">
        <Icon className="h-4 w-4 shrink-0" name={item.fileIcon} />
        <span className="truncate">Source: {item.fileName}</span>
      </div>
    </StreamCard>
  );
}

function StreamFileCard({ item }: { item: (typeof recentFiles)[number] }) {
  const toneClassName = item.tone === "error" ? "bg-error" : item.tone === "accent" ? "bg-accent" : "bg-primary";
  const iconToneClassName =
    item.tone === "error"
      ? "bg-error/10 text-error"
      : item.tone === "accent"
        ? "bg-accent/10 text-accent"
        : "bg-primary/10 text-primary";

  return (
    <StreamCard toneClassName={toneClassName}>

      <div className="flex items-center justify-between gap-4 pl-2 text-[11px] font-bold uppercase tracking-[0.16em] text-muted">
        <div className="flex min-w-0 items-center gap-2">
          <Icon className="h-4 w-4 shrink-0 text-primary" name="doc" />
          <span className="truncate">Recently Opened • {item.time}</span>
        </div>
        <span className="truncate">{item.deal}</span>
      </div>

      <div className="mt-3 flex items-center gap-3 pl-2">
        <div className={"flex h-10 w-10 shrink-0 items-center justify-center rounded-md " + iconToneClassName}>
          <Icon className="h-5 w-5" name={item.icon} />
        </div>
        <p className="min-w-0 truncate text-[16px] font-semibold text-text-main">{item.title}</p>
      </div>
    </StreamCard>
  );
}

function StreamCard({ children, toneClassName }: { children: ReactNode; toneClassName: string }) {
  return (
    <WorkspaceCard className="relative overflow-hidden p-4 sm:p-5">
      <div className={`absolute inset-y-0 left-0 w-1 ${toneClassName}`} />
      {children}
    </WorkspaceCard>
  );
}
