import type { KanbanCardRenderer } from "../kanban-board/types";
import {
  getDealTransitionName,
  type DealLifecycle,
} from "../../data/dealsView";
import { formatShortUtcDate } from "../../lib/formatters";
import { ViewTransition } from "../ui/ViewTransition";

export type DealKanbanCardData = {
  closeDate?: string;
  closeDateLabel: string;
  engagement: string;
  colorClassName: string;
  id: string;
  lifecycle: DealLifecycle;
  name: string;
  openQuestionCount: number;
  sponsor?: string;
  targetCompany?: string;
  type: string;
};

export const dealCardRenderer: KanbanCardRenderer<DealKanbanCardData> = {
  id: "deal-card",
  label: "Deal",
  render: (data) => <DealKanbanCard deal={data} />,
};

export function DealKanbanCard({ deal }: { deal: DealKanbanCardData }) {
  const transitionName = getDealTransitionName(deal.id);

  return (
    <ViewTransition default="none" name={transitionName} share="morph">
      <article className="rounded-[var(--radius-default)] border border-outline-variant/80 bg-surface-container-lowest p-3.5 text-on-surface shadow-sm transition-colors group-hover:border-outline group-focus-visible:border-outline">
        <div className="mb-2.5 flex items-center justify-between gap-3 text-[11px] text-muted">
          <span className="truncate font-medium tracking-wide" title={deal.id}>
            {deal.id}
          </span>
          <span className="shrink-0">{deal.lifecycle}</span>
        </div>
        <div className="flex items-start gap-3">
          <span
            aria-hidden="true"
            className={`mt-1 h-3 w-3 shrink-0 rounded-full border border-outline ${deal.colorClassName}`}
          />
          <div className="min-w-0 flex-1">
            <ViewTransition
              default="none"
              name={`${transitionName}-title`}
              share="text-morph"
            >
              <h3
                className="line-clamp-2 text-[14px] font-medium leading-5 text-text-main"
                title={deal.name}
              >
                {deal.name}
              </h3>
            </ViewTransition>
            <p className="mt-1 truncate text-[12px] font-normal text-muted">
              {deal.targetCompany ?? "Target company unavailable"}
            </p>
          </div>
        </div>

        <div className="mt-4 flex flex-wrap gap-1.5 text-[11px] text-muted">
          <span
            className="max-w-full truncate rounded-md border border-outline-variant/70 px-2 py-1"
            title={`Type: ${deal.type}`}
          >
            {deal.type}
          </span>
          <span
            className="rounded-md border border-outline-variant/70 px-2 py-1"
            title={`Engagement: ${deal.engagement}`}
          >
            {deal.engagement}
          </span>
        </div>
        {deal.sponsor ? (
          <p
            className="mt-2 truncate text-[11px] text-muted"
            title={`Sponsor: ${deal.sponsor}`}
          >
            Sponsor · {deal.sponsor}
          </p>
        ) : null}
        <div className="mt-4 flex flex-wrap items-center justify-between gap-x-3 gap-y-1 text-[11px] text-muted">
          <span>
            {deal.closeDateLabel} · {formatShortUtcDate(deal.closeDate)}
          </span>
          <span className="tabular-nums">
            {deal.openQuestionCount} open{" "}
            {deal.openQuestionCount === 1 ? "question" : "questions"}
          </span>
        </div>
      </article>
    </ViewTransition>
  );
}
