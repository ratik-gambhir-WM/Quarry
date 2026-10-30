import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type {
  ColumnDef,
  ColumnPinningState,
  PaginationState,
  SortingState,
} from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import type { DealPortfolioView } from "../../data/dealsView";
import {
  getDealRoomPath,
  type WorkspaceLocationState,
} from "../../data/workspace";
import { ArrowUpRight } from "lucide-react";
import {
  DataGrid,
  DataGridContainer,
  dataGridFeatures,
  type DataGridFeatures,
} from "../reui/data-grid/data-grid";
import { DataGridColumnHeader } from "../reui/data-grid/data-grid-column-header";
import { DataGridPagination } from "../reui/data-grid/data-grid-pagination";
import { DataGridScrollArea } from "../reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "../reui/data-grid/data-grid-table";
import {
  quarryDataGridHeaderClassName,
  quarryDataGridTableClassNames,
} from "../reui/data-grid/quarry-data-grid";
import { ViewTransition } from "../ui/ViewTransition";

const dealsDataGridTableClassNames = {
  ...quarryDataGridTableClassNames,
  base: `${quarryDataGridTableClassNames.base} bg-[var(--theme-workspace-surface)] text-center`,
  edgeCell: `${quarryDataGridTableClassNames.edgeCell} data-pinned:bg-[var(--theme-workspace-surface)]`,
  header: `${quarryDataGridTableClassNames.header} bg-[var(--theme-workspace-surface)] [&_th>div]:ms-0 [&_th>div]:justify-center`,
  headerRow: `${quarryDataGridTableClassNames.headerRow} bg-[var(--theme-workspace-surface)]`,
  headerSticky: `${quarryDataGridTableClassNames.headerSticky} bg-[var(--theme-workspace-surface)]`,
} as const;

type DealsTableProps = {
  deals: DealPortfolioView[];
  navigationState?: WorkspaceLocationState;
  onReset: () => void;
};

export function DealsTable(props: DealsTableProps) {
  return (
    <DealsTableState
      key={props.deals.map((deal) => deal.id).join("|")}
      {...props}
    />
  );
}
function DealsTableState({ deals, navigationState, onReset }: DealsTableProps) {
  const [sorting, setSorting] = useState<SortingState>([]);
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [columnPinning, setColumnPinning] = useState<ColumnPinningState>({
    end: [],
    start: ["deal"],
  });

  const columns = useMemo<ColumnDef<DataGridFeatures, DealPortfolioView>[]>(
    () => [
      {
        accessorKey: "name",
        id: "deal",
        header: ({ column }) => (
          <DataGridColumnHeader
            className={quarryDataGridHeaderClassName}
            column={column}
            title="Project name"
          />
        ),
        cell: ({ row }) => {
          const deal = row.original;
          return (
            <ViewTransition
              default="none"
              name={deal.transitionName}
              share="morph"
            >
              <div className="min-w-0">
                <ViewTransition
                  default="none"
                  name={`${deal.transitionName}-title`}
                  share="text-morph"
                >
                  <Link
                    aria-label={`Open ${deal.name}`}
                    className="block truncate font-medium text-foreground outline-none hover:text-primary focus-visible:rounded-sm focus-visible:ring-2 focus-visible:ring-ring"
                    state={navigationState}
                    to={getDealRoomPath(deal.id)}
                  >
                    {deal.name.trim() || "N/A"}
                  </Link>
                </ViewTransition>
              </div>
            </ViewTransition>
          );
        },
        minSize: 180,
        size: 220,
        meta: { fillWidth: true, headerTitle: "Project name" },
      },
      textColumn("status", "Status", 125),
      textColumn("targetCompany", "Target", 170),
      textColumn("portfolioCompany", "PortCo", 170),
      textColumn("engagement", "Engagement type", 150),
      textColumn("sponsor", "PE sponsor", 170),
      linkColumn("sowHref", "SOW", 85),
      linkColumn("factSheetHref", "Fact sheet", 100),
      linkColumn("sharepointHref", "SharePoint", 110),
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
    <div className="-mx-8 -mt-8 w-[calc(100%+4rem)] border-y border-outline-variant/70">
      <DataGrid
        recordCount={deals.length}
        table={table}
        tableClassNames={dealsDataGridTableClassNames}
        tableLayout={{
          cellBorder: true,
          columnsPinnable: true,
          columnsResizable: true,
          dense: true,
          headerBackground: false,
          headerBorder: true,
          rowBorder: true,
        }}
      >
        <DataGridContainer>
          <DataGridScrollArea orientation="horizontal">
            <DataGridTable />
          </DataGridScrollArea>
        </DataGridContainer>
        <div className="border-t border-outline-variant/70 px-5 py-2">
          <DataGridPagination sizes={[5, 10, 25, 50]} />
        </div>
      </DataGrid>
    </div>
  );
}

function textColumn(
  accessorKey: keyof DealPortfolioView,
  title: string,
  size: number,
): ColumnDef<DataGridFeatures, DealPortfolioView> {
  return {
    accessorKey,
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title={title}
      />
    ),
    cell: ({ row }) => (
      <span title={String(row.original[accessorKey] ?? "").trim() || "N/A"}>
        {String(row.original[accessorKey] ?? "").trim() || "N/A"}
      </span>
    ),
    minSize: 110,
    size,
    meta: { headerTitle: title },
  };
}

function linkColumn(
  accessorKey: "sowHref" | "factSheetHref" | "sharepointHref",
  title: string,
  size: number,
): ColumnDef<DataGridFeatures, DealPortfolioView> {
  return {
    accessorKey,
    header: title,
    cell: ({ row }) => {
      const href = row.original[accessorKey];
      return href ? (
        <a
          href={href}
          target="_blank"
          rel="noopener noreferrer"
          aria-label={`Open ${title} for ${row.original.name}`}
          className="inline-flex items-center gap-1 rounded-sm text-primary underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {title}
          <ArrowUpRight aria-hidden="true" className="size-3.5" />
        </a>
      ) : (
        <span className="text-muted-foreground">N/A</span>
      );
    },
    enableSorting: false,
    minSize: size,
    size,
    meta: { headerTitle: title },
  };
}

export function DealsEmptyState({ onReset }: { onReset: () => void }) {
  return (
    <section className="flex min-h-64 w-full flex-col items-center justify-center px-6 py-12 text-center">
      <p className="text-[16px] font-medium text-text-main">
        No deals match these filters
      </p>
      <p className="mt-2 w-full max-w-[28rem] text-[13px] font-normal leading-5 text-muted">
        Try a different search or return to the full current and historic
        portfolio.
      </p>
      <button
        className="mt-5 rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] font-medium text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={onReset}
        type="button"
      >
        Reset filters
      </button>
    </section>
  );
}
