import type {
  NotebookApi,
  NotebookSaveHandlers,
  NotebookSaveJob,
} from "../contracts/notebooks";

export type NotebookTransport = {
  request<T>(
    method: "GET" | "POST" | "PATCH" | "DELETE",
    path: string,
    body?: unknown,
  ): Promise<T>;
  subscribe(
    notebookId: string,
    jobId: string,
    handlers: NotebookSaveHandlers,
  ): () => void;
};
const id = encodeURIComponent;
const root = (notebookId: string) => `/api/v1/notebooks/${id(notebookId)}`;
const note = (notebookId: string, noteId: string) =>
  `${root(notebookId)}/notes/${id(noteId)}`;
const folder = (notebookId: string, folderId: string) =>
  `${root(notebookId)}/folders/${id(folderId)}`;
export const notebookJobPath = (notebookId: string, jobId: string) =>
  `${root(notebookId)}/save-jobs/${id(jobId)}`;
function query(options: object = {}) {
  const parameters = new URLSearchParams();
  for (const [key, value] of Object.entries(options))
    if (value !== undefined) parameters.set(key, String(value));
  const text = parameters.toString();
  return text ? `?${text}` : "";
}
/** Shared URL and payload mapping; each distribution supplies only its authenticated transport. */
export function createNotebookApi(transport: NotebookTransport): NotebookApi {
  const send = transport.request.bind(transport);
  return {
    createNotebook: (input) => send("POST", "/api/v1/notebooks", input),
    listNotebooks: (options) =>
      send("GET", `/api/v1/notebooks${query(options)}`),
    getNotebook: (n) => send("GET", root(n)),
    updateNotebook: (n, input) => send("PATCH", root(n), input),
    deleteNotebook: (n) => send("DELETE", root(n)),
    createNotebookFolder: (n, input) =>
      send("POST", `${root(n)}/folders`, input),
    listNotebookFolders: (n, options) =>
      send("GET", `${root(n)}/folders${query(options)}`),
    getNotebookFolder: (n, f) => send("GET", folder(n, f)),
    updateNotebookFolder: (n, f, input) => send("PATCH", folder(n, f), input),
    deleteNotebookFolder: (n, f) => send("DELETE", folder(n, f)),
    createNotebookNote: (n, input) => send("POST", `${root(n)}/notes`, input),
    listNotebookNotes: (n, options) =>
      send("GET", `${root(n)}/notes${query(options)}`),
    getNotebookNote: (n, v) => send("GET", note(n, v)),
    updateNotebookNote: (n, v, input) => send("PATCH", note(n, v), input),
    deleteNotebookNote: (n, v) => send("DELETE", note(n, v)),
    getNotebookSaveJob: (n, j) => send("GET", notebookJobPath(n, j)),
    retryNotebookSaveJob: (n, j) =>
      send("POST", `${notebookJobPath(n, j)}/retry`, {}),
    subscribeToNotebookSaveJob: transport.subscribe,
  };
}
export function parseNotebookSaveJob(data: string): NotebookSaveJob {
  const value: unknown = JSON.parse(data);
  if (typeof value !== "object" || !value)
    throw new Error("Invalid notebook save event.");
  const record = value as Record<string, unknown>;
  for (const key of [
    "jobId",
    "notebookId",
    "noteId",
    "statusUrl",
    "eventsUrl",
  ]) {
    if (typeof record[key] !== "string")
      throw new Error("Invalid notebook save event.");
  }
  for (const key of ["attempt", "sequence"]) {
    if (!Number.isSafeInteger(record[key]) || Number(record[key]) < 1)
      throw new Error("Invalid notebook save sequence.");
  }
  if (
    !(["queued", "processing", "completed", "failed"] as unknown[]).includes(
      record.status,
    ) ||
    !(
      [
        "persisting",
        "exporting",
        "chunking",
        "embedding",
        "indexing",
      ] as unknown[]
    ).includes(record.stage) ||
    typeof record.sqliteCommitted !== "boolean" ||
    typeof record.retryable !== "boolean" ||
    (record.committedRevision !== null &&
      (!Number.isSafeInteger(record.committedRevision) ||
        Number(record.committedRevision) < 1)) ||
    (record.errorCode !== null && typeof record.errorCode !== "string")
  )
    throw new Error("Invalid notebook save event.");
  if (
    record.status === "completed" &&
    (record.sqliteCommitted !== true ||
      record.committedRevision === null ||
      record.errorCode !== null)
  )
    throw new Error("Incomplete notebook save acknowledgement.");
  if (record.sqliteCommitted !== (record.committedRevision !== null))
    throw new Error("Invalid notebook commit state.");
  return record as NotebookSaveJob;
}
/** Persistent status sequence de-duplicates snapshot delivery across reconnects/retries. */
export function notebookEventReceiver(
  n: string,
  j: string,
  handlers: NotebookSaveHandlers,
) {
  let sequence = 0;
  return (data: string) => {
    const event = parseNotebookSaveJob(data);
    if (event.notebookId !== n || event.jobId !== j)
      throw new Error("Notebook event scope mismatch.");
    if (event.sequence > sequence) {
      sequence = event.sequence;
      handlers.onEvent(event);
    }
    return event.status === "completed" || event.status === "failed";
  };
}
