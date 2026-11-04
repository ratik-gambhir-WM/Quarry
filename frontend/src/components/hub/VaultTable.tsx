import type { ColumnDef } from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import type { VaultTableRow } from "../../data/vaultTable";
import {
  DataGrid,
  DataGridContainer,
  dataGridFeatures,
  type DataGridFeatures,
} from "../reui/data-grid/data-grid";
import { DataGridColumnHeader } from "../reui/data-grid/data-grid-column-header";
import { DataGridScrollArea } from "../reui/data-grid/data-grid-scroll-area";
import { DataGridTable } from "../reui/data-grid/data-grid-table";
import {
  quarryDataGridHeaderClassName,
  quarryDataGridTableClassNames,
} from "../reui/data-grid/quarry-data-grid";
import { Icon } from "../ui/Icon";

const vaultTableClassNames = {
  ...quarryDataGridTableClassNames,
  base: `${quarryDataGridTableClassNames.base} bg-transparent [&_tbody_td]:px-4 [&_tbody_td]:py-3`,
  header: "bg-transparent",
  headerRow: "border-outline-variant/70 bg-transparent",
} as const;

const vaultTableColumns: ColumnDef<DataGridFeatures, VaultTableRow>[] = [
  {
    accessorKey: "name",
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title="Name"
      />
    ),
    cell: ({ row }) => (
      <div className="flex min-w-0 items-start gap-3" title={row.original.name}>
        <Icon className="mt-0.5 h-4 w-4 shrink-0 text-muted" name={row.original.typeIcon} />
        <div className="min-w-0">
          <p className="truncate text-[14px] font-semibold text-text-main">
            {row.original.name}
          </p>
          <p className="mt-0.5 truncate text-[13px] text-muted">
            {row.original.description}
          </p>
        </div>
      </div>
    ),
    enableSorting: false,
    minSize: 320,
    size: 420,
    meta: { headerTitle: "Name" },
  },
  {
    accessorKey: "type",
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title="Type"
      />
    ),
    cell: ({ row }) => (
      <span className="text-[13px] text-muted">{row.original.type}</span>
    ),
    enableSorting: false,
    minSize: 100,
    size: 100,
    meta: { headerTitle: "Type" },
  },
  {
    accessorKey: "project",
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title="Project"
      />
    ),
    cell: ({ row }) => (
      <span
        className="block truncate text-[14px] text-text-main"
        title={row.original.project}
      >
        {row.original.project}
      </span>
    ),
    enableSorting: false,
    minSize: 150,
    size: 180,
    meta: { headerTitle: "Project" },
  },
  {
    accessorKey: "practiceArea",
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title="Practice area"
      />
    ),
    cell: ({ row }) => (
      <span
        className="block truncate text-[14px] text-text-main"
        title={row.original.practiceArea ?? undefined}
      >
        {row.original.practiceArea ?? "—"}
      </span>
    ),
    enableSorting: false,
    minSize: 150,
    size: 180,
    meta: { headerTitle: "Practice area" },
  },
  {
    accessorKey: "creator",
    header: ({ column }) => (
      <DataGridColumnHeader
        className={quarryDataGridHeaderClassName}
        column={column}
        title="Created by"
      />
    ),
    cell: ({ row }) => (
      <span
        className="block truncate text-[14px] text-text-main"
        title={row.original.creator ?? undefined}
      >
        {row.original.creator ?? "—"}
      </span>
    ),
    enableSorting: false,
    minSize: 140,
    size: 180,
    meta: { headerTitle: "Created by" },
  },
];

export function VaultTable({ rows }: { rows: VaultTableRow[] }) {
  const table = useTable({
    columns: vaultTableColumns,
    data: rows,
    features: dataGridFeatures,
    getRowId: (row) => row.id,
  });

  return (
    <DataGrid
      emptyMessage="No items match your filters. Clear filters to see all sample items."
      recordCount={rows.length}
      table={table}
      tableClassNames={vaultTableClassNames}
      tableLayout={{
        cellBorder: false,
        headerBackground: false,
        headerBorder: true,
        rowBorder: true,
        width: "fixed",
      }}
    >
      <DataGridContainer>
        <DataGridScrollArea orientation="horizontal">
          <DataGridTable />
        </DataGridScrollArea>
      </DataGridContainer>
    </DataGrid>
  );
}
