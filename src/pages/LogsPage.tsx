import { useDeferredValue, useMemo, useState } from "react";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { Icon } from "../components/ui/Icon";
import {
  buildActivityLogExport,
  clearActivityLog,
  type ActivityLogEntry,
  type ActivityLogSource,
  type ActivityLogStatus,
  useActivityLogEntries,
} from "../lib/activityLog";
import { TAURI_COMMANDS } from "../lib/constants";
import { execute } from "../lib/tauri/command";

type SourceFilter = "all" | ActivityLogSource;
type StatusFilter = "all" | ActivityLogStatus;

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  hour: "numeric",
  minute: "2-digit",
  second: "2-digit",
});

const statusStyles: Record<ActivityLogStatus, string> = {
  error: "border-error/25 bg-error/10 text-error",
  info: "border-outline-variant bg-surface-container text-muted",
  pending: "border-amber-500/25 bg-amber-500/10 text-amber-700 dark:text-amber-300",
  success: "border-emerald-600/25 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
};

export function LogsPage() {
  const entries = useActivityLogEntries();
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>("all");
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [search, setSearch] = useState("");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [exportError, setExportError] = useState("");
  const [exporting, setExporting] = useState(false);
  const deferredSearch = useDeferredValue(search.trim().toLocaleLowerCase());

  const visibleEntries = useMemo(
    () =>
      entries.filter((entry) => {
        if (sourceFilter !== "all" && entry.source !== sourceFilter) {
          return false;
        }
        if (statusFilter !== "all" && entry.status !== statusFilter) {
          return false;
        }
        if (!deferredSearch) {
          return true;
        }
        return [entry.title, entry.operation, entry.eventName, entry.details]
          .filter(Boolean)
          .some((value) => value?.toLocaleLowerCase().includes(deferredSearch));
      }),
    [deferredSearch, entries, sourceFilter, statusFilter],
  );

  const counts = useMemo(
    () => ({
      errors: entries.filter((entry) => entry.status === "error").length,
      events: entries.filter((entry) => entry.source === "event").length,
      ipc: entries.filter((entry) => entry.source === "ipc").length,
      success: entries.filter((entry) => entry.status === "success").length,
    }),
    [entries],
  );

  async function handleExport() {
    setExportError("");
    setExporting(true);
    try {
      await execute<boolean>(TAURI_COMMANDS.exportActivityLog, {
        payload: buildActivityLogExport(),
      });
    } catch (error) {
      setExportError(error instanceof Error ? error.message : String(error));
    } finally {
      setExporting(false);
    }
  }

  return (
    <WorkspaceHomeShell
      activeHomeSection="logs"
      header={<WorkspaceHeader title="Logs" />}
    >
      <div className="mx-auto flex w-full max-w-[1180px] flex-col gap-6 pb-10">
        <section className="flex flex-col justify-between gap-4 lg:flex-row lg:items-end">
          <div>
            <div className="flex items-center gap-2">
              <span className="h-2.5 w-2.5 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]" />
              <p className="text-[11px] font-bold uppercase tracking-[0.18em] text-muted">Live session capture</p>
            </div>
            <h2 className="mt-3 type-display text-text-main">Desktop activity</h2>
            <p className="mt-2 max-w-2xl text-[14px] leading-6 text-muted">
              Redacted IPC commands and Tauri events from this app session. Logs survive navigation and never
              interrupt the operation they observe.
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              className="inline-flex items-center gap-2 rounded-xl border border-outline-variant bg-surface px-4 py-2.5 text-[12px] font-semibold text-text-main transition hover:bg-surface-container-low disabled:opacity-45"
              disabled={entries.length === 0 || exporting}
              onClick={() => void handleExport()}
              type="button"
            >
              <Icon className="h-4 w-4" name="upload" />
              {exporting ? "Exporting…" : "Export JSON"}
            </button>
            <button
              className="rounded-xl border border-outline-variant px-4 py-2.5 text-[12px] font-semibold text-muted transition hover:border-error/30 hover:bg-error/5 hover:text-error disabled:opacity-45"
              disabled={entries.length === 0}
              onClick={() => {
                clearActivityLog();
                setExpandedId(null);
              }}
              type="button"
            >
              Clear session
            </button>
          </div>
        </section>

        {exportError ? <p className="text-[12px] text-error" role="alert">{exportError}</p> : null}

        <section aria-label="Log summary" className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <SummaryCard label="IPC commands" value={counts.ipc} />
          <SummaryCard label="Events" value={counts.events} />
          <SummaryCard label="Successful" tone="success" value={counts.success} />
          <SummaryCard label="Errors" tone={counts.errors > 0 ? "error" : "default"} value={counts.errors} />
        </section>

        <section className="overflow-hidden rounded-[22px] border border-outline-variant bg-surface shadow-[0_12px_34px_rgba(7,1,84,0.06)]">
          <div className="flex flex-col gap-3 border-b border-outline-variant bg-surface-container-low/60 p-4 lg:flex-row lg:items-center">
            <label className="relative min-w-0 flex-1">
              <span className="sr-only">Search logs</span>
              <Icon className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-muted" name="search" />
              <input
                className="h-11 w-full rounded-xl border border-outline-variant bg-surface pl-10 pr-4 text-[13px] text-text-main outline-none transition placeholder:text-muted/75 focus:border-primary"
                onChange={(event) => setSearch(event.target.value)}
                placeholder="Search command, event, status, or payload…"
                type="search"
                value={search}
              />
            </label>
            <div className="grid grid-cols-2 gap-2 lg:flex">
              <FilterSelect
                label="Source"
                onChange={(value) => setSourceFilter(value as SourceFilter)}
                options={[["all", "All sources"], ["ipc", "IPC"], ["event", "Events"]]}
                value={sourceFilter}
              />
              <FilterSelect
                label="Status"
                onChange={(value) => setStatusFilter(value as StatusFilter)}
                options={[["all", "All statuses"], ["success", "Success"], ["error", "Errors"], ["pending", "Pending"], ["info", "Info"]]}
                value={statusFilter}
              />
            </div>
          </div>

          <div className="flex items-center justify-between border-b border-outline-variant px-5 py-3 text-[11px] font-semibold uppercase tracking-[0.12em] text-muted">
            <span>{visibleEntries.length} {visibleEntries.length === 1 ? "entry" : "entries"}</span>
            <span>Newest first</span>
          </div>

          {visibleEntries.length > 0 ? (
            <div className="divide-y divide-outline-variant">
              {visibleEntries.map((entry) => (
                <LogRow
                  entry={entry}
                  expanded={expandedId === entry.id}
                  key={entry.id}
                  onToggle={setExpandedId}
                />
              ))}
            </div>
          ) : (
            <EmptyState hasEntries={entries.length > 0} />
          )}
        </section>
      </div>
    </WorkspaceHomeShell>
  );
}

function SummaryCard({
  label,
  tone = "default",
  value,
}: {
  label: string;
  tone?: "default" | "error" | "success";
  value: number;
}) {
  const valueClassName = tone === "error" ? "text-error" : tone === "success" ? "text-emerald-700 dark:text-emerald-300" : "text-text-main";

  return (
    <div className="rounded-[18px] border border-outline-variant bg-surface px-5 py-4 shadow-[0_8px_24px_rgba(7,1,84,0.04)]">
      <p className="text-[10px] font-bold uppercase tracking-[0.16em] text-muted">{label}</p>
      <p className={`mt-2 text-[28px] font-semibold leading-none ${valueClassName}`}>{value}</p>
    </div>
  );
}

function FilterSelect({ label, onChange, options, value }: {
  label: string;
  onChange: (value: string) => void;
  options: [string, string][];
  value: string;
}) {
  return (
    <label>
      <span className="sr-only">Filter by {label.toLocaleLowerCase()}</span>
      <select
        className="h-11 min-w-36 rounded-xl border border-outline-variant bg-surface px-3 text-[12px] font-semibold text-text-main outline-none transition focus:border-primary"
        onChange={(event) => onChange(event.target.value)}
        value={value}
      >
        {options.map(([optionValue, optionLabel]) => (
          <option key={optionValue} value={optionValue}>{optionLabel}</option>
        ))}
      </select>
    </label>
  );
}

function LogRow({ entry, expanded, onToggle }: {
  entry: ActivityLogEntry;
  expanded: boolean;
  onToggle: (id: string | null) => void;
}) {
  const metadata = [
    entry.eventName,
    entry.durationMs !== undefined ? `${entry.durationMs} ms` : undefined,
  ].filter(Boolean);

  return (
    <article className="log-list-item bg-surface transition-colors hover:bg-surface-container-low/55">
      <button
        aria-expanded={expanded}
        className="grid w-full grid-cols-[auto_minmax(0,1fr)_auto] items-start gap-3 px-5 py-4 text-left"
        onClick={() => onToggle(expanded ? null : entry.id)}
        type="button"
      >
        <span className={`mt-0.5 inline-flex min-w-14 justify-center rounded-md border px-2 py-1 text-[9px] font-bold uppercase tracking-[0.12em] ${statusStyles[entry.status]}`}>
          {entry.status}
        </span>
        <span className="min-w-0">
          <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <span className="rounded bg-primary/8 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-[0.13em] text-primary">
              {entry.source}
            </span>
            <span className="truncate text-[13px] font-semibold text-text-main">{entry.title}</span>
          </span>
          <span className="mt-1.5 block truncate font-mono text-[11px] text-muted">{entry.operation}</span>
          {metadata.length > 0 ? (
            <span className="mt-2 flex flex-wrap gap-x-3 text-[10px] font-semibold uppercase tracking-[0.1em] text-muted">
              {metadata.map((item) => <span key={item}>{item}</span>)}
            </span>
          ) : null}
        </span>
        <span className="flex items-center gap-2 whitespace-nowrap text-[11px] text-muted">
          <time dateTime={entry.occurredAt}>{formatTime(entry.occurredAt)}</time>
          <Icon className={`h-4 w-4 transition-transform ${expanded ? "rotate-180" : ""}`} name="chevronDown" />
        </span>
      </button>
      {expanded ? (
        <div className="border-t border-outline-variant bg-[#080d1c] px-5 py-4 text-[#d6def0]">
          <div className="mb-3 grid gap-2 text-[11px] sm:grid-cols-2">
            <Detail label="Timestamp" value={entry.occurredAt} />
            <Detail label="Operation" value={entry.operation} />
          </div>
          <pre className="max-h-[420px] overflow-auto whitespace-pre-wrap break-words rounded-xl border border-white/10 bg-black/20 p-4 font-mono text-[11px] leading-5">
            {entry.details ?? "No payload details were recorded for this entry."}
          </pre>
        </div>
      ) : null}
    </article>
  );
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <p className="min-w-0">
      <span className="mr-2 font-semibold uppercase tracking-[0.1em] text-[#7f8ca8]">{label}</span>
      <span className="break-all font-mono">{value}</span>
    </p>
  );
}

function EmptyState({ hasEntries }: { hasEntries: boolean }) {
  return (
    <div className="flex min-h-72 flex-col items-center justify-center px-6 text-center">
      <span className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/8 text-primary">
        <Icon className="h-6 w-6" name={hasEntries ? "filter" : "terminal"} />
      </span>
      <h3 className="mt-4 text-[16px] font-semibold text-text-main">
        {hasEntries ? "No matching entries" : "Waiting for activity"}
      </h3>
      <p className="mt-2 max-w-md text-[13px] leading-5 text-muted">
        {hasEntries
          ? "Try a different search or filter combination."
          : "IPC commands and Tauri events will appear here automatically as you use Quarry."}
      </p>
    </div>
  );
}

function formatTime(value: string) {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : dateTimeFormatter.format(date);
}
