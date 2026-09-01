import { ChangeEvent, DragEvent, useEffect, useRef, useState } from "react";
import { runtime } from "@quarry/runtime";
import type { ProcessFileJobEvent } from "../../contracts/quarryApi";
import { formatFileSize } from "../../lib/formatters";
import { DialogBackdrop } from "../ui/DialogBackdrop";
import { DialogHeader } from "../ui/DialogHeader";
import { Icon } from "../ui/Icon";

type UploadFilesModalProps = {
  dealId: string;
  onClose: () => void;
  userId: string;
};

type UploadStatus = "ready" | "uploading" | "processing" | "completed" | "skipped" | "failed";

type UploadEntry = {
  chunkCount?: number;
  documentId?: string;
  error?: string;
  file: File;
  id: string;
  jobId?: string;
  selected: boolean;
  status: UploadStatus;
};

const maxFileBytes = 50 * 1024 * 1024;
const supportedExtensions = new Set(["pdf", "docx"]);

export function UploadFilesModal({ dealId, onClose, userId }: UploadFilesModalProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const chooseFilesButtonRef = useRef<HTMLButtonElement>(null);
  const processingStartedAtRef = useRef<number | null>(null);
  const subscriptionsRef = useRef(new Map<string, () => void>());
  const [elapsedSeconds, setElapsedSeconds] = useState(0);
  const [entries, setEntries] = useState<UploadEntry[]>([]);
  const [processedSeconds, setProcessedSeconds] = useState<number | null>(null);
  const [selectionError, setSelectionError] = useState("");
  const activeCount = entries.filter((entry) => isActive(entry.status)).length;
  const isProcessing = activeCount > 0;
  const uploadableEntries = entries.filter(
    (entry) => entry.selected && (entry.status === "ready" || entry.status === "failed"),
  );
  const selectedCount = entries.filter((entry) => entry.selected).length;

  useEffect(() => {
    chooseFilesButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && activeCount === 0) {
        onClose();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [activeCount, onClose]);

  useEffect(() => {
    const subscriptions = subscriptionsRef.current;
    return () => {
      for (const unsubscribe of subscriptions.values()) {
        unsubscribe();
      }
      subscriptions.clear();
    };
  }, []);

  useEffect(() => {
    const startedAt = processingStartedAtRef.current;
    if (startedAt === null) {
      return;
    }

    if (!isProcessing) {
      const totalSeconds = Math.max(1, Math.ceil((Date.now() - startedAt) / 1000));
      setElapsedSeconds(totalSeconds);
      setProcessedSeconds(totalSeconds);
      processingStartedAtRef.current = null;
      return;
    }

    const updateElapsedSeconds = () => {
      setElapsedSeconds(Math.floor((Date.now() - startedAt) / 1000));
    };
    updateElapsedSeconds();
    const intervalId = window.setInterval(updateElapsedSeconds, 1000);
    return () => window.clearInterval(intervalId);
  }, [isProcessing]);

  function addFiles(fileList: FileList | File[]) {
    const files = Array.from(fileList);
    const accepted: File[] = [];
    const rejected: string[] = [];

    for (const file of files) {
      if (!supportedExtensions.has(getFileExtension(file.name))) {
        rejected.push(`${file.name} is not a PDF or DOCX file.`);
      } else if (file.size === 0) {
        rejected.push(`${file.name} is empty.`);
      } else if (file.size > maxFileBytes) {
        rejected.push(`${file.name} is larger than 50 MB.`);
      } else {
        accepted.push(file);
      }
    }

    setSelectionError(rejected.join(" "));
    setEntries((current) => {
      const existingIds = new Set(current.map((entry) => entry.id));
      const additions = accepted.flatMap((file) => {
        const id = fileIdentity(file);
        if (existingIds.has(id)) {
          return [];
        }
        existingIds.add(id);
        return [{ file, id, selected: true, status: "ready" as const }];
      });
      return [...current, ...additions];
    });
  }

  function handleFileSelection(event: ChangeEvent<HTMLInputElement>) {
    if (event.target.files) {
      addFiles(event.target.files);
    }
    event.target.value = "";
  }

  function handleDrop(event: DragEvent<HTMLDivElement>) {
    event.preventDefault();
    addFiles(event.dataTransfer.files);
  }

  function toggleEntry(entryId: string) {
    setEntries((current) =>
      current.map((entry) =>
        entry.id === entryId && !isActive(entry.status) && entry.status !== "completed"
          ? { ...entry, selected: !entry.selected }
          : entry,
      ),
    );
  }

  function removeEntry(entryId: string) {
    setEntries((current) =>
      current.filter((entry) => entry.id !== entryId || isActive(entry.status)),
    );
  }

  function handleUpload() {
    if (!dealId.trim()) {
      setSelectionError("Open a deal before uploading files.");
      return;
    }
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
          ? { ...entry, chunkCount: undefined, documentId: undefined, error: undefined, status: "uploading" }
          : entry,
      ),
    );

    for (const entry of uploadableEntries) {
      void startEntryUpload(entry);
    }
  }

  async function startEntryUpload(entry: UploadEntry) {
    try {
      const job = await runtime.api.startProcessFile(dealId, userId, entry.file);
      setEntries((current) =>
        current.map((currentEntry) =>
          currentEntry.id === entry.id
            ? { ...currentEntry, jobId: job.jobId, status: "processing" }
            : currentEntry,
        ),
      );

      const unsubscribe = runtime.api.subscribeToProcessFileJob(job.jobId, {
        onConnectionError: () => {
          setEntries((current) =>
            current.map((currentEntry) =>
              currentEntry.id === entry.id && currentEntry.status === "processing"
                ? { ...currentEntry, error: "Connection interrupted. Reconnecting…" }
                : currentEntry,
            ),
          );
        },
        onEvent: (event) => handleJobEvent(entry.id, event),
      });
      subscriptionsRef.current.set(entry.id, unsubscribe);
    } catch (error) {
      setEntries((current) =>
        current.map((currentEntry) =>
          currentEntry.id === entry.id
            ? {
                ...currentEntry,
                error: error instanceof Error ? error.message : String(error),
                status: "failed",
              }
            : currentEntry,
        ),
      );
    }
  }

  function handleJobEvent(entryId: string, event: ProcessFileJobEvent) {
    setEntries((current) =>
      current.map((entry) => {
        if (entry.id !== entryId) {
          return entry;
        }
        if (event.status === "completed") {
          return {
            ...entry,
            chunkCount: event.chunkCount,
            documentId: event.documentId,
            error: undefined,
            status: "completed",
          };
        }
        if (event.status === "skipped") {
          return {
            ...entry,
            documentId: event.documentId,
            error: undefined,
            status: "skipped",
          };
        }
        if (event.status === "failed") {
          return {
            ...entry,
            error: event.error || "Document processing failed.",
            status: "failed",
          };
        }
        return { ...entry, error: undefined, status: "processing" };
      }),
    );

    if (event.status === "completed" || event.status === "skipped" || event.status === "failed") {
      subscriptionsRef.current.get(entryId)?.();
      subscriptionsRef.current.delete(entryId);
    }
  }

  return (
    <DialogBackdrop
      closeLabel="Close upload files dialog"
      disabled={activeCount > 0}
      onClose={onClose}
    >
      <section
        aria-labelledby="upload-files-title"
        aria-modal="true"
        className="relative z-10 flex max-h-[calc(100vh-3rem)] w-full max-w-[680px] flex-col overflow-hidden rounded-[20px] border border-outline-variant bg-surface-container-lowest shadow-[0_28px_70px_rgba(7,1,84,0.24)]"
        role="dialog"
      >
        <DialogHeader
          className="border-b border-outline-variant px-6 py-5"
          closeLabel="Close upload files dialog"
          description="Choose PDF and DOCX files from your Mac, then select which ones to process."
          disabled={activeCount > 0}
          eyebrow="Data Room"
          onClose={onClose}
          title="Upload files"
          titleId="upload-files-title"
        />

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          <input
            accept=".pdf,.docx,application/pdf,application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            className="hidden"
            multiple
            onChange={handleFileSelection}
            ref={fileInputRef}
            type="file"
          />

          <div
            className="flex min-h-28 flex-col items-center justify-center rounded-2xl border border-dashed border-primary/40 bg-surface-container-low px-6 py-5 text-center transition hover:border-primary/70 hover:bg-surface-container-high"
            onDragOver={(event) => event.preventDefault()}
            onDrop={handleDrop}
          >
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 text-primary">
              <Icon className="h-5 w-5" name="upload" />
            </div>
            <p className="mt-3 text-[13px] font-semibold text-text-main">Drop PDF or DOCX files here</p>
            <button
              className="mt-2 text-[12px] font-semibold text-primary underline decoration-primary/30 underline-offset-4 transition hover:decoration-primary"
              onClick={() => fileInputRef.current?.click()}
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

        <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant bg-surface-container-low/70 px-6 py-4">
          <div className="flex items-center gap-2 text-[12px] text-muted">
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
              {entries.some((entry) => entry.status === "completed" || entry.status === "skipped") ? "Done" : "Cancel"}
            </button>
            <button
              className="inline-flex min-w-[150px] items-center justify-center gap-2 rounded-full bg-primary-container px-6 py-3 text-[13px] font-semibold text-on-primary-container shadow-[0_10px_30px_rgba(7,1,84,0.18)] transition hover:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-not-allowed disabled:opacity-50"
              disabled={uploadableEntries.length === 0}
              onClick={handleUpload}
              type="button"
            >
              <Icon className="h-4 w-4" name="upload" />
              <span>
                {uploadableEntries.some((entry) => entry.status === "failed") ? "Retry" : "Upload"}{" "}
                {uploadableEntries.length > 0 ? uploadableEntries.length : ""}
              </span>
            </button>
          </div>
        </footer>
      </section>
    </DialogBackdrop>
  );
}

type UploadFileRowProps = {
  entry: UploadEntry;
  onRemove: (entryId: string) => void;
  onToggle: (entryId: string) => void;
};

function UploadFileRow({ entry, onRemove, onToggle }: UploadFileRowProps) {
  const active = isActive(entry.status);
  const completed = entry.status === "completed";
  const skipped = entry.status === "skipped";
  const failed = entry.status === "failed";

  return (
    <li className="flex items-start gap-3 bg-surface-container-lowest px-4 py-3.5">
      <input
        aria-label={`Select ${entry.file.name}`}
        checked={entry.selected}
        className="mt-1 h-4 w-4 shrink-0 accent-[#0c006b]"
        disabled={active || completed || skipped}
        onChange={() => onToggle(entry.id)}
        type="checkbox"
      />
      <div className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-surface-container text-primary">
        <Icon className="h-5 w-5" name={getFileExtension(entry.file.name) === "pdf" ? "pdf" : "doc"} />
      </div>
      <div className="min-w-0 flex-1">
        <p className="truncate text-[13px] font-semibold text-text-main" title={entry.file.name}>
          {entry.file.name}
        </p>
        <div className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-[11px] text-muted">
          <span>{formatFileSize(entry.file.size)}</span>
          <span aria-hidden="true">·</span>
          <span className={failed ? "font-semibold text-error" : completed || skipped ? "font-semibold text-[#168447]" : ""}>
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
            aria-label={entry.status === "uploading" ? "Uploading" : "Processing"}
            className="h-5 w-5 rounded-full border-2 border-primary/25 border-t-primary motion-safe:animate-spin"
          />
        ) : (
          <button
            aria-label={`Remove ${entry.file.name}`}
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

function getStatusLabel(entry: UploadEntry) {
  switch (entry.status) {
    case "uploading":
      return "Reading and uploading bytes";
    case "processing":
      return "Processing";
    case "completed":
      return "Complete";
    case "skipped":
      return "Already processed";
    case "failed":
      return "Failed";
    default:
      return "Ready to upload";
  }
}

function isActive(status: UploadStatus) {
  return status === "uploading" || status === "processing";
}

function getFileExtension(filename: string) {
  return filename.split(".").pop()?.toLowerCase() ?? "";
}

function fileIdentity(file: File) {
  return `${file.name}:${file.size}:${file.lastModified}`;
}

function formatElapsedSeconds(seconds: number) {
  return `${seconds} ${seconds === 1 ? "second" : "seconds"}`;
}
