import { useEffect, useMemo, useRef, useState } from "react";
import type { ColumnDef, ColumnPinningState, PaginationState, SortingState } from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import { Search, X } from "lucide-react";
import type { DataRoomFileEntry, DataRoomTreeNode } from "../../data/dataRoom";
import {
  buildFileReviewRows,
  type FileReviewRow,
  type FileReviewStatus,
  type ReviewSignalLevel,
} from "../../data/fileReview";
import { illustrativeFileReviewSummaries } from "../../fixtures/data-room/fileReview";
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
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

type FileReviewTableProps = {
  files: readonly DataRoomFileEntry[];
  onSelectFile: (file: DataRoomTreeNode) => void;
};

type StatusFilter = "All" | FileReviewStatus;
type TypeFilter = "All types" | FileReviewRow["fileType"];
const compactHeaderClassName = `${quarryDataGridHeaderClassName} text-[11px]`;
const fileReviewColumnSize = 160;

export function FileReviewTable({ files, onSelectFile }: FileReviewTableProps) {
  const rows = useMemo(
    () => buildFileReviewRows(files, illustrativeFileReviewSummaries),
    [files],
  );
  const [query, setQuery] = useState("");
  const [searchExpanded, setSearchExpanded] = useState(false);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("All");
  const [typeFilter, setTypeFilter] = useState<TypeFilter>("All types");
  const [sorting, setSorting] = useState<SortingState>([]);
  const [pagination, setPagination] = useState<PaginationState>({ pageIndex: 0, pageSize: 10 });
  const [columnPinning, setColumnPinning] = useState<ColumnPinningState>({ end: [], start: ["file"] });
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchTriggerRef = useRef<HTMLButtonElement>(null);

  const visibleRows = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return rows.filter((row) => {
      const matchesStatus = statusFilter === "All" || row.status === statusFilter;
      const matchesType = typeFilter === "All types" || row.fileType === typeFilter;
      const searchText = [
        row.fileName,
        row.folderLabel,
        row.keyFinding,
        row.risks,
        row.opportunities,
        row.impact,
      ]
        .join(" ")
        .toLocaleLowerCase();
      return matchesStatus && matchesType && (!normalizedQuery || searchText.includes(normalizedQuery));
    });
  }, [query, rows, statusFilter, typeFilter]);

  useEffect(() => {
    setPagination((current) => ({ ...current, pageIndex: 0 }));
  }, [query, statusFilter, typeFilter]);

  useEffect(() => {
    if (searchExpanded) {
      searchInputRef.current?.focus();
    }
  }, [searchExpanded]);

  const columns = useMemo<ColumnDef<DataGridFeatures, FileReviewRow>[]>(
    () => [
      {
        accessorKey: "fileName",
        id: "file",
        header: ({ column }) => <DataGridColumnHeader className={compactHeaderClassName} column={column} title="File" />,
        cell: ({ row }) => (
          <div className="min-w-0">
            <button
              className="block max-w-full truncate text-left text-[11px] font-medium text-foreground hover:text-primary focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              onClick={() => onSelectFile(row.original.node)}
              title={`${row.original.fileName} — ${row.original.folderLabel} · ${row.original.fileType}`}
              type="button"
            >
              {row.original.fileName}
            </button>
            <span className="block truncate text-[10px] text-muted-foreground">
              {row.original.folderLabel} · {row.original.fileType}
            </span>
          </div>
        ),
        size: fileReviewColumnSize,
        minSize: fileReviewColumnSize,
        meta: { headerTitle: "File" },
      },
      reviewColumn("keyFinding", "Key findings"),
      reviewColumn("risks", "Risks", "riskLevel"),
      reviewColumn("opportunities", "Opportunities", "opportunityLevel"),
      reviewColumn("impact", "Key question impact", "impactLevel"),
    ],
    [onSelectFile],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: visibleRows,
    getRowId: (row) => row.id,
    state: { columnPinning, pagination, sorting },
    onColumnPinningChange: setColumnPinning,
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    autoResetPageIndex: false,
  });

  const resetFilters = () => {
    setQuery("");
    setStatusFilter("All");
    setTypeFilter("All types");
  };

  const collapseSearch = () => {
    setQuery("");
    setSearchExpanded(false);
    requestAnimationFrame(() => searchTriggerRef.current?.focus());
  };

  return (
    <section className="glass-panel workspace-pane relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center gap-4 border-b border-outline-variant bg-background px-5">
        <h1 className="min-w-0 flex-1 truncate text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">File review</h1>
      </header>
      <div className="min-h-0 flex-1 overflow-hidden p-3 pb-28">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <div
            className={`relative h-8 shrink-0 overflow-hidden rounded-full border border-input bg-background transition-[width,border-color,box-shadow] duration-300 ease-out focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50 motion-reduce:transition-none ${
              searchExpanded ? "w-64 max-sm:w-44" : "w-8"
            }`}
          >
            {searchExpanded ? (
              <>
                <Search aria-hidden="true" className="pointer-events-none absolute left-2 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                <Input
                  aria-label="Search file reviews"
                  className="h-full rounded-full border-0 bg-transparent pr-8 pl-8 text-xs shadow-none focus-visible:border-0 focus-visible:ring-0 md:text-xs dark:bg-transparent"
                  id="file-review-search"
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Escape") {
                      collapseSearch();
                    }
                  }}
                  placeholder="Search files and findings"
                  ref={searchInputRef}
                  type="search"
                  value={query}
                />
                <button
                  aria-label={query ? "Clear file review search" : "Close file review search"}
                  className="absolute right-1.5 top-1/2 grid size-5 -translate-y-1/2 place-items-center rounded-full text-muted-foreground hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onClick={() => {
                    if (query) {
                      setQuery("");
                      searchInputRef.current?.focus();
                    } else {
                      collapseSearch();
                    }
                  }}
                  type="button"
                >
                  <X aria-hidden="true" className="size-3.5" />
                </button>
              </>
            ) : (
              <button
                aria-controls="file-review-search"
                aria-expanded="false"
                aria-label="Open file review search"
                className="grid size-full place-items-center rounded-full text-muted-foreground transition-colors hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
                onClick={() => setSearchExpanded(true)}
                ref={searchTriggerRef}
                type="button"
              >
                <Search aria-hidden="true" className="size-4" />
              </button>
            )}
          </div>
          {(["All", "Needs attention", "Reviewed", "Pending"] as const).map((status) => (
            <Button
              aria-pressed={statusFilter === status}
              className="text-xs"
              key={status}
              onClick={() => setStatusFilter(status)}
              size="sm"
              type="button"
              variant={statusFilter === status ? "default" : "outline"}
            >
              {status}
            </Button>
          ))}
          <label className="ml-auto text-xs text-muted-foreground" htmlFor="file-review-type">File type</label>
          <select
            className="h-8 rounded-lg border border-input bg-background px-2 text-xs text-foreground"
            id="file-review-type"
            onChange={(event) => setTypeFilter(event.target.value as TypeFilter)}
            value={typeFilter}
          >
            {(["All types", "PDF", "Document", "Spreadsheet"] as const).map((type) => (
              <option key={type}>{type}</option>
            ))}
          </select>
          {(query || statusFilter !== "All" || typeFilter !== "All types") ? (
            <Button onClick={resetFilters} size="sm" type="button" variant="ghost">Reset</Button>
          ) : null}
        </div>
        <DataGrid
          emptyMessage="No file reviews match these filters."
          recordCount={visibleRows.length}
          table={table}
          tableClassNames={quarryDataGridTableClassNames}
          tableLayout={{
            cellBorder: true,
            columnsPinnable: true,
            dense: true,
            headerBackground: false,
            headerBorder: true,
            headerSticky: true,
            rowBorder: true,
            width: "fixed",
          }}
        >
          <div className="overflow-hidden rounded-2xl border border-outline-variant/70 bg-surface-container-lowest">
            <DataGridContainer>
              <DataGridScrollArea className="max-h-[calc(100vh-17rem)]" orientation="both">
                <DataGridTable />
              </DataGridScrollArea>
            </DataGridContainer>
            <div className="border-t border-outline-variant/70 px-5 py-2">
              <DataGridPagination sizes={[5, 10, 25, 50]} />
            </div>
          </div>
        </DataGrid>
      </div>
    </section>
  );
}

function reviewColumn(
  accessorKey: "impact" | "keyFinding" | "opportunities" | "risks",
  title: string,
  levelKey?: "impactLevel" | "opportunityLevel" | "riskLevel",
): ColumnDef<DataGridFeatures, FileReviewRow> {
  return {
    accessorKey,
    header: ({ column }) => <DataGridColumnHeader className={compactHeaderClassName} column={column} title={title} />,
    cell: ({ row }) => (
      <div className="flex min-w-0 items-center gap-1.5" title={row.original[accessorKey]}>
        {levelKey ? <SignalBadge level={row.original[levelKey]} /> : null}
        <span className="truncate text-[11px] leading-5">{row.original[accessorKey]}</span>
      </div>
    ),
    size: fileReviewColumnSize,
    minSize: fileReviewColumnSize,
    meta: { headerTitle: title },
  };
}

function SignalBadge({ level }: { level: ReviewSignalLevel }) {
  return <Badge variant={level === "high" ? "destructive" : "outline"}>{level}</Badge>;
}
