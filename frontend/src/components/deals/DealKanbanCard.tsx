import type { KanbanCardRenderer } from "../kanban-board/types";
import { getDealTransitionName, type DealLifecycle } from "../../data/dealsView";
import { ViewTransition } from "../ui/ViewTransition";

export type DealKanbanCardData = {
  closeDate?: string;
  colorClassName: string;
  id: string;
  lifecycle: DealLifecycle;
  name: string;
  openQuestionCount: number;
  sponsor?: string;
  targetCompany?: string;
  type: string;
};

const closeDateFormatter = new Intl.DateTimeFormat(undefined, {
  day: "numeric",
  month: "short",
  timeZone: "UTC",
  year: "numeric",
});

export const dealCardRenderer: KanbanCardRenderer<DealKanbanCardData> = {
  id: "deal-card",
  label: "Deal",
  render: (data) => <DealKanbanCard deal={data} />,
};

export function DealKanbanCard({ deal }: { deal: DealKanbanCardData }) {
  const transitionName = getDealTransitionName(deal.id);

  return (
    <ViewTransition default="none" name={transitionName} share="morph">
      <article className="rounded-xl border border-outline-variant bg-surface-container-lowest p-4 text-on-surface shadow-[0_8px_20px_rgba(7,1,84,0.05)] transition-colors group-hover:bg-surface-container-low">
        <div className="flex items-start gap-3">
          <span aria-hidden="true" className={`mt-1 h-2.5 w-2.5 shrink-0 rounded-full ${deal.colorClassName}`} />
          <div className="min-w-0 flex-1">
            <ViewTransition default="none" name={`${transitionName}-title`} share="text-morph">
              <h3 className="truncate text-[13px] font-medium text-text-main">{deal.name}</h3>
            </ViewTransition>
            <p className="mt-1 truncate text-[11px] font-normal text-muted">
              {deal.targetCompany ?? "Target company unavailable"}
            </p>
          </div>
          <span className="rounded-full border border-outline-variant px-2 py-0.5 text-[10px] font-normal text-muted">
            {deal.lifecycle}
          </span>
        </div>

        <dl className="mt-4 grid grid-cols-2 gap-x-3 gap-y-3 border-t border-outline-variant/60 pt-3">
          <div className="min-w-0">
            <dt className="text-[10px] font-normal uppercase tracking-[0.08em] text-muted">Type</dt>
            <dd className="mt-1 truncate text-[11px] font-normal text-on-surface">{deal.type}</dd>
          </div>
          <div className="min-w-0">
            <dt className="text-[10px] font-normal uppercase tracking-[0.08em] text-muted">Sponsor</dt>
            <dd className="mt-1 truncate text-[11px] font-normal text-on-surface">{deal.sponsor ?? "—"}</dd>
          </div>
          <div>
            <dt className="text-[10px] font-normal uppercase tracking-[0.08em] text-muted">Target close</dt>
            <dd className="mt-1 text-[11px] font-normal text-on-surface">{formatCloseDate(deal.closeDate)}</dd>
          </div>
          <div>
            <dt className="text-[10px] font-normal uppercase tracking-[0.08em] text-muted">Open questions</dt>
            <dd className="mt-1 text-[11px] font-medium tabular-nums text-text-main">{deal.openQuestionCount}</dd>
          </div>
        </dl>
      </article>
    </ViewTransition>
  );
}

function formatCloseDate(value?: string): string {
  if (!value) return "—";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "—" : closeDateFormatter.format(date);
}
