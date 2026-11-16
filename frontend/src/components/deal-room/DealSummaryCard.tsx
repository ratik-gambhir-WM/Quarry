import type { CSSProperties } from "react";
import { CalendarDays, ChartNoAxesColumnIncreasing, CircleAlert, ClipboardList, Files, SlidersHorizontal } from "lucide-react";
import { overviewDashboardPreview } from "../../fixtures/deal-room/overviewDashboard";
import type { DealResource, DealRoomData } from "../../data/workspace";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { WorkstreamStatusBadge } from "./WorkstreamStatusBadge";
import { Separator } from "../ui/separator";

type DealSummaryCardProps = {
  deal: DealRoomData;
  onAddResources?: () => void;
  onEdit?: () => void;
};

const overviewCardStyle: CSSProperties = {
  background: "linear-gradient(var(--theme-surface-muted), transparent)",
  border: 0,
  borderRadius: 8,
  boxShadow: "none",
};

export function DealSummaryCard({
  deal,
  onAddResources,
  onEdit,
}: DealSummaryCardProps) {
  return (
    <div className="grid min-w-0 grid-cols-12 gap-4 lg:gap-5">
      <WorkspaceCard
        aria-labelledby="deal-overview-heading"
        className="col-span-12 p-5 sm:p-6"
        interactive={false}
        style={overviewCardStyle}
      >
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0 flex-1">
            <p className="type-label uppercase tracking-[0.14em] text-muted">Deal overview</p>
            <h1 className="type-display mt-2 break-words text-text-main" id="deal-overview-heading">{deal.name}</h1>
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge variant="default">{deal.stageLabel}</Badge>
              <Badge variant="secondary">{deal.phaseLabel}</Badge>
              <Badge variant="outline">{deal.sectorLabel}</Badge>
            </div>
          </div>
          {onEdit ? (
            <Button type="button" variant="outline" onClick={onEdit}>
              <SlidersHorizontal aria-hidden="true" className="size-4" />
              Edit deal
            </Button>
          ) : null}
        </div>
        <Separator className="my-5 bg-outline-variant/70" />
        {deal.summaryDetails?.length ? (
          <dl aria-label="Engagement overview" className="grid gap-x-8 gap-y-4 sm:grid-cols-2 lg:grid-cols-4">
            {deal.summaryDetails.map((detail, index) => (
              <div className={`min-w-0 ${detail.label === "Scope" ? "sm:col-span-2" : ""}`} key={`${detail.label}-${index}`}>
                <dt className="text-xs text-muted">{detail.label}</dt>
                <dd className="mt-1 break-words text-sm leading-6 text-text-main">{detail.value}</dd>
              </div>
            ))}
          </dl>
        ) : (
          <p className="max-w-[80ch] text-sm leading-6 text-text-main/86">{deal.summary}</p>
        )}
      </WorkspaceCard>

      <div className="col-span-12 flex flex-wrap items-center gap-2 text-xs text-muted">
        <Badge variant="outline">Sample data</Badge>
        <p>Metrics, assessment progress, actions, and milestones are illustrative. Deal details and resources reflect this deal.</p>
      </div>
      <section aria-label="Illustrative assessment metrics" className="col-span-12 grid grid-cols-2 gap-4 lg:grid-cols-4">
        {[
          { label: "Documents", value: overviewDashboardPreview.documents, detail: "In data room", icon: Files },
          { label: "Findings", value: overviewDashboardPreview.findings, detail: "Identified", icon: ClipboardList },
          { label: "Open items", value: overviewDashboardPreview.openItems, detail: "Awaiting resolution", icon: CircleAlert },
          { label: "Progress", value: `${overviewDashboardPreview.progress}%`, detail: "Assessment complete", icon: ChartNoAxesColumnIncreasing },
        ].map(({ label, value, detail, icon: MetricIcon }) => (
          <WorkspaceCard key={label} aria-label={`${label}: ${value} (sample)`} className="min-w-0 p-4 sm:p-5" style={overviewCardStyle}>
            <div className="flex items-center justify-between gap-2 text-muted">
              <span className="text-sm">{label}</span><MetricIcon aria-hidden="true" className="size-4 shrink-0" />
            </div>
            <p className="mt-4 text-3xl tabular-nums text-text-main">{value}</p>
            <p className="mt-2 text-xs leading-5 text-muted">{detail}</p>
          </WorkspaceCard>
        ))}
      </section>

      <WorkspaceCard aria-labelledby="assessment-progress-heading" className="col-span-12 min-w-0 p-5 sm:p-6 lg:col-span-8" style={overviewCardStyle}>
        <div className="mb-6 flex flex-wrap items-center justify-between gap-2">
          <h2 className="type-h3 text-text-main" id="assessment-progress-heading">Assessment Progress</h2>
          <span className="text-xs text-muted">By scope · Sample</span>
        </div>
        <ul aria-label="Sample assessment status by scope" className="divide-y divide-outline-variant/70">
          {overviewDashboardPreview.workstreams.map((workstream) => (
            <li key={workstream.name} className="flex items-center justify-between gap-3 py-3 first:pt-0 last:pb-0">
              <span className="min-w-0 break-words text-sm text-text-main">{workstream.name}</span>
              <WorkstreamStatusBadge stage={workstream.stage} />
            </li>
          ))}
        </ul>
      </WorkspaceCard>

      <WorkspaceCard aria-labelledby="deal-resources-heading" className="col-span-12 min-w-0 p-5 sm:p-6 lg:col-span-4" style={overviewCardStyle}>
        <h2 className="type-h3 text-text-main" id="deal-resources-heading">Deal Resources</h2>
        <div className="mt-4 space-y-1" role="list">
          {deal.resources.map((resource) => <ResourceRow key={resource.id} resource={resource} />)}
        </div>
        {onAddResources ? (
          <Button className="mt-4 w-full" type="button" variant="outline" onClick={onAddResources}>Manage resources</Button>
        ) : null}
      </WorkspaceCard>

      <WorkspaceCard aria-labelledby="actions-heading" className="col-span-12 min-w-0 p-5 sm:p-6 lg:col-span-8" style={overviewCardStyle}>
        <div className="mb-4 flex flex-wrap items-center justify-between gap-2">
          <h2 className="type-h3 text-text-main" id="actions-heading">Action Required</h2>
          <span className="text-xs text-muted">Outstanding items · Sample</span>
        </div>
        <ul className="divide-y divide-outline-variant/70">
          {overviewDashboardPreview.actions.map((action) => (
            <li key={action.title} className="flex items-start justify-between gap-3 py-3">
              <div className="min-w-0"><p className="break-words text-sm text-text-main">{action.title}</p><p className="mt-1 text-xs text-muted">{action.detail}</p></div>
              <Badge className="shrink-0" variant="outline">{action.priority}</Badge>
            </li>
          ))}
        </ul>
      </WorkspaceCard>

      <WorkspaceCard aria-labelledby="milestones-heading" className="col-span-12 min-w-0 p-5 sm:p-6 lg:col-span-4" style={overviewCardStyle}>
        <div className="mb-4 flex items-center justify-between gap-2">
          <h2 className="type-h3 text-text-main" id="milestones-heading">Upcoming Milestones</h2>
          <CalendarDays aria-hidden="true" className="size-4 shrink-0 text-muted" />
        </div>
        <ol className="grid gap-4" aria-label="Sample milestones">
          {overviewDashboardPreview.milestones.map((milestone) => (
            <li key={milestone.title} className="flex items-start gap-3">
              <span className="shrink-0 rounded-md bg-surface-container-lowest px-2 py-1 text-xs text-muted">{milestone.date}</span>
              <div className="min-w-0"><p className="break-words text-sm text-text-main">{milestone.title}</p><p className="mt-1 text-xs text-muted">{milestone.detail}</p></div>
            </li>
          ))}
        </ol>
        <p className="mt-4 text-xs text-muted">Illustrative schedule</p>
      </WorkspaceCard>
    </div>
  );
}

function ResourceRow({ resource }: { resource: DealResource }) {
  const content = (
    <>
      <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-surface-container-lowest text-primary">
        <Icon
          className="size-4"
          name={resource.id === "sharepoint" ? "sharepoint" : "doc"}
        />
      </span>
      <span className="min-w-0 flex-1 text-left">
        <span className="flex min-w-0 items-center justify-between gap-3">
          <span className="min-w-0 break-words text-[13px] font-semibold text-text-main">
            {resource.label}
          </span>
          <Badge
            className="shrink-0"
            variant={
              resource.availability === "available" ? "secondary" : "outline"
            }
          >
            {resource.availability === "available"
              ? resource.href
                ? "Available"
                : "Submitted"
              : resource.availability === "coming-soon"
                ? "Coming soon"
                : "Unavailable"}
          </Badge>
        </span>
        {resource.sourceName ? (
          <span className="mt-0.5 block break-words text-[11px] font-normal text-muted">
            {resource.sourceName}
          </span>
        ) : null}
      </span>
      {resource.href ? (
        <Icon className="size-4 shrink-0 text-muted" name="openInNew" />
      ) : null}
    </>
  );

  if (resource.href) {
    return (
      <div role="listitem">
        <Button
          asChild
          className="h-auto w-full justify-start gap-2 rounded-lg px-0 py-2"
          variant="ghost"
        >
          <a
            aria-label={`Open ${resource.label} in a new tab`}
            href={resource.href}
            rel="noopener noreferrer"
            target="_blank"
          >
            {content}
          </a>
        </Button>
      </div>
    );
  }

  return (
    <div
      aria-disabled="true"
      className="flex w-full items-center gap-2 rounded-lg px-0 py-2 opacity-70"
      role="listitem"
    >
      {content}
    </div>
  );
}
