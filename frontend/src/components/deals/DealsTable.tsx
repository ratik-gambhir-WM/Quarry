import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import type {
  ColumnDef,
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
  base: `${quarryDataGridTableClassNames.base} bg-[var(--theme-workspace-surface)] text-left [&_th]:px-4 [&_tbody_td]:px-4 [&_tbody_td]:py-3`,
  edgeCell: `${quarryDataGridTableClassNames.edgeCell} data-pinned:bg-[var(--theme-workspace-surface)]`,
  header: `${quarryDataGridTableClassNames.header} bg-[var(--theme-workspace-surface)] [&_th>div]:ms-0 [&_th>div]:justify-start`,
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

  const columns = useMemo<ColumnDef<DataGridFeatures, DealPortfolioView>[]>(
    () => [
      {
        accessorKey: "name",
        id: "deal",
        header: ({ column }) => (
          <DataGridColumnHeader
            className={`${quarryDataGridHeaderClassName} px-0`}
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
              <div className="min-w-0 max-w-[24rem]">
                <ViewTransition
                  default="none"
                  name={`${deal.transitionName}-title`}
                  share="text-morph"
                >
                  <Link
                    aria-label={`Open ${deal.name}`}
                    title={deal.name.trim() || "N/A"}
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
        meta: { headerTitle: "Project name" },
      },
      textColumn("status", "Status"),
      textColumn("targetCompany", "Target"),
      textColumn("portfolioCompany", "PortCo"),
      textColumn("engagement", "Engagement type"),
      textColumn("sponsor", "PE sponsor"),
      linkColumn("sowHref", "SOW"),
      linkColumn("factSheetHref", "Fact sheet"),
      linkColumn("sharepointHref", "SharePoint"),
    ],
    [navigationState],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: deals,
    getRowId: (row) => row.id,
    state: { pagination, sorting },
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    autoResetPageIndex: false,
  });

  if (deals.length === 0) return <DealsEmptyState onReset={onReset} />;

  return (
    <div className="min-w-0 w-full">
      <DataGrid
        recordCount={deals.length}
        table={table}
        tableClassNames={dealsDataGridTableClassNames}
        tableLayout={{
          cellBorder: false,
          width: "auto",
          dense: false,
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
        <div className="border-t border-outline-variant/70 px-4 py-3">
          <DataGridPagination sizes={[5, 10, 25, 50]} />
        </div>
      </DataGrid>
    </div>
  );
}

function textColumn(
  accessorKey: keyof DealPortfolioView,
  title: string,
): ColumnDef<DataGridFeatures, DealPortfolioView> {
  return {
    accessorKey,
    header: ({ column }) => (
      <DataGridColumnHeader
        className={`${quarryDataGridHeaderClassName} px-0`}
        column={column}
        title={title}
      />
    ),
    cell: ({ row }) => (
      <span className="block max-w-[20rem] truncate" title={String(row.original[accessorKey] ?? "").trim() || "N/A"}>
        {String(row.original[accessorKey] ?? "").trim() || "N/A"}
      </span>
    ),
    meta: { headerTitle: title },
  };
}

function linkColumn(
  accessorKey: "sowHref" | "factSheetHref" | "sharepointHref",
  title: string,
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
          className="inline-flex items-center gap-1 whitespace-nowrap rounded-sm text-primary underline-offset-4 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          {title}
          <ArrowUpRight aria-hidden="true" className="size-3.5" />
        </a>
      ) : (
        <span className="text-muted-foreground">N/A</span>
      );
    },
    enableSorting: false,
    meta: { headerTitle: title },
  };
}

export function DealsEmptyState({ onReset, filtered = true }: { onReset: () => void; filtered?: boolean }) {
  return (
    <section className="flex min-h-64 w-full flex-col items-center justify-center px-6 py-12 text-center">
      <p className="text-[16px] font-medium text-text-main">
        {filtered ? "No deals match these filters" : "No deals yet"}
      </p>
      <p className="mt-2 w-full max-w-[28rem] text-[13px] font-normal leading-5 text-muted">
        {filtered
          ? "Try a different search or return to the full portfolio."
          : "Create a deal using New in the header to start organizing your diligence work."}
      </p>
      {filtered ? <button
        className="mt-5 rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] font-medium text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={onReset}
        type="button"
      >
        Reset filters
      </button> : null}
    </section>
  );
}
