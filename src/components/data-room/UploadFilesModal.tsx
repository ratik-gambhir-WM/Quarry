import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { productApi } from "../../lib/product";
import type {
  DocumentJobEvent,
  DocumentJobStatus,
  SelectedLocalFile,
} from "../../lib/product/types";
import { useTauriEvent } from "../../lib/tauri/useTauriEvent";
import { Icon } from "../ui/Icon";

type UploadFilesModalProps = {
  onClose: () => void;
  userId: string;
};

export type UploadStatus = "ready" | DocumentJobStatus;

export type UploadEntry = SelectedLocalFile & {
  chunkCount?: number;
  documentId?: string;
  error?: string;
  id: string;
  jobId?: string;
  selected: boolean;
  status: UploadStatus;
};

const JOB_EVENT = "documents:job";

export function UploadFilesModal({ onClose, userId }: UploadFilesModalProps) {
  const chooseFilesButtonRef = useRef<HTMLButtonElement>(null);
  const processingStartedAtRef = useRef<number | null>(null);
  const [entries, setEntries] = useState<UploadEntry[]>([]);
  const [dragActive, setDragActive] = useState(false);
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [processedSeconds, setProcessedSeconds] = useState<number | null>(null);
  const [selectionError, setSelectionError] = useState("");
  const activeCount = entries.filter((entry) => entry.status === "processing").length;
  const isProcessing = activeCount > 0;
  const uploadableEntries = entries.filter(
    (entry) => entry.selected && (entry.status === "ready" || entry.status === "failed") && entry.path,
  );
  const selectedCount = entries.filter((entry) => entry.selected).length;
  const storageKey = useMemo(() => `quarry.document-jobs:${userId}`, [userId]);

  const addSelectedFiles = useCallback((files: SelectedLocalFile[]) => {
    setEntries((current) => {
      const paths = new Set(current.map((entry) => entry.path).filter(Boolean));
      return [
        ...current,
        ...files
          .filter((file) => !paths.has(file.path))
          .map((file) => ({
            ...file,
            id: file.path,
            selected: true,
            status: "ready" as const,
          })),
      ];
    });
  }, []);

  const addDroppedFiles = useCallback(
    async (paths: string[]) => {
      setSelectionError("");
      try {
        addSelectedFiles(await productApi.describeDocumentFiles(paths));
      } catch (caught) {
        setSelectionError(caught instanceof Error ? caught.message : String(caught));
      }
    },
    [addSelectedFiles],
  );

  useEffect(() => {
    chooseFilesButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    let mounted = true;
    let unsubscribe: (() => void) | undefined;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!mounted) {
          return;
        }
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDragActive(true);
        } else if (event.payload.type === "drop") {
          setDragActive(false);
          void addDroppedFiles(event.payload.paths);
        } else {
          setDragActive(false);
        }
      })
      .then((unlisten) => {
        if (mounted) {
          unsubscribe = unlisten;
        } else {
          unlisten();
        }
      })
      .catch(() => {
        // The browser-only development preview does not expose native drag events.
      });
    return () => {
      mounted = false;
      unsubscribe?.();
    };
  }, [addDroppedFiles]);

  const applyJobEvent = useCallback((event: DocumentJobEvent) => {
    setEntries((current) => {
      let changed = false;
      const next = current.map((entry) => {
        if (entry.jobId !== event.jobId) {
          return entry;
        }
        if (
          entry.chunkCount === event.chunkCount &&
          entry.documentId === event.documentId &&
          entry.error === event.error &&
          entry.status === event.status
        ) {
          return entry;
        }
        changed = true;
        return {
          ...entry,
          chunkCount: event.chunkCount,
          documentId: event.documentId,
          error: event.error,
          status: event.status,
        };
      });
      return changed ? next : current;
    });
  }, []);

  useTauriEvent<DocumentJobEvent>(JOB_EVENT, (event) => applyJobEvent(event.payload));

  useEffect(() => {
    try {
      const stored = window.sessionStorage.getItem(storageKey);
      const jobs = stored ? (JSON.parse(stored) as DocumentJobEvent[]) : [];
      if (!Array.isArray(jobs)) {
        return;
      }
      setEntries(
        jobs.map((job) => ({
          chunkCount: job.chunkCount,
          documentId: job.documentId,
          error: job.error,
          id: job.jobId,
          jobId: job.jobId,
          name: job.filename,
          path: "",
          selected: job.status !== "failed",
          sizeBytes: 0,
          status: job.status,
        })),
      );
      for (const job of jobs) {
        void productApi.getDocumentJob(job.jobId).then(applyJobEvent).catch(() => undefined);
      }
    } catch {
      // A malformed remount cache should not block fresh native selection.
    }
  }, [applyJobEvent, storageKey]);

  useEffect(() => {
    const jobs: DocumentJobEvent[] = entries.flatMap((entry) =>
      entry.jobId
        ? [{
            chunkCount: entry.chunkCount,
            documentId: entry.documentId,
            error: entry.error,
            filename: entry.name,
            jobId: entry.jobId,
            status: entry.status === "ready" ? "processing" : entry.status,
          }]
        : [],
    );
    try {
      window.sessionStorage.setItem(storageKey, JSON.stringify(jobs.slice(-50)));
    } catch {
      // Remount recovery is best-effort and must not interrupt processing.
    }
  }, [entries, storageKey]);

  useEffect(() => {
    const startedAt = processingStartedAtRef.current;
    if (startedAt === null) {
      return;
    }

    if (!isProcessing) {
      const totalSeconds = Math.max(1, Math.ceil((Date.now() - startedAt) / 1_000));
      setElapsedSeconds(totalSeconds);
      setProcessedSeconds(totalSeconds);
      processingStartedAtRef.current = null;
      return;
    }

    const updateElapsedSeconds = () => {
      setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1_000));
    };
    updateElapsedSeconds();
    const intervalId = window.setInterval(updateElapsedSeconds, 1_000);
    return () => window.clearInterval(intervalId);
  }, [isProcessing]);

  useEffect(() => {
    const processingJobIds = entries.flatMap((entry) =>
      entry.status === "processing" && entry.jobId ? [entry.jobId] : [],
    );
    if (processingJobIds.length === 0) {
      return;
    }
    let cancelled = false;
    const refresh = () => {
      for (const jobId of processingJobIds) {
        void productApi.getDocumentJob(jobId).then((job) => {
          if (!cancelled) {
            applyJobEvent(job);
          }
        }).catch(() => undefined);
      }
    };
    refresh();
    const timer = window.setInterval(refresh, 2_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [applyJobEvent, entries]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && activeCount === 0) {
        onClose();
      }
    }
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [activeCount, onClose]);

  async function chooseFiles() {
    setSelectionError("");
    try {
      addSelectedFiles(await productApi.selectDocumentFiles());
    } catch (caught) {
      setSelectionError(caught instanceof Error ? caught.message : String(caught));
    }
  }

  function toggleEntry(entryId: string) {
    setEntries((current) =>
      current.map((entry) =>
        entry.id === entryId &&
        entry.status !== "processing" &&
        entry.status !== "completed" &&
        entry.status !== "skipped"
          ? { ...entry, selected: !entry.selected }
          : entry,
      ),
    );
  }

  function removeEntry(entryId: string) {
    setEntries((current) =>
      current.filter((entry) => entry.id !== entryId || entry.status === "processing"),
    );
  }

  async function startJobs() {
    if (!userId.trim()) {
      setSelectionError("Sign in again before uploading files.");
      return;
    }
    if (uploadableEntries.length === 0) {
      return;
    }

    const uploadIds = new Set(uploadableEntries.map((entry) => entry.id));
    processingStartedAtRef.current = Date.now();
    setElapsedSeconds(0);
    setProcessedSeconds(null);
    setSelectionError("");
    setEntries((current) =>
      current.map((entry) =>
        uploadIds.has(entry.id)
          ? {
              ...entry,
              chunkCount: undefined,
              documentId: undefined,
              error: undefined,
              jobId: undefined,
              status: "processing",
            }
          : entry,
      ),
    );

    try {
      const response = await productApi.startDocumentJobs({
        paths: uploadableEntries.map((entry) => entry.path),
        userId,
      });
      const byId = new Map(
        uploadableEntries.map((entry, index) => [entry.id, response.jobs[index]]),
      );
      setEntries((current) =>
        current.map((entry) => {
          const job = byId.get(entry.id);
          return job
            ? { ...entry, error: undefined, jobId: job.jobId, status: job.status }
            : entry;
        }),
      );
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : String(caught);
      setEntries((current) =>
        current.map((entry) =>
          uploadIds.has(entry.id) ? { ...entry, error: message, status: "failed" } : entry,
        ),
      );
    }
  }

  return (
    <div
      className="fixed inset-0 z-[100] flex items-center justify-center bg-text-main/30 px-4 py-6 backdrop-blur-sm"
      role="presentation"
    >
      <button
        aria-label="Close upload files dialog"
        className="absolute inset-0 cursor-default disabled:cursor-wait"
        disabled={activeCount > 0}
        onClick={onClose}
        type="button"
      />

      <section
        aria-labelledby="upload-files-title"
        aria-modal="true"
        className="relative z-10 flex max-h-[calc(100vh-3rem)] w-full max-w-[680px] flex-col overflow-hidden rounded-[20px] border border-outline-variant bg-surface-container-lowest shadow-[0_28px_70px_rgba(7,1,84,0.24)]"
        role="dialog"
      >
        <header className="flex items-start justify-between gap-5 border-b border-outline-variant px-6 py-5">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">
              Data Room
            </p>
            <h2
              className="mt-2 text-[2rem] font-bold leading-none text-text-main [font-family:var(--font-heading)]"
              id="upload-files-title"
            >
              Upload files
            </h2>
            <p className="mt-2 text-[13px] leading-5 text-muted">
              Choose PDF and DOCX files from your Mac, then select which ones to process.
            </p>
          </div>
          <button
            aria-label="Close upload files dialog"
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-40"
            disabled={activeCount > 0}
            onClick={onClose}
            type="button"
          >
            <Icon className="h-5 w-5 rotate-45" name="plus" />
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          <div
            className={`flex min-h-28 flex-col items-center justify-center rounded-2xl border border-dashed px-6 py-5 text-center transition ${
              dragActive
                ? "border-primary bg-surface-container-high ring-2 ring-primary/15"
                : "border-primary/40 bg-surface-container-low hover:border-primary/70 hover:bg-surface-container-high"
            }`}
          >
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 text-primary">
              <Icon className="h-5 w-5" name="upload" />
            </div>
            <p className="mt-3 text-[13px] font-semibold text-text-main">
              Drop PDF or DOCX files here
            </p>
            <button
              className="mt-2 text-[12px] font-semibold text-primary underline decoration-primary/30 underline-offset-4 transition hover:decoration-primary"
              onClick={() => void chooseFiles()}
              ref={chooseFilesButtonRef}
              type="button"
            >
              Browse files on your Mac
            </button>
          </div>

          {selectionError ? (
            <p
              className="mt-4 rounded-xl border border-error/25 bg-error-container/40 px-4 py-3 text-[12px] font-medium leading-5 text-error"
              role="alert"
            >
              {selectionError}
            </p>
          ) : null}

          {entries.length > 0 ? (
            <div className="mt-5 overflow-hidden rounded-2xl border border-outline-variant">
              <div className="flex items-center justify-between border-b border-outline-variant bg-surface-container-low px-4 py-3">
                <p className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted">
                  Selected files
                </p>
                <span className="text-[11px] font-semibold text-muted">
                  {selectedCount} of {entries.length} checked
                </span>
              </div>
              <ul aria-live="polite" className="divide-y divide-outline-variant">
                {entries.map((entry) => (
                  <UploadFileRow
                    entry={entry}
                    key={entry.id}
                    onRemove={removeEntry}
                    onToggle={toggleEntry}
                  />
                ))}
              </ul>
            </div>
          ) : null}
        </div>

        <UploadFooter
          activeCount={activeCount}
          elapsedSeconds={elapsedSeconds}
          hasCompletedEntries={entries.some(
            (entry) => entry.status === "completed" || entry.status === "skipped",
          )}
          isProcessing={isProcessing}
          onClose={onClose}
          onStart={() => void startJobs()}
          processedSeconds={processedSeconds}
          retrying={uploadableEntries.some((entry) => entry.status === "failed")}
          uploadableCount={uploadableEntries.length}
        />
      </section>
    </div>
  );
}

type UploadFileRowProps = {
  entry: UploadEntry;
  onRemove: (entryId: string) => void;
  onToggle: (entryId: string) => void;
};

export function UploadFileRow({ entry, onRemove, onToggle }: UploadFileRowProps) {
  const active = entry.status === "processing";
  const completed = entry.status === "completed";
  const skipped = entry.status === "skipped";
  const failed = entry.status === "failed";

  return (
    <li className="flex items-start gap-3 bg-surface-container-lowest px-4 py-3.5">
      <input
        aria-label={`Select ${entry.name}`}
        checked={entry.selected}
        className="mt-1 h-4 w-4 shrink-0 accent-[#0c006b]"
        disabled={!entry.path || active || completed || skipped}
        onChange={() => onToggle(entry.id)}
        type="checkbox"
      />
      <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-surface-container text-primary">
        <Icon
          className="h-5 w-5"
          name={getFileExtension(entry.name) === "pdf" ? "pdf" : "doc"}
        />
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-semibold text-text-main" title={entry.name}>
          {entry.name}
        </p>
        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted">
          <span>{formatFileSize(entry.sizeBytes)}</span>
          <span aria-hidden="true">·</span>
          <span
            className={
              failed
                ? "font-semibold text-error"
                : completed || skipped
                  ? "font-semibold text-[#168447]"
                  : ""
            }
          >
            {getStatusLabel(entry)}
          </span>
        </div>
        {entry.error ? (
          <p className={`mt-1 text-[11px] leading-4 ${failed ? "text-error" : "text-muted"}`}>
            {entry.error}
          </p>
        ) : null}
      </div>
      <div className="flex h-9 w-9 shrink-0 items-center justify-center">
        {completed || skipped ? (
          <span
            aria-label={skipped ? "Already processed" : "Processing complete"}
            className="flex h-7 w-7 items-center justify-center rounded-full bg-[#dff5e8] text-[#168447]"
            title={skipped ? "Already processed" : "Processing complete"}
          >
            <Icon className="h-4 w-4 stroke-[2.5]" name="check" />
          </span>
        ) : active ? (
          <span
            aria-label="Processing"
            className="h-5 w-5 rounded-full border-2 border-primary/25 border-t-primary motion-safe:animate-spin"
          />
        ) : (
          <button
            aria-label={`Remove ${entry.name}`}
            className="flex h-8 w-8 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-error"
            onClick={() => onRemove(entry.id)}
            type="button"
          >
            <Icon className="h-4 w-4 rotate-45" name="plus" />
          </button>
        )}
      </div>
    </li>
  );
}

type UploadFooterProps = {
  activeCount: number;
  elapsedSeconds: number;
  hasCompletedEntries: boolean;
  isProcessing: boolean;
  onClose: () => void;
  onStart: () => void;
  processedSeconds: number | null;
  retrying: boolean;
  uploadableCount: number;
};

export function UploadFooter({
  activeCount,
  elapsedSeconds,
  hasCompletedEntries,
  isProcessing,
  onClose,
  onStart,
  processedSeconds,
  retrying,
  uploadableCount,
}: UploadFooterProps) {
  return (
    <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant bg-surface-container-low/70 px-6 py-4">
      <div className="flex items-center gap-2 text-[12px] text-muted" aria-live="polite">
        {isProcessing ? (
          <>
            <p>{`Processing ${activeCount} ${activeCount === 1 ? "file" : "files"}…`}</p>
            <span className="rounded-full bg-surface-container-high px-2.5 py-1 font-semibold tabular-nums text-text-main">
              {formatElapsedSeconds(elapsedSeconds)}
            </span>
          </>
        ) : processedSeconds !== null ? (
          <p className="font-medium text-text-main">
            Processed in {formatElapsedSeconds(processedSeconds)}
          </p>
        ) : (
          <p>Files are processed independently and update as jobs finish.</p>
        )}
      </div>
      <div className="flex items-center gap-3">
        <button
          className="rounded-full px-5 py-3 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:opacity-50"
          disabled={activeCount > 0}
          onClick={onClose}
          type="button"
        >
          {hasCompletedEntries ? "Done" : "Cancel"}
        </button>
        <button
          className="inline-flex min-w-[150px] items-center justify-center gap-2 rounded-full bg-primary-container px-6 py-3 text-[13px] font-semibold text-on-primary-container shadow-[0_10px_30px_rgba(7,1,84,0.18)] transition hover:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-not-allowed disabled:opacity-50"
          disabled={uploadableCount === 0}
          onClick={onStart}
          type="button"
        >
          <Icon className="h-4 w-4" name="upload" />
          <span>
            {retrying ? "Retry" : "Upload"} {uploadableCount > 0 ? uploadableCount : ""}
          </span>
        </button>
      </div>
    </footer>
  );
}

function getStatusLabel(entry: UploadEntry) {
  switch (entry.status) {
    case "processing":
      return "Processing";
    case "completed":
      return "Complete";
    case "skipped":
      return "Already processed";
    case "failed":
      return entry.path ? "Failed" : "Re-select this file to retry";
    default:
      return "Ready to upload";
  }
}

function getFileExtension(filename: string) {
  return filename.split(".").pop()?.toLowerCase() ?? "";
}

function formatFileSize(sizeBytes: number) {
  if (!sizeBytes) return "Previously selected";
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${(sizeBytes / 1024).toFixed(1)} KB`;
  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function formatElapsedSeconds(seconds: number) {
  return `${seconds} ${seconds === 1 ? "second" : "seconds"}`;
}
