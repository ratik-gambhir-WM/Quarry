import type { ColumnDef } from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import { vaultTableRows, type VaultTableRow } from "../../data/vaultTable";
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
  base: `${quarryDataGridTableClassNames.base} bg-transparent [&_tbody_td]:px-5 [&_tbody_td]:py-5`,
  header: "bg-transparent",
  headerRow: "border-outline-variant/70 bg-transparent",
} as const;

const vaultTableColumns: ColumnDef<DataGridFeatures, VaultTableRow>[] = [
  {
    id: "favorite",
    header: () => <span className="sr-only">Favorite</span>,
    cell: () => <Icon className="h-5 w-5 text-muted" name="star" />,
    enableSorting: false,
    maxSize: 48,
    minSize: 48,
    size: 48,
    meta: { headerTitle: "Favorite" },
  },
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
      <div className="min-w-0" title={row.original.name}>
        <p className="truncate text-[15px] font-semibold text-text-main">
          {row.original.name}
        </p>
        <p className="mt-1 truncate text-[13px] text-muted">
          {row.original.description}
        </p>
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
      <div className="flex min-w-0 items-center gap-2.5">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-surface-container-low text-primary">
          <Icon className="h-4 w-4" name={row.original.typeIcon} />
        </span>
        <span className="truncate text-[14px] text-text-main">
          {row.original.type}
        </span>
      </div>
    ),
    enableSorting: false,
    minSize: 150,
    size: 180,
    meta: { headerTitle: "Type" },
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
        title={row.original.practiceArea}
      >
        {row.original.practiceArea}
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
        title="Creator"
      />
    ),
    cell: ({ row }) => (
      <span
        className="block truncate text-[14px] text-text-main"
        title={row.original.creator}
      >
        {row.original.creator}
      </span>
    ),
    enableSorting: false,
    minSize: 140,
    size: 180,
    meta: { headerTitle: "Creator" },
  },
];

export function VaultTable() {
  const table = useTable({
    columns: vaultTableColumns,
    data: vaultTableRows,
    features: dataGridFeatures,
    getRowId: (row) => row.id,
  });

  return (
    <section aria-labelledby="vault-table-heading">
      <h2
        className="px-8 text-[11px] font-bold uppercase tracking-[0.2em] text-muted"
        id="vault-table-heading"
      >
        Vault table
      </h2>

      <DataGrid
        emptyMessage="No vault items are available yet."
        recordCount={vaultTableRows.length}
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
        <DataGridContainer className="mt-4 border-y border-outline-variant/70">
          <DataGridScrollArea orientation="horizontal">
            <DataGridTable />
          </DataGridScrollArea>
        </DataGridContainer>
      </DataGrid>
    </section>
  );
}
