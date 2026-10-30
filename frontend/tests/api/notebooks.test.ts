// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { BlockNoteEditor } from "@blocknote/core";
import {
  notebookSchema,
  type NotebookBlock,
} from "@/components/notebook/notebookSchema";
import native from "../../../backend/tests/unit/domains/notebooks/fixtures/native-v1.json";
import { httpQuarryApi, BackendApiError } from "@/api/httpQuarryApi";
import { createTauriQuarryApi } from "@/api/tauriQuarryApi";
import { notebookEventReceiver } from "@/api/notebookApi";
import { NotebookSseParser } from "@/api/notebookSse";
import type { NotebookSaveJob, QuarryApi } from "@/contracts/quarryApi";

const n = "00000000-0000-4000-8000-000000000001";
const j = "00000000-0000-4000-8000-000000000002";
const event = (
  status: NotebookSaveJob["status"] = "completed",
  sequence = 8,
): NotebookSaveJob => ({
  jobId: j,
  notebookId: n,
  noteId: j,
  status,
  stage: "indexing",
  attempt: 1,
  sequence,
  committedRevision: 1,
  sqliteCommitted: true,
  errorCode: null,
  retryable: false,
  statusUrl: `/api/v1/notebooks/${n}/save-jobs/${j}`,
  eventsUrl: `/api/v1/notebooks/${n}/save-jobs/${j}/events`,
});
afterEach(() => vi.unstubAllGlobals());

describe("Notebook native serialization contract", () => {
  it("roundtrips the shared Rust fixture through the installed editor", () => {
    const editor = BlockNoteEditor.create({
      schema: notebookSchema,
      initialContent: native as NotebookBlock[],
    });
    expect(JSON.parse(JSON.stringify(editor.document))).toEqual(native);
  });
});
const operations: [
  string,
  (api: QuarryApi) => Promise<unknown>,
  string,
  string,
  unknown?,
][] = [
  [
    "create notebook",
    (a) => a.createNotebook({ title: "Book", dealId: null }),
    "POST",
    "/api/v1/notebooks",
    { title: "Book", dealId: null },
  ],
  [
    "list notebooks",
    (a) => a.listNotebooks({ dealId: "none", limit: 1, offset: 2 }),
    "GET",
    "/api/v1/notebooks?dealId=none&limit=1&offset=2",
  ],
  ["get notebook", (a) => a.getNotebook(n), "GET", `/api/v1/notebooks/${n}`],
  [
    "patch notebook",
    (a) => a.updateNotebook(n, { dealId: null }),
    "PATCH",
    `/api/v1/notebooks/${n}`,
    { dealId: null },
  ],
  [
    "delete notebook",
    (a) => a.deleteNotebook(n),
    "DELETE",
    `/api/v1/notebooks/${n}`,
  ],
  [
    "create folder",
    (a) => a.createNotebookFolder(n, { name: "Folder" }),
    "POST",
    `/api/v1/notebooks/${n}/folders`,
    { name: "Folder" },
  ],
  [
    "list folders",
    (a) => a.listNotebookFolders(n),
    "GET",
    `/api/v1/notebooks/${n}/folders`,
  ],
  [
    "get folder",
    (a) => a.getNotebookFolder(n, j),
    "GET",
    `/api/v1/notebooks/${n}/folders/${j}`,
  ],
  [
    "patch folder",
    (a) => a.updateNotebookFolder(n, j, { name: "Renamed" }),
    "PATCH",
    `/api/v1/notebooks/${n}/folders/${j}`,
    { name: "Renamed" },
  ],
  [
    "delete folder",
    (a) => a.deleteNotebookFolder(n, j),
    "DELETE",
    `/api/v1/notebooks/${n}/folders/${j}`,
  ],
  [
    "create note",
    (a) =>
      a.createNotebookNote(n, {
        operationId: j,
        blocks: [],
        contentSchemaVersion: 1,
      }),
    "POST",
    `/api/v1/notebooks/${n}/notes`,
    { operationId: j, blocks: [], contentSchemaVersion: 1 },
  ],
  [
    "list notes",
    (a) => a.listNotebookNotes(n, { folderId: "none", q: "%_\\" }),
    "GET",
    `/api/v1/notebooks/${n}/notes?folderId=none&q=%25_%5C`,
  ],
  [
    "get note",
    (a) => a.getNotebookNote(n, j),
    "GET",
    `/api/v1/notebooks/${n}/notes/${j}`,
  ],
  [
    "patch note",
    (a) =>
      a.updateNotebookNote(n, j, {
        operationId: j,
        expectedRevision: 2,
        folderId: null,
      }),
    "PATCH",
    `/api/v1/notebooks/${n}/notes/${j}`,
    { operationId: j, expectedRevision: 2, folderId: null },
  ],
  [
    "delete note",
    (a) => a.deleteNotebookNote(n, j),
    "DELETE",
    `/api/v1/notebooks/${n}/notes/${j}`,
  ],
  [
    "job status",
    (a) => a.getNotebookSaveJob(n, j),
    "GET",
    `/api/v1/notebooks/${n}/save-jobs/${j}`,
  ],
  [
    "retry",
    (a) => a.retryNotebookSaveJob(n, j),
    "POST",
    `/api/v1/notebooks/${n}/save-jobs/${j}/retry`,
    {},
  ],
];
describe.each(["web", "desktop"])("Notebook %s transport parity", (target) => {
  it.each(operations)("%s", async (_name, invoke, method, path, body) => {
    const response = method === "DELETE" ? undefined : event();
    const fetch = vi.fn().mockImplementation(() =>
      Promise.resolve(
        new Response(response ? JSON.stringify(response) : null, {
          status: response ? 200 : 204,
        }),
      ),
    );
    vi.stubGlobal("fetch", fetch);
    const get = vi.fn().mockResolvedValue(response),
      post = vi.fn().mockResolvedValue(response),
      patch = vi.fn().mockResolvedValue(response),
      remove = vi.fn().mockResolvedValue(undefined);
    const desktop = createTauriQuarryApi({
      get,
      post,
      patch,
      delete: remove,
      getPdf: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });
    await expect(
      invoke(target === "web" ? httpQuarryApi : desktop),
    ).resolves.toEqual(response);
    if (target === "web") {
      const [url, init] = fetch.mock.calls[0];
      expect(url).toBe(path);
      expect(init.method).toBe(method);
      expect(init.redirect).toBe("error");
      expect(init.body).toBe(
        body === undefined ? undefined : JSON.stringify(body),
      );
    } else {
      const mock = { GET: get, POST: post, PATCH: patch, DELETE: remove }[
        method
      ];
      expect(mock).toHaveBeenCalledWith(
        ...(body === undefined ? [path] : [path, body]),
      );
    }
  });
});
describe("Notebook error and SSE recovery contracts", () => {
  it.each([409, 413, 503])("preserves HTTP %s errors", async (status) => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockImplementation(() =>
          Promise.resolve(new Response('{"error":"safe error"}', { status })),
        ),
    );
    await expect(
      httpQuarryApi.updateNotebook(n, { title: "New" }),
    ).rejects.toMatchObject({ status, message: "safe error" });
    await expect(httpQuarryApi.getNotebook(n)).rejects.toBeInstanceOf(
      BackendApiError,
    );
  });
  it("de-duplicates durable snapshots and rejects a foreign job", () => {
    const onEvent = vi.fn();
    const receive = notebookEventReceiver(n, j, { onEvent });
    receive(JSON.stringify(event("processing", 2)));
    receive(JSON.stringify(event("processing", 2)));
    receive(JSON.stringify(event("completed", 8)));
    expect(onEvent).toHaveBeenCalledTimes(2);
    expect(() =>
      receive(JSON.stringify({ ...event(), jobId: "foreign" })),
    ).toThrow("scope mismatch");
  });
  it("parses split CRLF events, ignores keepalives and bounds buffers", () => {
    const parser = new NotebookSseParser();
    const received: string[] = [];
    const text = `: keepalive\r\n\r\nevent: completed\r\ndata: ${JSON.stringify(event())}\r\n\r\n`;
    for (const c of text) parser.push(c, (data) => received.push(data));
    expect(received).toEqual([JSON.stringify(event())]);
    expect(() => parser.push("x".repeat(65537), () => undefined)).toThrow(
      "size limit",
    );
  });
  it("cancels a desktop subscription even if cleanup arrives after unsubscribe", async () => {
    let data: ((data: string) => void) | undefined;
    let resolve: ((stop: () => void) => void) | undefined;
    const stop = vi.fn(),
      onEvent = vi.fn();
    const api = createTauriQuarryApi({
      get: vi.fn(),
      post: vi.fn(),
      delete: vi.fn(),
      getPdf: vi.fn(),
      postPowerPoint: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
      subscribeNotebookJob: (_n, _j, receive) => {
        data = receive;
        return new Promise((r) => {
          resolve = r;
        });
      },
    });
    const cancel = api.subscribeToNotebookSaveJob(n, j, { onEvent });
    cancel();
    data?.(JSON.stringify(event()));
    resolve?.(stop);
    await Promise.resolve();
    expect(stop).toHaveBeenCalledOnce();
    expect(onEvent).not.toHaveBeenCalled();
  });
});

describe("Notebook private activity diagnostics", () => {
  it("excludes note text, response content and title-search query values", async () => {
    const { clearActivityLog, getActivityLogEntries } =
      await import("@/lib/activityLog");
    clearActivityLog();
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(() =>
        Promise.resolve(
          new Response(JSON.stringify({ title: "private-response" }), {
            status: 200,
          }),
        ),
      ),
    );
    await httpQuarryApi.createNotebookNote(n, {
      operationId: j,
      title: "private-title",
    });
    await httpQuarryApi.listNotebookNotes(n, { q: "private-query" });
    const diagnostics = JSON.stringify(getActivityLogEntries());
    for (const secret of ["private-title", "private-query", "private-response"])
      expect(diagnostics).not.toContain(secret);
  });
});
