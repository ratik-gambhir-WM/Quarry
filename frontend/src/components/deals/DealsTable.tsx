import { Link } from "react-router-dom";
import type { DealPortfolioView } from "../../data/dealsView";
import { getDealRoomPath, type WorkspaceLocationState } from "../../data/workspace";
import { formatShortUtcDate } from "../../lib/formatters";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { DataTableHeaderRow, DataTableHeading } from "../ui/DataTable";
import { ViewTransition } from "../ui/ViewTransition";

type DealsTableProps = {
  deals: DealPortfolioView[];
  navigationState?: WorkspaceLocationState;
  onReset: () => void;
};

export function DealsTable({ deals, navigationState, onReset }: DealsTableProps) {
  if (deals.length === 0) return <DealsEmptyState onReset={onReset} />;

  return (
    <div className="overflow-x-auto border-y border-outline-variant/70">
      <table className="w-full min-w-[980px] border-collapse text-left">
          <thead>
            <DataTableHeaderRow>
              <DataTableHeading className="px-5">Deal</DataTableHeading>
              <DataTableHeading>Lifecycle</DataTableHeading>
              <DataTableHeading>Status</DataTableHeading>
              <DataTableHeading>Type</DataTableHeading>
              <DataTableHeading>Sponsor</DataTableHeading>
              <DataTableHeading>Target close</DataTableHeading>
              <DataTableHeading className="px-5 text-right">Open questions</DataTableHeading>
            </DataTableHeaderRow>
          </thead>
          <tbody className="divide-y divide-outline-variant/60">
            {deals.map((deal) => (
              <ViewTransition
                default="none"
                key={deal.id}
                name={deal.transitionName}
                share="morph"
              >
                <tr className="deals-table-row transition-colors hover:bg-[var(--theme-workspace-chrome)] focus-within:bg-[var(--theme-workspace-chrome)]">
                  <td className="px-5 py-4">
                    <div className="flex min-w-0 items-center gap-3">
                      <span aria-hidden="true" className={`h-2.5 w-2.5 shrink-0 rounded-full ${deal.colorClassName}`} />
                      <div className="min-w-0">
                        <ViewTransition
                          default="none"
                          name={`${deal.transitionName}-title`}
                          share="text-morph"
                        >
                          <Link
                            aria-label={`Open ${deal.name}`}
                            className="block truncate text-[13px] font-medium text-text-main outline-none hover:text-primary focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-primary-fixed"
                            state={navigationState}
                            to={getDealRoomPath(deal.id)}
                          >
                            {deal.name}
                          </Link>
                        </ViewTransition>
                        <p className="mt-1 truncate text-[11px] font-normal text-muted">
                          {deal.targetCompany ?? "Target company unavailable"}
                        </p>
                      </div>
                    </div>
                  </td>
                  <td className="px-4 py-4 text-[12px] font-normal text-on-surface">
                    <span className="rounded-full border border-outline-variant px-2.5 py-1">{deal.lifecycle}</span>
                  </td>
                  <td className="px-4 py-4 text-[12px] font-normal text-on-surface">{deal.status}</td>
                  <td className="max-w-[180px] px-4 py-4 text-[12px] font-normal text-on-surface">
                    <span className="block truncate">{deal.type}</span>
                  </td>
                  <td className="max-w-[170px] px-4 py-4 text-[12px] font-normal text-on-surface">
                    <span className="block truncate">{deal.sponsor ?? "—"}</span>
                  </td>
                  <td className="whitespace-nowrap px-4 py-4 text-[12px] font-normal text-on-surface">
                    {formatShortUtcDate(deal.closeDate)}
                  </td>
                  <td className="px-5 py-4 text-right text-[12px] font-medium tabular-nums text-text-main">
                    {deal.openQuestionCount}
                  </td>
                </tr>
              </ViewTransition>
            ))}
          </tbody>
      </table>
    </div>
  );
}

export function DealsEmptyState({ onReset }: { onReset: () => void }) {
  return (
    <WorkspaceCard className="flex min-h-64 flex-col items-center justify-center px-6 py-12 text-center" radius="small">
      <p className="text-[16px] font-medium text-text-main">No deals match these filters</p>
      <p className="mt-2 max-w-md text-[13px] font-normal leading-5 text-muted">
        Try a different search or return to the full current and historic portfolio.
      </p>
      <button
        className="mt-5 rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] font-medium text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={onReset}
        type="button"
      >
        Reset filters
      </button>
    </WorkspaceCard>
  );
}
