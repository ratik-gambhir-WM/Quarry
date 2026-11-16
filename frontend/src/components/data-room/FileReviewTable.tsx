import { useCallback, useId, useMemo, useRef, useState } from "react";
import type {
  ColumnDef,
  ColumnPinningState,
  ExpandedState,
  PaginationState,
  SortingState,
} from "@tanstack/react-table";
import { useTable } from "@tanstack/react-table";
import { ChevronRight, Search, X } from "lucide-react";
import { FILE_TYPES } from "@/data/fileTypes";
import { FileTypeIcon } from "@/components/ui/FileTypeIcon";
import type { DataRoomFileEntry, DataRoomTreeNode } from "../../data/dataRoom";
import {
  buildFileReviewRows,
  type FileReviewRow,
  type FileReviewStatus,
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
import { cn } from "../../lib/utils";
import { formatFileSize, formatShortUtcDate } from "../../lib/formatters";
import { Button } from "../ui/button";
import { Input } from "../ui/input";

type FileReviewTableProps = {
  files: readonly DataRoomFileEntry[];
  onSelectFile: (file: DataRoomTreeNode) => void;
};

type StatusFilter = "All" | FileReviewStatus;
type TypeFilter = "All types" | FileReviewRow["fileType"];
const compactHeaderClassName = `${quarryDataGridHeaderClassName} text-xs`;
const reviewTableClassNames = {
  ...quarryDataGridTableClassNames,
  base: cn(
    quarryDataGridTableClassNames.base,
    "bg-[var(--theme-workspace-surface)] [&_tbody_td]:align-top [&_tbody_td]:py-2.5",
  ),
  edgeCell: cn(
    quarryDataGridTableClassNames.edgeCell,
    "data-pinned:bg-[var(--theme-workspace-surface)]",
  ),
  header: "bg-[var(--theme-workspace-surface)]",
  headerRow: "border-outline-variant/70 bg-[var(--theme-workspace-surface)]",
  headerSticky: "sticky top-0 z-40 bg-[var(--theme-workspace-surface)]",
};

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
  const [expanded, setExpanded] = useState<ExpandedState>({});
  const reviewId = useId();
  const [pagination, setPagination] = useState<PaginationState>({
    pageIndex: 0,
    pageSize: 10,
  });
  const [columnPinning, setColumnPinning] = useState<ColumnPinningState>({
    end: [],
    start: ["file"],
  });
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchTriggerRef = useRef<HTMLButtonElement>(null);

  const visibleRows = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return rows.filter((row) => {
      const matchesStatus =
        statusFilter === "All" || row.status === statusFilter;
      const matchesType =
        typeFilter === "All types" || row.fileType === typeFilter;
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
      return (
        matchesStatus &&
        matchesType &&
        (!normalizedQuery || searchText.includes(normalizedQuery))
      );
    });
  }, [query, rows, statusFilter, typeFilter]);

  const resetPage = () =>
    setPagination((current) => ({ ...current, pageIndex: 0 }));
  const updateQuery = (value: string) => {
    setQuery(value);
    resetPage();
  };
  const attachSearchInput = useCallback((node: HTMLInputElement | null) => {
    searchInputRef.current = node;
    node?.focus();
  }, []);

  const columns = useMemo<ColumnDef<DataGridFeatures, FileReviewRow>[]>(
    () => [
      {
        accessorKey: "fileName",
        id: "file",
        header: ({ column }) => (
          <DataGridColumnHeader
            className={compactHeaderClassName}
            column={column}
            title="File"
          />
        ),
        cell: ({ row }) => (
          <div className="flex min-w-0 items-start gap-3">
            <button
              aria-controls={row.getIsExpanded() ? `${reviewId}-${encodeURIComponent(row.id)}` : undefined}
              aria-expanded={row.getIsExpanded()}
              aria-label={`${row.getIsExpanded() ? "Hide" : "Show"} review for ${row.original.fileName}`}
              className="file-review-toggle mt-0.5 grid size-8 shrink-0 place-items-center rounded-md text-muted-foreground transition-colors [@media(hover:hover)]:hover:bg-surface-container-high [@media(hover:hover)]:hover:text-foreground active:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              id={`${reviewId}-toggle-${encodeURIComponent(row.id)}`}
              onClick={(event) => {
                event.stopPropagation();
                row.toggleExpanded();
              }}
              onKeyDown={(event) => {
                if (event.key === "ArrowRight" || event.key === "ArrowLeft") {
                  event.preventDefault();
                  row.toggleExpanded(event.key === "ArrowRight");
                }
              }}
              type="button"
            >
              <ChevronRight aria-hidden="true" className={cn("file-review-chevron size-4", row.getIsExpanded() && "rotate-90")} />
            </button>
            <FileTypeIcon filename={row.original.fileName} kind={row.original.node.kind} />
            <div className="min-w-0">
              <button
                className="block max-w-full whitespace-normal break-words text-left text-xs font-medium leading-5 text-foreground [@media(hover:hover)]:hover:text-primary active:text-primary focus-visible:rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                onClick={(event) => {
                  event.stopPropagation();
                  onSelectFile(row.original.node);
                }}
                title={`${row.original.fileName} — ${row.original.folderLabel} · ${row.original.fileType}`}
                type="button"
              >
                {row.original.fileName}
              </button>
              <span
                className="mt-1 block truncate text-[11px] leading-4 text-muted-foreground"
                title={`${row.original.folderLabel}${row.original.source === "stored" ? ` · Uploaded by ${row.original.node.uploadedByName ?? "Unknown"}` : ""}`}
              >
                {row.original.folderLabel}
                {row.original.source === "stored"
                  ? ` · Uploaded by ${row.original.node.uploadedByName ?? "Unknown"}`
                  : ""}
              </span>
            </div>
          </div>
        ),
        size: 360,
        minSize: 220,
        meta: {
          headerTitle: "File",
          expandedContent: (row: FileReviewRow) => (
            <FileReviewDetails id={`${reviewId}-${encodeURIComponent(row.id)}`} row={row} />
          ),
        },
      },
      {
        accessorKey: "fileType",
        header: ({ column }) => <DataGridColumnHeader className={compactHeaderClassName} column={column} title="File type" />,
        cell: ({ row }) => <span className="text-xs">{row.original.fileType}</span>,
        size: 130,
        minSize: 100,
        meta: { headerTitle: "File type" },
      },
      {
        accessorKey: "sizeBytes",
        header: ({ column }) => <DataGridColumnHeader className={cn(compactHeaderClassName, "ml-auto")} column={column} title="File size" />,
        cell: ({ row }) => (
          <span className="block text-right text-xs tabular-nums">
            {row.original.sizeBytes === undefined ? "N/A" : formatFileSize(row.original.sizeBytes)}
          </span>
        ),
        sortUndefined: "last",
        size: 120,
        minSize: 100,
        meta: { headerTitle: "File size" },
      },
      {
        id: "category",
        header: () => <span className={compactHeaderClassName}>File category</span>,
        cell: () => <span className="text-xs text-muted-foreground">N/A</span>,
        enableSorting: false,
        size: 150,
        minSize: 120,
        meta: { headerTitle: "File category" },
      },
      {
        id: "addedAt",
        accessorFn: (row) => row.addedAt ? Date.parse(row.addedAt) : undefined,
        header: ({ column }) => (
          <DataGridColumnHeader className={compactHeaderClassName} column={column} title="Date added" />
        ),
        cell: ({ row }) => (
          <span className="text-xs tabular-nums">{formatShortUtcDate(row.original.addedAt)}</span>
        ),
        sortFn: "basic",
        sortUndefined: "last",
        size: 160,
        minSize: 140,
        meta: { headerTitle: "Date added" },
      },
      {
        id: "rlId",
        header: () => <span className={compactHeaderClassName}>RL ID</span>,
        cell: () => <span className="text-xs text-muted-foreground">Coming soon</span>,
        enableSorting: false,
        size: 150,
        minSize: 130,
        meta: { headerTitle: "RL ID" },
      },
    ],
    [onSelectFile, reviewId],
  );

  const table = useTable({
    features: dataGridFeatures,
    columns,
    data: visibleRows,
    getRowId: (row) => row.id,
    state: { columnPinning, expanded, pagination, sorting },
    getRowCanExpand: () => true,
    onExpandedChange: setExpanded,
    onColumnPinningChange: setColumnPinning,
    onPaginationChange: setPagination,
    onSortingChange: setSorting,
    autoResetPageIndex: false,
    autoResetExpanded: false,
  });

  const resetFilters = () => {
    updateQuery("");
    setStatusFilter("All");
    setTypeFilter("All types");
  };

  const collapseSearch = () => {
    updateQuery("");
    setSearchExpanded(false);
    requestAnimationFrame(() => searchTriggerRef.current?.focus());
  };

  return (
    <section className="file-review-table glass-panel workspace-pane relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center gap-4 border-b border-outline-variant bg-[var(--theme-workspace-surface)] px-5">
        <h1 className="min-w-0 flex-1 truncate text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">
          File review
        </h1>
        <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
          {rows.length} {rows.length === 1 ? "file" : "files"}
        </span>
      </header>
      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex shrink-0 flex-wrap items-center gap-3 px-5 py-3">
          <div
            data-slot="text-input-group"
            className={`relative h-8 shrink-0 overflow-hidden rounded-[8px] border border-input bg-[var(--theme-workspace-surface)] transition-[border-color,box-shadow] duration-150 ease-out motion-reduce:transition-none ${
              searchExpanded ? "w-64 max-sm:w-44" : "w-8"
            }`}
          >
            {searchExpanded ? (
              <>
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-2 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  aria-label="Search file reviews"
                  className="h-full rounded-[8px] border-0 bg-transparent pr-8 pl-8 text-xs shadow-none focus-visible:border-0 focus-visible:ring-0 md:text-xs dark:bg-transparent"
                  id="file-review-search"
                  onChange={(event) => updateQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Escape") {
                      collapseSearch();
                    }
                  }}
                  placeholder="Search files and findings"
                  ref={attachSearchInput}
                  type="search"
                  value={query}
                />
                <button
                  aria-label={
                    query
                      ? "Clear file review search"
                      : "Close file review search"
                  }
                  className="absolute right-1.5 top-1/2 grid size-5 -translate-y-1/2 place-items-center rounded-full text-muted-foreground hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onClick={() => {
                    if (query) {
                      updateQuery("");
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
                className="grid size-full place-items-center rounded-[8px] text-muted-foreground transition-colors hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
                onClick={() => setSearchExpanded(true)}
                ref={searchTriggerRef}
                type="button"
              >
                <Search aria-hidden="true" className="size-4" />
              </button>
            )}
          </div>
          <div
            className="flex max-w-full items-center gap-1 overflow-x-auto rounded-[8px] border border-outline-variant/70 bg-surface-container-low p-1"
            role="group"
            aria-label="Review status"
          >
            {(["All", "Needs attention", "Reviewed", "Pending"] as const).map(
              (status) => (
                <Button
                  aria-pressed={statusFilter === status}
                  className="shrink-0 rounded-[6px] border-0 px-3 text-xs motion-reduce:transition-none"
                  key={status}
                  onClick={() => {
                    setStatusFilter(status);
                    resetPage();
                  }}
                  size="sm"
                  type="button"
                  variant={statusFilter === status ? "default" : "ghost"}
                >
                  {status}
                </Button>
              ),
            )}
          </div>
          <div className="ml-auto flex items-center gap-2">
            <label
              className="text-xs text-muted-foreground"
              htmlFor="file-review-type"
            >
              File type
            </label>
            <select
              className="h-8 min-w-32 rounded-[8px] border border-input bg-[var(--theme-workspace-surface)] px-3 text-xs text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              id="file-review-type"
              onChange={(event) => {
                setTypeFilter(event.target.value as TypeFilter);
                resetPage();
              }}
              value={typeFilter}
            >
              {(["All types", ...FILE_TYPES] as const).map(
                (type) => (
                  <option key={type}>{type}</option>
                ),
              )}
            </select>
          </div>
          {query || statusFilter !== "All" || typeFilter !== "All types" ? (
            <Button
              onClick={resetFilters}
              size="sm"
              type="button"
              variant="ghost"
            >
              Reset
            </Button>
          ) : null}
        </div>
        <DataGrid
          emptyMessage="No file reviews match these filters."
          onRowClick={(row: FileReviewRow) => {
            if (window.getSelection()?.toString()) return;
            table.getRow(row.id).toggleExpanded();
            document.getElementById(`${reviewId}-toggle-${encodeURIComponent(row.id)}`)?.focus({ preventScroll: true });
          }}
          recordCount={visibleRows.length}
          table={table}
          tableClassNames={reviewTableClassNames}
          tableLayout={{
            cellBorder: false,
            columnsPinnable: true,
            dense: true,
            headerBackground: false,
            headerBorder: true,
            headerSticky: true,
            rowBorder: true,
            width: "fixed",
          }}
        >
          <DataGridContainer className="min-h-0 flex-1 border-t border-outline-variant/70">
            <DataGridScrollArea className="h-full" orientation="both">
              <DataGridTable />
            </DataGridScrollArea>
          </DataGridContainer>
          <div className="shrink-0 border-t border-outline-variant/70 px-5 py-3">
            <DataGridPagination sizes={[5, 10, 25, 50]} />
          </div>
        </DataGrid>
      </div>
    </section>
  );
}

function FileReviewDetails({ id, row }: { id: string; row: FileReviewRow }) {
  return (
    <section aria-label={`Review for ${row.fileName}`} className="file-review-details whitespace-normal bg-surface-container-low px-4 py-4 sm:px-5" id={id}>
      <p className="mb-4 text-[11px] text-muted-foreground">Illustrative review · Sample analysis, not generated from this file</p>
      <dl className="grid gap-x-8 gap-y-5">
        {([
          ["Key findings", row.keyFinding],
          ["Risks", row.risks],
          ["Opportunities", row.opportunities],
          ["Key question impact", row.impact],
        ] as const).map(([label, content]) => (
          <div className="min-w-0" key={label}>
            <dt className="mb-1 text-xs font-medium text-foreground">{label}</dt>
            <dd className="break-words text-xs leading-5 text-muted-foreground">{content}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
