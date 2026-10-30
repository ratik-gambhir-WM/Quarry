import type {
  NotebookApi,
  Notebook,
  NotebookSaveJob,
  PersistedNote,
} from "@/contracts/notebooks";
import { sessionStore } from "@/auth/sessionStore";
import {
  getNotebookSession,
  type NotebookDraft,
  type NotebookSession,
} from "./notebookSession";

type PersistenceApi = Pick<
  NotebookApi,
  | "listNotebooks"
  | "listNotebookNotes"
  | "getNotebookNote"
  | "createNotebook"
  | "createNotebookNote"
  | "updateNotebookNote"
  | "deleteNotebookNote"
  | "getNotebookSaveJob"
  | "retryNotebookSaveJob"
  | "subscribeToNotebookSaveJob"
>;
type Content = Pick<NotebookDraft, "title" | "blocks" | "contentSchemaVersion">;
type Attempt = {
  operationId: string;
  content: Content;
  expectedRevision?: number;
  job?: NotebookSaveJob;
};
export type NotePersistence = {
  notebookId: string;
  noteId?: string;
  revision?: number;
  hydrated: boolean;
  reloadKey?: number;
  savedContent?: string;
  status: "idle" | "saving" | "saved" | "error" | "conflict";
  message?: string;
  attempt?: Attempt;
};
type Snapshot = {
  notebooks: Notebook[];
  activeNotebookId?: string | null;
  catalog: "idle" | "loading" | "ready" | "error";
  catalogError?: string;
  records: Readonly<Record<string, NotePersistence>>;
};
export const hasUnconfirmedSave = (saved?: NotePersistence) =>
  Boolean(
    saved?.attempt &&
    (!saved.attempt.job ||
      saved.attempt.job.status === "queued" ||
      saved.attempt.job.status === "processing"),
  );
export const noteContent = (note: Content) =>
  JSON.stringify({
    title: note.title.trim(),
    blocks: note.blocks,
    contentSchemaVersion: note.contentSchemaVersion,
  });
const message = (error: unknown) =>
  error instanceof Error
    ? error.message
    : "Could not connect. Please try again.";
const status = (error: unknown) =>
  typeof error === "object" && error && "status" in error
    ? error.status
    : undefined;

/** Session-scoped edit buffers and durable save tracking survive route changes, never sign-out. */
export function createNotebookPersistence(
  session: NotebookSession,
  api: PersistenceApi,
) {
  let state: Snapshot = { notebooks: [], catalog: "idle", records: {} };
  let active = true;
  let loading: Promise<void> | undefined;
  let destination: string | undefined;
  const listeners = new Set<() => void>();
  const monitors = new Map<string, () => void>();
  const hydrating = new Set<string>();
  const generation = sessionStore.getSnapshot().generation;
  const publish = (patch: Partial<Snapshot>) => {
    if (!active) return;
    state = { ...state, ...patch };
    listeners.forEach((listener) => listener());
  };
  const record = (id: string, value: NotePersistence) =>
    publish({ records: { ...state.records, [id]: value } });
  const stopMonitor = (id: string) => {
    monitors.get(id)?.();
    monitors.delete(id);
  };
  const alive = (id: string) =>
    active && session.getSnapshot().some((note) => note.id === id);
  const replaceRemote = (
    id: string,
    remote: PersistedNote,
    discard: boolean,
  ) => {
    const previous = state.records[id];
    session.replace(id, {
      title: remote.title,
      blocks: remote.blocks,
      contentSchemaVersion: remote.contentSchemaVersion,
      createdAt: remote.createdAt,
      updatedAt: remote.updatedAt,
    });
    record(id, {
      notebookId: remote.notebookId,
      noteId: remote.noteId,
      revision: remote.revision,
      hydrated: true,
      reloadKey: (previous?.reloadKey ?? 0) + (discard ? 1 : 0),
      savedContent: noteContent(remote),
      status: "idle",
    });
  };
  const fail = (id: string, error: unknown) => {
    const previous = state.records[id];
    if (alive(id) && previous)
      record(id, {
        ...previous,
        status: status(error) === 409 ? "conflict" : "error",
        message:
          status(error) === 409
            ? "This note changed on the server. Reload the saved note before saving again."
            : message(error),
      });
  };

  function observe(id: string, job: NotebookSaveJob) {
    const previous = state.records[id];
    if (!alive(id) || !previous?.attempt) return;
    if (previous.attempt.job && job.sequence < previous.attempt.job.sequence)
      return;
    const attempt = { ...previous.attempt, job };
    const revision = job.committedRevision ?? previous.revision;
    if (job.status === "completed") {
      stopMonitor(id);
      record(id, {
        ...previous,
        noteId: job.noteId,
        revision,
        status: "saved",
        message: undefined,
        savedContent: noteContent(attempt.content),
        attempt: undefined,
      });
    } else if (job.status === "failed") {
      stopMonitor(id);
      const conflict = job.errorCode === "revision_conflict";
      record(id, {
        ...previous,
        noteId: job.sqliteCommitted ? job.noteId : previous.noteId,
        revision,
        status: conflict ? "conflict" : "error",
        attempt,
        message: conflict
          ? "This note changed on the server. Reload the saved note before saving again."
          : job.sqliteCommitted
            ? "Note stored; indexing failed. Save again to retry indexing."
            : "The note could not be saved. Please try again.",
      });
    } else {
      record(id, {
        ...previous,
        noteId: job.noteId,
        revision,
        status: "saving",
        message: undefined,
        attempt,
      });
    }
  }

  function monitor(id: string, job: NotebookSaveJob) {
    stopMonitor(id);
    observe(id, job);
    if (job.status === "completed" || job.status === "failed" || !alive(id))
      return;
    let stopped = false;
    let unsubscribe: (() => void) | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let failures = 0;
    const stop = () => {
      stopped = true;
      unsubscribe?.();
      if (timer) clearTimeout(timer);
    };
    monitors.set(id, stop);
    // Status polling also covers a missed terminal event or a broken SSE connection.
    const poll = async () => {
      if (stopped || !alive(id)) return;
      try {
        const current = await api.getNotebookSaveJob(job.notebookId, job.jobId);
        if (stopped || !alive(id)) return;
        failures = 0;
        observe(id, current);
      } catch (error) {
        if (stopped || !alive(id)) return;
        failures += 1;
        if (failures >= 3 || status(error) === 401 || status(error) === 404) {
          stopMonitor(id);
          fail(id, error);
          return;
        }
      }
      if (!stopped)
        timer = setTimeout(() => {
          void poll();
        }, 2000);
    };
    try {
      unsubscribe = api.subscribeToNotebookSaveJob(job.notebookId, job.jobId, {
        onEvent: (current) => {
          if (!stopped) observe(id, current);
        },
        onConnectionError: () => {
          /* The durable status poll reconciles this connection. */
        },
      });
      if (stopped) unsubscribe();
    } catch {
      /* Status polling remains available when SSE cannot start. */
    }
    if (!stopped)
      timer = setTimeout(() => {
        void poll();
      }, 1000);
  }

  const store = {
    getSnapshot: () => state,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    selectNotebook(notebookId: string | null) {
      if (!active) return;
      if (notebookId && !state.notebooks.some((book) => book.notebookId === notebookId))
        return;
      publish({ activeNotebookId: notebookId });
    },
    createDraft(notebookId: string | null) {
      const note = session.create();
      if (notebookId) {
        record(note.id, { notebookId, hydrated: true, status: "idle" });
      }
      return note;
    },
    async load() {
      if (!active) return;
      if (loading) return loading;
      publish({ catalog: "loading", catalogError: undefined });
      loading = (async () => {
        try {
          const notebooks: Notebook[] = [];
          for (let offset = 0; ; offset += 100) {
            const page = await api.listNotebooks({ limit: 100, offset });
            if (!active) return;
            notebooks.push(...page.notebooks);
            if (!page.hasMore) break;
            if (offset >= 100000)
              throw new Error("Too many notebooks to load.");
          }
          publish({ notebooks });
          for (const notebook of notebooks) {
            for (let offset = 0; ; offset += 100) {
              const page = await api.listNotebookNotes(notebook.notebookId, {
                limit: 100,
                offset,
              });
              if (!active) return;
              const known = new Set(
                Object.values(state.records).map((saved) => saved.noteId),
              );
              const records: Record<string, NotePersistence> = {};
              const additions: NotebookDraft[] = [];
              for (const summary of page.notes) {
                if (known.has(summary.noteId)) continue;
                records[summary.noteId] = {
                  notebookId: notebook.notebookId,
                  noteId: summary.noteId,
                  revision: summary.revision,
                  hydrated: false,
                  status: "idle",
                };
                additions.push({
                  id: summary.noteId,
                  title: summary.title,
                  blocks: [],
                  contentSchemaVersion: 1,
                  createdAt: summary.createdAt,
                  updatedAt: summary.updatedAt,
                });
              }
              if (additions.length) {
                publish({ records: { ...state.records, ...records } });
                session.importMany(additions);
              }
              if (!page.hasMore) break;
              if (offset >= 100000) throw new Error("Too many notes to load.");
            }
          }
          publish({ catalog: "ready" });
        } catch (error) {
          publish({
            catalog: "error",
            catalogError: `Could not load saved notes. ${message(error)}`,
          });
        }
      })();
      const request = loading;
      void request.finally(() => {
        loading = undefined;
      });
      return request;
    },
    async hydrate(id: string, discard = false) {
      const saved = state.records[id];
      if (!saved?.noteId || (!discard && saved.hydrated) || hydrating.has(id))
        return;
      hydrating.add(id);
      record(id, {
        ...saved,
        hydrated: discard ? false : saved.hydrated,
        status: "idle",
        message: undefined,
      });
      try {
        const remote = await api.getNotebookNote(
          saved.notebookId,
          saved.noteId,
        );
        if (alive(id)) replaceRemote(id, remote, discard);
      } catch (error) {
        fail(id, error);
      } finally {
        hydrating.delete(id);
      }
    },
    async save(
      id: string,
      target?: { notebookId: string } | { title: string },
    ) {
      const note = session.getSnapshot().find((note) => note.id === id);
      const previous = state.records[id];
      if (
        !active ||
        !note ||
        previous?.status === "saving" ||
        previous?.status === "conflict"
      )
        return;
      if (previous && !previous.hydrated) return;
      // Keep uncertain admissions immutable: retry with the same operation ID and original input.
      const prior = previous?.attempt;
      if (prior?.job && !prior.job.retryable && prior.job.status !== "failed") {
        record(id, { ...previous, status: "saving", message: undefined });
        try {
          monitor(
            id,
            await api.getNotebookSaveJob(prior.job.notebookId, prior.job.jobId),
          );
        } catch (error) {
          fail(id, error);
        }
        return;
      }
      const retryJob =
        prior?.job?.status === "failed" && prior.job.retryable
          ? prior.job
          : undefined;
      const retryAdmission = prior && !prior.job ? prior : undefined;
      const content: Content = structuredClone({
        title: note.title.trim(),
        blocks: note.blocks,
        contentSchemaVersion: note.contentSchemaVersion,
      });
      const attempt: Attempt = retryAdmission ??
        (retryJob && prior ? prior : undefined) ?? {
          operationId: crypto.randomUUID(),
          content,
          expectedRevision: previous?.revision,
        };
      const notebookId =
        previous?.notebookId ||
        (target
          ? "notebookId" in target ? target.notebookId : undefined
          : destination);
      record(id, {
        notebookId: notebookId ?? "",
        noteId: previous?.noteId,
        revision: previous?.revision,
        hydrated: true,
        reloadKey: previous?.reloadKey,
        savedContent: previous?.savedContent,
        status: "saving",
        attempt,
      });
      try {
        let parent = notebookId;
        if (!parent && target && "title" in target) {
          const notebook = await api.createNotebook({
            title: target.title.trim(),
          });
          if (!active) return;
          parent = notebook.notebookId;
          publish({ notebooks: [...state.notebooks, notebook] });
        }
        if (!parent) throw new Error("Choose a notebook before saving.");
        destination = parent;
        record(id, { ...state.records[id], notebookId: parent });
        const job = retryJob
          ? await api.retryNotebookSaveJob(parent, retryJob.jobId)
          : previous?.noteId && attempt.expectedRevision !== undefined
            ? await api.updateNotebookNote(parent, previous.noteId, {
                ...attempt.content,
                operationId: attempt.operationId,
                expectedRevision: attempt.expectedRevision,
              })
            : await api.createNotebookNote(parent, {
                ...attempt.content,
                operationId: attempt.operationId,
              });
        if (alive(id)) monitor(id, job);
      } catch (error) {
        if (alive(id)) {
          const current = state.records[id];
          const definitive = [400, 404, 409, 413, 415, 503].includes(
            Number(status(error)),
          );
          if (current && definitive && !retryJob)
            record(id, { ...current, attempt: undefined });
          if (retryJob || (!previous?.revision && status(error) === 409)) {
            if (current)
              record(id, {
                ...state.records[id],
                status: "error",
                message: message(error),
              });
          } else fail(id, error);
        }
      }
    },
    async remove(id: string) {
      const previous = state.records[id];
      if (previous?.status === "saving" || hasUnconfirmedSave(previous))
        throw new Error(
          "Wait for this save to finish before deleting the note.",
        );
      if (previous?.noteId && previous.revision !== undefined)
        await api.deleteNotebookNote(previous.notebookId, previous.noteId);
      if (active) {
        stopMonitor(id);
        session.remove(id);
      }
    },
    dispose() {
      active = false;
      for (const stop of monitors.values()) stop();
      monitors.clear();
      hydrating.clear();
      destination = undefined;
      state = { notebooks: [], catalog: "idle", records: {} };
      listeners.forEach((listener) => listener());
      listeners.clear();
      unwatch();
    },
  };
  const unwatch = sessionStore.subscribe(() => {
    if (sessionStore.getSnapshot().generation !== generation) store.dispose();
  });
  return store;
}

let current:
  | {
      session: NotebookSession;
      store: ReturnType<typeof createNotebookPersistence>;
    }
  | undefined;
export function getNotebookPersistence(api: NotebookApi) {
  const session = getNotebookSession();
  if (current?.session !== session)
    current = { session, store: createNotebookPersistence(session, api) };
  return current.store;
}
