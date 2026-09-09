import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type { ColumnDef, ColumnPinningState, PaginationState, SortingState } from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import type { DealPortfolioView } from "../../data/dealsView";
import { getDealRoomPath, type WorkspaceLocationState } from "../../data/workspace";
import { formatShortUtcDate } from "../../lib/formatters";
import { DataGrid, DataGridContainer, dataGridFeatures, type DataGridFeatures } from "../reui/data-grid/data-grid";
import { DataGridColumnHeader } from "../reui/data-grid/data-grid-column-header";
import { DataGridPagination } from "../reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "../reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "../reui/data-grid/data-grid-table";
import { quarryDataGridHeaderClassName, quarryDataGridTableClassNames } from "../reui/data-grid/quarry-data-grid";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { ViewTransition } from "../ui/ViewTransition";

type DealsTableProps = {
  deals: DealPortfolioView[];
  navigationState?: WorkspaceLocationState;
  onReset: () => void;
};

export function DealsTable({ deals, navigationState, onReset }: DealsTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [pagination, setPagination] = useState<PaginationState>({ pageIndex: 0, pageSize: 10 });
  const [columnPinning, setColumnPinning] = useState<ColumnPinningState>({ end: [], start: ["deal"] });

  useEffect(() => {
    setPagination((current) => ({ ...current, pageIndex: 0 }));
  }, [deals]);

  const columns = useMemo<ColumnDef<DataGridFeatures, DealPortfolioView>[]>(
    () => [
      {
        accessorKey: "name",
        id: "deal",
        header: ({ column }) => <DataGridColumnHeader className={quarryDataGridHeaderClassName} column={column} title="Deal" />,
        cell: ({ row }) => {
          const deal = row.original;
          return (
            <ViewTransition default="none" name={deal.transitionName} share="morph">
              <div className="min-w-0">
                <ViewTransition default="none" name={`${deal.transitionName}-title`} share="text-morph">
                  <Link
                    aria-label={`Open ${deal.name}`}
                    className="block truncate font-medium text-foreground outline-none hover:text-primary focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring"
                    state={navigationState}
                    to={getDealRoomPath(deal.id)}
                  >
                    {deal.name}
                  </Link>
                </ViewTransition>
                <span className="block truncate text-xs text-muted-foreground">
                  {deal.targetCompany ?? "Target company unavailable"}
                </span>
              </div>
            </ViewTransition>
          );
        },
        minSize: 200,
        size: 270,
        meta: { headerTitle: "Deal" },
      },
      textColumn("lifecycle", "Lifecycle", 130),
      textColumn("status", "Status", 150),
      textColumn("type", "Type", 190),
      textColumn("sponsor", "Sponsor", 170, (value) => String(value ?? "—")),
      textColumn("closeDate", "Target close", 150, (value) => formatShortUtcDate(typeof value === "string" ? value : undefined)),
      textColumn("openQuestionCount", "Open questions", 150),
    ],
    [navigationState],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: deals,
    getRowId: (row) => row.id,
    state: { columnPinning, pagination, sorting },
    onColumnPinningChange: setColumnPinning,
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    autoResetPageIndex: false,
  });

  if (deals.length === 0) return <DealsEmptyState onReset={onReset} />;

  return (
    <div className="overflow-hidden rounded-2xl border border-outline-variant/70 bg-surface-container-lowest">
      <DataGrid
        recordCount={deals.length}
        table={table}
        tableClassNames={quarryDataGridTableClassNames}
        tableLayout={{ cellBorder: true, columnsPinnable: true, columnsResizable: true, dense: true, headerBackground: false, headerBorder: true, rowBorder: true }}
      >
        <DataGridContainer>
          <DataGridScrollArea orientation="horizontal"><DataGridTable /></DataGridScrollArea>
        </DataGridContainer>
        <div className="border-t border-outline-variant/70 px-5 py-2"><DataGridPagination sizes={[5, 10, 25, 50]} /></div>
      </DataGrid>
    </div>
  );
}

function textColumn(
  accessorKey: keyof DealPortfolioView,
  title: string,
  size: number,
  format: (value: DealPortfolioView[keyof DealPortfolioView]) => string | number = (value) => String(value ?? "—"),
): ColumnDef<DataGridFeatures, DealPortfolioView> {
  return {
    accessorKey,
    header: ({ column }) => <DataGridColumnHeader className={quarryDataGridHeaderClassName} column={column} title={title} />,
    cell: ({ row }) => <span>{format(row.original[accessorKey])}</span>,
    minSize: 110,
    size,
    meta: { headerTitle: title },
  };
}

export function DealsEmptyState({ onReset }: { onReset: () => void }) {
  return (
    <WorkspaceCard className="flex min-h-64 flex-col items-center justify-center px-6 py-12 text-center" radius="small">
      <p className="text-[16px] font-medium text-text-main">No deals match these filters</p>
      <p className="mt-2 max-w-md text-[13px] font-normal leading-5 text-muted">Try a different search or return to the full current and historic portfolio.</p>
      <button className="mt-5 rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] font-medium text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed" onClick={onReset} type="button">Reset filters</button>
    </WorkspaceCard>
  );
}
