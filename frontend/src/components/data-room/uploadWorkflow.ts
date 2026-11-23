import type { QuarryApi, UploadProgressEvent } from "../../contracts/quarryApi";

export type UploadStatus = "ready" | "queued" | "uploading" | "processing" | "completed" | "skipped" | "failed";
export type UploadEntry = {
  chunkCount?: number;
  documentId?: string;
  error?: string;
  file: File;
  id: string;
  jobId?: string;
  stage?: UploadProgressEvent["status"];
  selected: boolean;
  status: UploadStatus;
};
type Snapshot = {
  entries: UploadEntry[];
  elapsedSeconds: number;
  processedSeconds: number | null;
};
export const isActive = (status: UploadStatus) =>
  status === "queued" || status === "uploading" || status === "processing";
export const canUpload = (entry: UploadEntry) =>
  entry.selected && (entry.status === "ready" || entry.status === "failed");

/** One modal owns a bounded queue, including processing jobs and their subscriptions. */
export function createUploadWorkflow(
  api: Pick<QuarryApi, "startProcessFile">,
  dealId: string,
) {
  // Matches the server's default background document workers; the server still
  // enforces its own bound across every client.
  const concurrency = 5;
  let state: Snapshot = { entries: [], elapsedSeconds: 0, processedSeconds: null };
  let generation = 0;
  let pumping = false;
  let startedAt: number | null = null;
  let timer: ReturnType<typeof setInterval> | undefined;
  const listeners = new Set<() => void>();
  const running = new Map<string, { stop: () => void }>();
  const publish = (patch: Partial<Snapshot>) => {
    state = { ...state, ...patch };
    listeners.forEach((listener) => listener());
  };
  const change = (id: string, patch: Partial<UploadEntry>) => {
    publish({ entries: state.entries.map((entry) => entry.id === id ? { ...entry, ...patch } : entry) });
  };
  const finish = (id: string, patch: Partial<UploadEntry>) => {
    const run = running.get(id);
    running.delete(id);
    run?.stop();
    change(id, patch);
    pump();
    if (!state.entries.some((entry) => isActive(entry.status)) && startedAt !== null) {
      const seconds = Math.max(1, Math.ceil((Date.now() - startedAt) / 1000));
      clearInterval(timer);
      startedAt = null;
      publish({ elapsedSeconds: seconds, processedSeconds: seconds });
    }
  };
  async function start(entry: UploadEntry) {
    const attemptGeneration = generation;
    const run = { stop: () => {} };
    const current = () => generation === attemptGeneration && listeners.size > 0 && running.get(entry.id) === run;
    running.set(entry.id, run);
    change(entry.id, { status: "uploading" });
    if (!current()) return;
    try {
      const job = await api.startProcessFile(dealId, entry.file, event => {
        if (!current()) return;
        change(entry.id, { jobId: event.jobId, status: "processing", stage: event.status, error: undefined });
      });
      if (!current()) return;
      finish(entry.id, { jobId: job.jobId, status: "completed", stage: "accepted", error: undefined });
    } catch (error) {
      if (current()) finish(entry.id, { status: "failed", error: error instanceof Error ? error.message : String(error) });
    }
  }
  function pump() {
    if (pumping || listeners.size === 0) return;
    pumping = true;
    try {
      while (listeners.size > 0 && running.size < concurrency) {
        const next = state.entries.find((entry) => entry.status === "queued");
        if (!next) break;
        void start(next);
      }
    } finally { pumping = false; }
  }

  return {
    getSnapshot: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
        if (listeners.size) return;
        generation++;
        clearInterval(timer);
        running.forEach((run) => run.stop());
        running.clear();
      };
    },
    add(files: File[]) {
      const ids = new Set(state.entries.map((entry) => entry.id));
      const additions = files.flatMap((file) => {
        const id = `${file.name}:${file.size}:${file.lastModified}`;
        if (ids.has(id)) return [];
        ids.add(id);
        return [{ file, id, selected: true, status: "ready" as const }];
      });
      publish({ entries: [...state.entries, ...additions] });
    },
    toggle(id: string) {
      const entry = state.entries.find((entry) => entry.id === id);
      if (entry && !isActive(entry.status) && entry.status !== "completed" && entry.status !== "skipped")
        change(id, { selected: !entry.selected });
    },
    remove(id: string) {
      publish({ entries: state.entries.filter((entry) => entry.id !== id || isActive(entry.status)) });
    },
    upload() {
      if (!state.entries.some(canUpload) || listeners.size === 0) return;
      if (startedAt === null) {
        startedAt = Date.now();
        timer = setInterval(() => {
          if (startedAt !== null) publish({ elapsedSeconds: Math.floor((Date.now() - startedAt) / 1000) });
        }, 1000);
      }
      publish({
        elapsedSeconds: Math.floor((Date.now() - startedAt) / 1000),
        processedSeconds: null,
        entries: state.entries.map((entry) => canUpload(entry) ? {
          ...entry, status: "queued", chunkCount: undefined, documentId: undefined, jobId: undefined, error: undefined,
        } : entry),
      });
      pump();
    },
  };
}
