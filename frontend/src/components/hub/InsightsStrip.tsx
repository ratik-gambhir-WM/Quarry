import { useEffect, useRef, useState } from "react";
import type { WorkspaceInsight } from "../../data/workspace";
import { DataTableHeaderRow, DataTableHeading } from "../ui/DataTable";
import { Icon } from "../ui/Icon";
import { ViewColumnsIcon } from "../ui/icons/ViewColumnsIcon";
import { WorkspaceCard } from "./WorkspaceCard";

type InsightsStripProps = {
  className?: string;
  contextLabel?: string;
  items: readonly WorkspaceInsight[];
};

const shimmerClassName =
  "animate-[shimmer_1.5s_linear_infinite] bg-[linear-gradient(90deg,rgba(255,255,255,0)_0%,rgba(255,255,255,0.4)_50%,rgba(255,255,255,0)_100%)] [background-size:200%_100%]";

const reviewColumns = [
  { key: "category", label: "Category" },
  { key: "finding", label: "Extracted finding" },
  { key: "confidence", label: "Confidence" },
  { key: "status", label: "Review status" },
] as const;

type ReviewColumnKey = (typeof reviewColumns)[number]["key"];
type ReviewStatus = "Needs review" | "Reviewed";

const defaultVisibleColumns: ReviewColumnKey[] = ["category", "finding", "confidence", "status"];

export function InsightsStrip({ className = "col-span-12 mt-2", contextLabel = "Across Projects", items }: InsightsStripProps) {
  return (
    <section className={className}>
      <div className="mb-4 flex items-center justify-between px-2">
        <h2 className="type-h3 flex items-center gap-3 text-text-main">
          <Icon className="h-6 w-6 text-accent" name="dataset" />
          Analyzed Files
        </h2>
        <span className="type-label text-muted">{contextLabel}</span>
      </div>

      <div className="workspace-scrollbar-hidden flex gap-4 overflow-x-auto px-2 pb-6 pt-2">
        {items.map((item) => (
          <InsightChip item={item} key={`${item.deal}-${item.category}`} />
        ))}

        <LoadingChip />

        <div className="flex w-12 shrink-0 items-center justify-center rounded-2xl border-2 border-dashed border-muted/25 text-muted/55 transition hover:bg-white/20 hover:text-muted">
          <Icon className="h-5 w-5" name="arrowRight" />
        </div>
      </div>

      <div className="mx-2 h-px bg-outline-variant/70" />

      <ReviewTable items={items} />
    </section>
  );
}

function ReviewTable({ items }: { items: readonly WorkspaceInsight[] }) {
  const [visibleColumns, setVisibleColumns] = useState<ReviewColumnKey[]>(defaultVisibleColumns);
  const [columnMenuOpen, setColumnMenuOpen] = useState(false);
  const [reviewStatuses, setReviewStatuses] = useState<Record<string, ReviewStatus>>({});
  const columnMenuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!columnMenuRef.current?.contains(event.target as Node)) {
        setColumnMenuOpen(false);
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setColumnMenuOpen(false);
      }
    }

    if (columnMenuOpen) {
      document.addEventListener("pointerdown", handlePointerDown);
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [columnMenuOpen]);

  function toggleColumn(column: ReviewColumnKey) {
    setVisibleColumns((currentColumns) =>
      currentColumns.includes(column)
        ? currentColumns.filter((currentColumn) => currentColumn !== column)
        : [...currentColumns, column],
    );
  }

  function toggleReviewStatus(fileName: string) {
    setReviewStatuses((currentStatuses) => ({
      ...currentStatuses,
      [fileName]: currentStatuses[fileName] === "Reviewed" ? "Needs review" : "Reviewed",
    }));
  }

  return (
    <div className="px-2 pt-6">
      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h3 className="type-h2 text-text-main">Review table</h3>
          <p className="mt-1 text-[13px] text-muted">Review extracted information across analyzed documents.</p>
        </div>

        <div className="relative self-start sm:self-auto" ref={columnMenuRef}>
          <button
            aria-expanded={columnMenuOpen}
            aria-haspopup="menu"
            className="group flex h-10 items-center gap-2 rounded-xl border border-outline-variant bg-surface-container-lowest px-3.5 text-[13px] font-semibold text-text-main transition hover:bg-surface-container-low focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            onClick={() => setColumnMenuOpen((isOpen) => !isOpen)}
            type="button"
          >
            <ViewColumnsIcon className="h-4 w-4 text-muted" />
            Customize columns
            <Icon className={`h-3.5 w-3.5 text-muted transition ${columnMenuOpen ? "rotate-180" : ""}`} name="chevronDown" />
          </button>

          {columnMenuOpen ? (
            <div
              className="absolute right-0 top-[calc(100%+0.5rem)] z-30 w-60 rounded-2xl border border-outline-variant bg-surface-container-lowest p-2 shadow-[0_18px_44px_rgba(7,1,84,0.14)]"
              role="menu"
            >
              <p className="px-3 pb-2 pt-1 text-[10px] font-bold uppercase tracking-[0.16em] text-muted">
                Extracted fields
              </p>
              {reviewColumns.map((column) => {
                const visible = visibleColumns.includes(column.key);

                return (
                  <button
                    aria-checked={visible}
                    className="flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left text-[13px] font-medium text-text-main transition hover:bg-surface-container-high"
                    key={column.key}
                    onClick={() => toggleColumn(column.key)}
                    role="menuitemcheckbox"
                    type="button"
                  >
                    <span>{column.label}</span>
                    <span
                      className={`flex h-5 w-5 items-center justify-center rounded-md border ${
                        visible ? "border-primary bg-primary text-white" : "border-outline-variant text-transparent"
                      }`}
                    >
                      <Icon className="h-3.5 w-3.5" name="check" />
                    </span>
                  </button>
                );
              })}
            </div>
          ) : null}
        </div>
      </div>

      <WorkspaceCard className="overflow-hidden" radius="compact">
        <div className="workspace-scrollbar-hidden overflow-x-auto">
          <table className="w-full min-w-[860px] border-collapse text-left">
            <thead>
              <DataTableHeaderRow surface>
                <DataTableHeading className="min-w-64" density="compact">Document</DataTableHeading>
                {visibleColumns.includes("category") ? <DataTableHeading density="compact">Category</DataTableHeading> : null}
                {visibleColumns.includes("finding") ? (
                  <DataTableHeading className="min-w-96" density="compact">Extracted finding</DataTableHeading>
                ) : null}
                {visibleColumns.includes("confidence") ? <DataTableHeading density="compact">Confidence</DataTableHeading> : null}
                {visibleColumns.includes("status") ? <DataTableHeading density="compact">Review status</DataTableHeading> : null}
              </DataTableHeaderRow>
            </thead>
            <tbody className="divide-y divide-outline-variant/55">
              {items.length > 0 ? (
                items.map((item, index) => {
                  const status = reviewStatuses[item.fileName] ?? "Needs review";
                  const confidence = item.image ? "92%" : index % 2 === 0 ? "96%" : "94%";

                  return (
                    <tr className="group transition hover:bg-surface-container-low/55" key={`${item.deal}-${item.fileName}`}>
                      <td className="px-4 py-4 align-top">
                        <div className="flex items-center gap-3">
                          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary/8 text-primary">
                            <Icon className="h-4.5 w-4.5" name={item.fileIcon} />
                          </span>
                          <div className="min-w-0">
                            <p className="max-w-56 truncate text-[13px] font-semibold text-text-main">{item.fileName}</p>
                            <p className="mt-1 text-[11px] text-muted">Analyzed today</p>
                          </div>
                        </div>
                      </td>
                      {visibleColumns.includes("category") ? (
                        <td className="px-4 py-4 align-top">
                          <span className="inline-flex whitespace-nowrap rounded-lg bg-surface-container-high px-2.5 py-1.5 text-[11px] font-bold uppercase tracking-[0.08em] text-muted">
                            {item.category}
                          </span>
                        </td>
                      ) : null}
                      {visibleColumns.includes("finding") ? (
                        <td className="px-4 py-4 align-top text-[13px] leading-6 text-text-main/82">{item.quote}</td>
                      ) : null}
                      {visibleColumns.includes("confidence") ? (
                        <td className="px-4 py-4 align-top">
                          <span className="inline-flex items-center gap-2 text-[13px] font-semibold text-text-main">
                            <span className="h-2 w-2 rounded-full bg-primary" />
                            {confidence}
                          </span>
                        </td>
                      ) : null}
                      {visibleColumns.includes("status") ? (
                        <td className="px-4 py-4 align-top">
                          <button
                            aria-label={`Mark ${item.fileName} as ${status === "Reviewed" ? "needing review" : "reviewed"}`}
                            className={`whitespace-nowrap rounded-xl border px-3 py-2 text-[12px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
                              status === "Reviewed"
                                ? "border-primary/20 bg-primary/10 text-primary"
                                : "border-outline-variant bg-surface-container-lowest text-muted hover:text-text-main"
                            }`}
                            onClick={() => toggleReviewStatus(item.fileName)}
                            type="button"
                          >
                            {status}
                          </button>
                        </td>
                      ) : null}
                    </tr>
                  );
                })
              ) : (
                <tr>
                  <td className="px-6 py-10 text-center text-[13px] text-muted" colSpan={visibleColumns.length + 1}>
                    Analyzed documents will appear here when extraction is complete.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </WorkspaceCard>
    </div>
  );
}

function InsightChip({ item }: { item: WorkspaceInsight }) {
  return (
    <article className="chip-card group relative flex w-72 shrink-0 flex-col gap-3 overflow-hidden p-4">
      <div className={`absolute left-0 top-0 h-full w-1 ${item.toneClassName}`} />
      <div className="flex items-start justify-between pl-2">
        <div className="space-y-1">
          <p className={`text-[11px] font-bold uppercase tracking-[0.16em] ${item.toneTextClassName}`}>{item.deal}</p>
          <span className="inline-flex rounded-md bg-surface-container-low px-2 py-1 text-[11px] font-bold uppercase tracking-[0.1em] text-muted">
            {item.category}
          </span>
        </div>
        <Icon className="h-4 w-4 text-muted opacity-0 transition group-hover:opacity-100" name="openInNew" />
      </div>

      {item.image ? <div className="ml-2 h-20 rounded-xl border border-white/60 bg-[linear-gradient(120deg,rgba(80,101,142,0.22),rgba(206,215,230,0.5),rgba(232,238,248,0.8))]" /> : null}

      <p className="line-clamp-3 pl-2 font-heading text-[0.98rem] leading-relaxed text-text-main">{item.quote}</p>

      <div className="mt-auto flex items-center gap-2 pl-2">
        <Icon className="h-4 w-4 text-muted" name={item.fileIcon} />
        <span className="truncate text-xs text-muted">{item.fileName}</span>
      </div>
    </article>
  );
}

function LoadingChip() {
  return (
    <WorkspaceCard className="relative flex w-72 shrink-0 flex-col gap-4 overflow-hidden p-4 opacity-55">
      <div className="absolute left-0 top-0 h-full w-1 bg-muted/25" />
      <div className="space-y-2 pl-2 pt-1">
        <div className={`${shimmerClassName} h-2 w-24 rounded-full bg-muted/10`} />
        <div className={`${shimmerClassName} h-2 w-16 rounded-full bg-muted/10`} />
      </div>
      <div className={`${shimmerClassName} ml-2 mt-4 h-3 w-full rounded-full bg-muted/10`} />
      <div className={`${shimmerClassName} ml-2 h-3 w-5/6 rounded-full bg-muted/10`} />
      <div className="mt-auto flex items-center gap-2 pl-2">
        <div className={`${shimmerClassName} h-4 w-4 rounded-full bg-muted/10`} />
        <div className={`${shimmerClassName} h-2 w-32 rounded-full bg-muted/10`} />
      </div>
    </WorkspaceCard>
  );
}
