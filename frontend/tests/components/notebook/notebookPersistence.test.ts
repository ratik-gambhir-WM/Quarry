import { afterEach, describe, expect, it, vi } from "vitest";
import {
  createNotebookPersistence,
  noteContent,
} from "@/data/notebook/notebookPersistence";
import { createNotebookSession } from "@/data/notebook/notebookSession";
import type {
  NotebookSaveHandlers,
  NotebookSaveJob,
  PersistedNote,
} from "@/contracts/notebooks";

const book = {
  notebookId: "book",
  title: "Research",
  dealId: null,
  createdAt: "2026-10-28T00:00:00Z",
  updatedAt: "2026-10-28T00:00:00Z",
};
function job(overrides: Partial<NotebookSaveJob> = {}): NotebookSaveJob {
  return {
    notebookId: book.notebookId,
    noteId: "server-note",
    jobId: "job",
    status: "queued",
    stage: "persisting",
    attempt: 1,
    sequence: 1,
    committedRevision: null,
    sqliteCommitted: false,
    retryable: false,
    errorCode: null,
    statusUrl: "/job",
    eventsUrl: "/events",
    ...overrides,
  };
}
function remote(overrides: Partial<PersistedNote> = {}): PersistedNote {
  return {
    notebookId: book.notebookId,
    noteId: "server-note",
    title: "Persisted",
    blocks: [],
    folderId: null,
    revision: 1,
    contentSchemaVersion: 1,
    createdAt: book.createdAt,
    updatedAt: book.updatedAt,
    ...overrides,
  };
}
const stores: ReturnType<typeof createNotebookPersistence>[] = [];
afterEach(() => {
  stores.forEach((store) => store.dispose());
  stores.length = 0;
  vi.useRealTimers();
});
function setup() {
  const session = createNotebookSession();
  let handlers: NotebookSaveHandlers | undefined;
  const stop = vi.fn();
  const api = {
    listNotebooks: vi
      .fn()
      .mockResolvedValue({ notebooks: [book], hasMore: false }),
    listNotebookNotes: vi.fn().mockResolvedValue({ notes: [], hasMore: false }),
    getNotebookNote: vi.fn().mockResolvedValue(remote()),
    createNotebook: vi.fn().mockResolvedValue(book),
    createNotebookNote: vi.fn().mockResolvedValue(job()),
    updateNotebookNote: vi.fn().mockResolvedValue(job({ jobId: "patch-job" })),
    getNotebookSaveJob: vi.fn().mockResolvedValue(job()),
    retryNotebookSaveJob: vi
      .fn()
      .mockResolvedValue(job({ attempt: 2, sequence: 9 })),
    deleteNotebookNote: vi.fn().mockResolvedValue(undefined),
    subscribeToNotebookSaveJob: vi.fn(
      (_n: string, _j: string, next: NotebookSaveHandlers) => {
        handlers = next;
        return stop;
      },
    ),
  };
  const store = createNotebookPersistence(session, api);
  stores.push(store);
  return {
    session,
    store,
    api,
    stop,
    emit: (event: NotebookSaveJob) => handlers?.onEvent(event),
  };
}
describe("Notebook persistence", () => {
  it("honors an explicit new notebook after saving to another notebook", async () => {
    const { session, store, api, emit } = setup();
    const first = session.create();
    await store.save(first.id, { notebookId: book.notebookId });
    emit(job({ status: "completed", sqliteCommitted: true, committedRevision: 1, sequence: 8 }));
    api.createNotebook.mockResolvedValue({ ...book, notebookId: "second-book", title: "Finance" });
    const second = session.create();
    await store.save(second.id, { title: "Finance" });
    expect(api.createNotebook).toHaveBeenCalledWith({ title: "Finance" });
    expect(api.createNotebookNote).toHaveBeenLastCalledWith("second-book", expect.any(Object));
  });

  it("keeps notebook selection and draft assignment in memory without writing to the server", async () => {
    const { session, store, api } = setup();
    await store.load();
    store.selectNotebook(book.notebookId);
    const draft = store.createDraft(book.notebookId);
    expect(store.getSnapshot().activeNotebookId).toBe(book.notebookId);
    expect(store.getSnapshot().records[draft.id]).toMatchObject({ notebookId: book.notebookId, hydrated: true });
    store.selectNotebook(null);
    expect(session.getSnapshot().some((note) => note.id === draft.id)).toBe(true);
    expect(api.createNotebookNote).not.toHaveBeenCalled();
    store.dispose();
    expect(store.getSnapshot().activeNotebookId).toBeUndefined();
  });

  it("creates only on explicit save and then patches the same server note with its revision", async () => {
    const { session, store, api, emit } = setup();
    await store.load();
    expect(api.createNotebook).not.toHaveBeenCalled();
    const draft = session.create();
    session.update(draft.id, { title: "First" });
    await store.save(draft.id, { title: "Research" });
    expect(api.createNotebook).toHaveBeenCalledWith({ title: "Research" });
    expect(api.createNotebookNote).toHaveBeenCalledWith(
      "book",
      expect.objectContaining({
        title: "First",
        blocks: [],
        contentSchemaVersion: 1,
      }),
    );
    expect(store.getSnapshot().records[draft.id].status).toBe("saving");
    emit(
      job({
        status: "completed",
        sqliteCommitted: true,
        committedRevision: 1,
        sequence: 8,
      }),
    );
    session.update(draft.id, { title: "Second" });
    await store.save(draft.id);
    expect(api.updateNotebookNote).toHaveBeenCalledWith(
      "book",
      "server-note",
      expect.objectContaining({ expectedRevision: 1, title: "Second" }),
    );
    expect(api.createNotebookNote).toHaveBeenCalledTimes(1);
  });
  it("does not mark newer edits saved or replace the live buffer on completion", async () => {
    const { session, store, emit } = setup();
    const draft = session.create();
    session.update(draft.id, { title: "Submitted" });
    await store.save(draft.id, { notebookId: "book" });
    session.update(draft.id, { title: "Newer edits" });
    emit(
      job({
        status: "completed",
        sqliteCommitted: true,
        committedRevision: 1,
        sequence: 8,
      }),
    );
    const current = session.getSnapshot()[0];
    expect(current.title).toBe("Newer edits");
    expect(store.getSnapshot().records[draft.id].savedContent).not.toBe(
      noteContent(current),
    );
  });
  it("reuses the immutable operation and snapshot after an uncertain admission response", async () => {
    const { session, store, api } = setup();
    const draft = session.create();
    session.update(draft.id, { title: "Original" });
    api.createNotebookNote.mockRejectedValueOnce(new Error("Connection lost"));
    await store.save(draft.id, { notebookId: "book" });
    const original = api.createNotebookNote.mock.calls[0];
    session.update(draft.id, { title: "New changes" });
    await store.save(draft.id);
    expect(api.createNotebookNote.mock.calls[1]).toEqual(original);
    expect(session.getSnapshot()[0].title).toBe("New changes");
  });
  it("retries indexing without repeating the canonical mutation and keeps its committed revision", async () => {
    const { session, store, api, emit } = setup();
    const draft = session.create();
    await store.save(draft.id, { notebookId: "book" });
    emit(
      job({
        status: "failed",
        sqliteCommitted: true,
        committedRevision: 1,
        sequence: 8,
        retryable: true,
        errorCode: "embedding_failed",
      }),
    );
    await store.save(draft.id);
    expect(api.retryNotebookSaveJob).toHaveBeenCalledWith("book", "job");
    expect(api.createNotebookNote).toHaveBeenCalledTimes(1);
    expect(api.updateNotebookNote).not.toHaveBeenCalled();
    expect(store.getSnapshot().records[draft.id].revision).toBe(1);
  });
  it("hydrates saved notes lazily and patches them after a fresh session load", async () => {
    const { session, store, api, emit } = setup();
    api.listNotebookNotes.mockResolvedValue({
      notes: [remote()],
      hasMore: false,
    });
    await store.load();
    expect(api.getNotebookNote).not.toHaveBeenCalled();
    expect(store.getSnapshot().records["server-note"].hydrated).toBe(false);
    await store.hydrate("server-note");
    session.update("server-note", { title: "Loaded and edited" });
    await store.save("server-note");
    expect(api.updateNotebookNote).toHaveBeenCalledWith(
      "book",
      "server-note",
      expect.objectContaining({
        expectedRevision: 1,
        title: "Loaded and edited",
      }),
    );
    emit(
      job({
        jobId: "patch-job",
        status: "completed",
        sqliteCommitted: true,
        committedRevision: 2,
        sequence: 8,
      }),
    );
    expect(store.getSnapshot().records["server-note"].revision).toBe(2);
  });
  it("prevents duplicate admission and ignores events after session disposal", async () => {
    const { session, store, api, emit, stop } = setup();
    const draft = session.create();
    await Promise.all([
      store.save(draft.id, { notebookId: "book" }),
      store.save(draft.id, { notebookId: "book" }),
    ]);
    expect(api.createNotebookNote).toHaveBeenCalledTimes(1);
    store.dispose();
    emit(
      job({
        status: "completed",
        sqliteCommitted: true,
        committedRevision: 1,
        sequence: 8,
      }),
    );
    expect(stop).toHaveBeenCalled();
    expect(store.getSnapshot().records).toEqual({});
  });
  it("reconciles missed SSE through status GET without reapplying a save", async () => {
    vi.useFakeTimers();
    const { session, store, api } = setup();
    const draft = session.create();
    api.getNotebookSaveJob.mockResolvedValue(
      job({
        status: "completed",
        sqliteCommitted: true,
        committedRevision: 1,
        sequence: 8,
      }),
    );
    await store.save(draft.id, { notebookId: "book" });
    await vi.advanceTimersByTimeAsync(1000);
    expect(store.getSnapshot().records[draft.id].status).toBe("saved");
    expect(api.createNotebookNote).toHaveBeenCalledTimes(1);
  });
  it("leaves local edits intact on conflict until an explicit reload", async () => {
    const { session, store, api } = setup();
    api.listNotebookNotes.mockResolvedValue({
      notes: [remote()],
      hasMore: false,
    });
    await store.load();
    await store.hydrate("server-note");
    session.update("server-note", { title: "Local changes" });
    api.updateNotebookNote.mockRejectedValue(
      Object.assign(new Error("Conflict"), { status: 409 }),
    );
    await store.save("server-note");
    expect(store.getSnapshot().records["server-note"].status).toBe("conflict");
    expect(session.getSnapshot()[0].title).toBe("Local changes");
    api.getNotebookNote.mockResolvedValue(
      remote({ title: "Latest server version", revision: 3 }),
    );
    await store.hydrate("server-note", true);
    expect(session.getSnapshot()[0].title).toBe("Latest server version");
    expect(store.getSnapshot().records["server-note"].revision).toBe(3);
  });
  it("keeps a saved note when server deletion fails", async () => {
    const { session, store, api } = setup();
    api.listNotebookNotes.mockResolvedValue({
      notes: [remote()],
      hasMore: false,
    });
    await store.load();
    api.deleteNotebookNote.mockRejectedValueOnce(new Error("Offline"));
    await expect(store.remove("server-note")).rejects.toThrow();
    expect(session.getSnapshot()).toHaveLength(1);
    await store.remove("server-note");
    expect(session.getSnapshot()).toHaveLength(0);
  });
});

describe("Notebook save failure recovery", () => {
  it("accepts corrected content after a definitive validation failure", async () => {
    const { session, store, api } = setup();
    const draft = session.create();
    session.update(draft.id, { title: "Invalid" });
    api.createNotebookNote.mockRejectedValueOnce(
      Object.assign(new Error("Invalid content"), { status: 400 }),
    );
    await store.save(draft.id, { notebookId: "book" });
    session.update(draft.id, { title: "Corrected" });
    await store.save(draft.id);
    expect(api.createNotebookNote.mock.calls[1][1].title).toBe("Corrected");
    expect(api.createNotebookNote.mock.calls[1][1].operationId).not.toBe(
      api.createNotebookNote.mock.calls[0][1].operationId,
    );
  });
  it("prevents discarding a creation whose admission outcome is uncertain", async () => {
    const { session, store, api } = setup();
    const draft = session.create();
    api.createNotebookNote.mockRejectedValueOnce(new Error("Response lost"));
    await store.save(draft.id, { notebookId: "book" });
    await expect(store.remove(draft.id)).rejects.toThrow("finish");
    expect(session.getSnapshot()).toHaveLength(1);
  });
});
