// @vitest-environment happy-dom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  HashRouter,
  MemoryRouter,
  Route,
  Routes,
  useNavigate,
} from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { sessionStore } from "@/auth/sessionStore";
import { UserProvider } from "@/auth/UserProvider";
import { testSession } from "@/test/fixtures/auth";
import { WorkspaceProvider } from "@/app/WorkspaceProvider";
import type {
  NotebookSaveHandlers,
  NotebookSaveJob,
} from "@/contracts/notebooks";
import { NotebookPage } from "@/pages/NotebookPage";

const saveEvents = vi.hoisted(() => {
  const value: { handlers?: NotebookSaveHandlers } = {};
  return value;
});
const api = vi.hoisted(() => ({
  listDeals: vi.fn(),
  listNotebooks: vi.fn(),
  listNotebookNotes: vi.fn(),
  getNotebookNote: vi.fn(),
  createNotebook: vi.fn(),
  createNotebookNote: vi.fn(),
  updateNotebookNote: vi.fn(),
  deleteNotebookNote: vi.fn(),
  getNotebookSaveJob: vi.fn(),
  retryNotebookSaveJob: vi.fn(),
  subscribeToNotebookSaveJob: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({ runtime: { api } }));
const book = {
  notebookId: "00000000-0000-4000-8000-000000000001",
  title: "Research",
  dealId: null,
  createdAt: "now",
  updatedAt: "now",
};
function saveJob(overrides: Partial<NotebookSaveJob> = {}): NotebookSaveJob {
  return {
    notebookId: book.notebookId,
    noteId: "00000000-0000-4000-8000-000000000002",
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
vi.mock("@/components/notebook/NotebookEditor", () => ({
  NotebookEditor: ({
    blocks,
    onChange,
  }: {
    blocks: unknown[];
    onChange: (blocks: unknown[]) => void;
  }) => (
    <textarea
      aria-label="Test note content"
      defaultValue={JSON.stringify(blocks)}
      onChange={(event) =>
        onChange([{ type: "paragraph", content: event.currentTarget.value }])
      }
    />
  ),
}));

beforeEach(() => {
  vi.clearAllMocks();
  saveEvents.handlers = undefined;
  api.listDeals.mockResolvedValue([]);
  api.listNotebooks.mockResolvedValue({ notebooks: [], hasMore: false });
  api.listNotebookNotes.mockResolvedValue({ notes: [], hasMore: false });
  api.createNotebook.mockResolvedValue(book);
  api.createNotebookNote.mockResolvedValue(saveJob());
  api.updateNotebookNote.mockResolvedValue(saveJob({ jobId: "patch-job" }));
  api.getNotebookSaveJob.mockResolvedValue(saveJob());
  api.deleteNotebookNote.mockResolvedValue(undefined);
  api.subscribeToNotebookSaveJob.mockImplementation(
    (_n: string, _j: string, handlers: NotebookSaveHandlers) => {
      saveEvents.handlers = handlers;
      return () => undefined;
    },
  );
  sessionStore.set(testSession);
});
afterEach(() => {
  cleanup();
  sessionStore.set(null);
});
function HistoryControls() {
  const navigate = useNavigate();
  return (
    <>
      <button onClick={() => navigate(-1)}>History back</button>
      <button onClick={() => navigate(1)}>History forward</button>
    </>
  );
}
function setup(path = "/hub/notebook") {
  return render(
    <MemoryRouter initialEntries={[path]}>
      <UserProvider>
        <WorkspaceProvider>
          <Routes>
            <Route path="/hub/notebook/:noteId?" element={<NotebookPage />} />
          </Routes>
          <HistoryControls />
        </WorkspaceProvider>
      </UserProvider>
    </MemoryRouter>,
  );
}

describe("Notebook UI", () => {
  it("starts empty without the save-explanation banner", () => {
    setup();
    expect(
      screen.getByRole("heading", { name: "A little room to think" }),
    ).toBeTruthy();
    expect(
      screen.queryByText(
        "Save keeps your notes on the server. Unsaved changes are lost on reload or sign-out.",
      ),
    ).toBeNull();
    expect(screen.queryByLabelText("Note title")).toBeNull();
  });

  it("creates, edits, searches, and restores the selected note with browser history", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    const title = await screen.findByLabelText("Note title");
    await waitFor(() => expect(document.activeElement).toBe(title));
    await user.type(title, "First idea");
    const body = await screen.findByLabelText("Test note content");
    fireEvent.change(body, { target: { value: "Remember this" } });
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Second idea");
    await user.type(
      screen.getByRole("searchbox", { name: "Search notes" }),
      "First",
    );
    expect(screen.getByRole("link", { name: /First idea/ })).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Second idea/ })).toBeNull();
    await user.click(screen.getByRole("link", { name: /First idea/ }));
    expect(
      (screen.getByLabelText("Note title") as HTMLInputElement).value,
    ).toBe("First idea");
    expect(
      (
        (await screen.findByLabelText(
          "Test note content",
        )) as HTMLTextAreaElement
      ).value,
    ).toContain("Remember this");
    await user.click(screen.getByRole("button", { name: "History back" }));
    expect(
      (screen.getByLabelText("Note title") as HTMLInputElement).value,
    ).toBe("Second idea");
    await user.click(screen.getByRole("button", { name: "History forward" }));
    expect(
      (screen.getByLabelText("Note title") as HTMLInputElement).value,
    ).toBe("First idea");
  });

  it("shows a missing-note state for a direct URL without making a blank note", () => {
    setup("/hub/notebook/missing");
    expect(
      screen.getByRole("heading", { name: "Note unavailable" }),
    ).toBeTruthy();
    expect(screen.queryByLabelText("Note title")).toBeNull();
  });

  it("distinguishes an empty search and clears it without losing drafts", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Research");
    await user.type(screen.getByRole("searchbox"), "No match");
    expect(screen.getByText("No matching notes")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Clear search" }));
    expect(screen.getByRole("link", { name: /Research/ })).toBeTruthy();
  });

  it("keeps drafts across route unmounts in the same session", async () => {
    const user = userEvent.setup();
    const first = setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Keep this draft");
    first.unmount();
    setup();
    await user.click(screen.getByRole("link", { name: /Keep this draft/ }));
    expect(
      (screen.getByLabelText("Note title") as HTMLInputElement).value,
    ).toBe("Keep this draft");
  });

  it("supports desktop hash routes and direct URLs within the same session", async () => {
    window.history.replaceState(null, "", "/#/hub/notebook");
    const user = userEvent.setup();
    const view = render(
      <HashRouter>
        <UserProvider>
          <WorkspaceProvider>
            <Routes>
              <Route path="/hub/notebook/:noteId?" element={<NotebookPage />} />
            </Routes>
          </WorkspaceProvider>
        </UserProvider>
      </HashRouter>,
    );
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Desktop draft");
    expect(window.location.hash).toMatch(/^#\/hub\/notebook\/[a-f0-9-]+$/);
    view.unmount();
    render(
      <HashRouter>
        <UserProvider>
          <WorkspaceProvider>
            <Routes>
              <Route path="/hub/notebook/:noteId?" element={<NotebookPage />} />
            </Routes>
          </WorkspaceProvider>
        </UserProvider>
      </HashRouter>,
    );
    expect(
      (screen.getByLabelText("Note title") as HTMLInputElement).value,
    ).toBe("Desktop draft");
    window.history.replaceState(null, "", "/");
  });

  it("focuses the title after creating from the mobile drawer and restores its trigger on Escape", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getByRole("button", { name: "Notes" }));
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(document.activeElement).toBe(
        screen.getByRole("button", { name: "Notes" }),
      ),
    );
    await user.click(screen.getByRole("button", { name: "Notes" }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "New note",
      }),
    );
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByLabelText("Note title")),
    );
  });

  it("supports mobile note selection and confirms deletion", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Draft to delete");
    await user.click(screen.getByRole("button", { name: "Notes" }));
    const drawer = screen.getByRole("dialog", { name: "Notebook" });
    await user.click(
      within(drawer).getByRole("link", { name: /Draft to delete/ }),
    );
    expect(screen.queryByRole("dialog")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByLabelText("Note title")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Delete note",
      }),
    );
    expect(
      screen.getByRole("heading", { name: "A little room to think" }),
    ).toBeTruthy();
  });
});

describe("Notebook Save button", () => {
  it("sits immediately before Export, creates a notebook on choice, and patches subsequent edits", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Save me");
    const save = screen.getByRole("button", { name: "Save" });
    expect(save.className).toContain("bg-action");
    expect(save.className).toContain("text-on-action");
    await user.click(save);
    expect(screen.getByRole("dialog", { name: "Save note" })).toBeTruthy();
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Save note",
      }),
    );
    await waitFor(() =>
      expect(api.createNotebookNote).toHaveBeenCalledTimes(1),
    );
    expect(api.createNotebook).toHaveBeenCalledWith({
      title: "Personal notes",
    });
    const editorBefore = screen.getByLabelText("Test note content");
    expect(
      (screen.getByRole("button", { name: "Saving…" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    await user.type(screen.getByLabelText("Note title"), " more");
    saveEvents.handlers?.onEvent(
      saveJob({
        status: "completed",
        committedRevision: 1,
        sqliteCommitted: true,
        sequence: 8,
      }),
    );
    await screen.findByText("Unsaved changes");
    expect(screen.getByLabelText("Test note content")).toBe(editorBefore);
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.updateNotebookNote).toHaveBeenCalledWith(
        book.notebookId,
        saveJob().noteId,
        expect.objectContaining({ expectedRevision: 1, title: "Save me more" }),
      ),
    );
    saveEvents.handlers?.onEvent(
      saveJob({
        jobId: "patch-job",
        status: "completed",
        committedRevision: 2,
        sqliteCommitted: true,
        sequence: 8,
      }),
    );
    expect(await screen.findByText("Saved", { exact: true })).toBeTruthy();
  });
  it("loads and opens an existing saved note without creating anything", async () => {
    const user = userEvent.setup();
    api.listNotebooks.mockResolvedValue({ notebooks: [book], hasMore: false });
    const remote = {
      ...saveJob(),
      title: "Existing",
      blocks: [],
      folderId: null,
      revision: 3,
      contentSchemaVersion: 1,
      createdAt: "2026-10-28T00:00:00Z",
      updatedAt: "2026-10-28T00:00:00Z",
    };
    api.listNotebookNotes.mockResolvedValue({
      notes: [remote],
      hasMore: false,
    });
    api.getNotebookNote.mockResolvedValue(remote);
    setup();
    await user.click(await screen.findByRole("link", { name: /Existing/ }));
    expect(await screen.findByDisplayValue("Existing")).toBeTruthy();
    await user.type(screen.getByLabelText("Note title"), " edited");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.updateNotebookNote).toHaveBeenCalledWith(
        book.notebookId,
        remote.noteId,
        expect.objectContaining({ expectedRevision: 3 }),
      ),
    );
    expect(api.createNotebook).not.toHaveBeenCalled();
    expect(api.createNotebookNote).not.toHaveBeenCalled();
  });
  it("keeps the note when the notebook choice is cancelled", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Keep local");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByDisplayValue("Keep local")).toBeTruthy();
    expect(api.createNotebookNote).not.toHaveBeenCalled();
  });
});

function notebookCatalog() {
  const secondBook = { ...book, notebookId: "00000000-0000-4000-8000-000000000003", title: "Finance" };
  const firstNote = {
    notebookId: book.notebookId, noteId: saveJob().noteId, title: "Research source",
    folderId: null, revision: 1, contentSchemaVersion: 1, blocks: [],
    createdAt: "2026-10-28T00:00:00Z", updatedAt: "2026-10-28T00:00:00Z",
  };
  const secondNote = { ...firstNote, notebookId: secondBook.notebookId,
    noteId: "00000000-0000-4000-8000-000000000004", title: "Finance source" };
  api.listNotebooks.mockResolvedValue({ notebooks: [book, secondBook], hasMore: false });
  api.listNotebookNotes.mockImplementation(async (id: string) => ({
    notes: [id === book.notebookId ? firstNote : secondNote], hasMore: false,
  }));
  api.getNotebookNote.mockImplementation(async (_book: string, id: string) =>
    id === firstNote.noteId ? firstNote : secondNote,
  );
  api.createNotebookNote.mockImplementation(async (notebookId: string) => saveJob({
    notebookId, noteId: "00000000-0000-4000-8000-000000000005",
  }));
  return { secondBook, firstNote, secondNote };
}
async function switchNotebook(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByRole("button", { name: "Choose notebook" }));
  await user.click(screen.getByRole("menuitemradio", { name }));
}

describe("Notebook selector", () => {
  it("does not undo a notebook switch when an earlier deletion completes", async () => {
    notebookCatalog();
    let finishDelete = () => {};
    api.deleteNotebookNote.mockImplementationOnce(() => new Promise<void>((resolve) => {
      finishDelete = resolve;
    }));
    const user = userEvent.setup();
    setup();
    await screen.findByText("Research notes · 1");
    await switchNotebook(user, "Finance");
    await user.click(screen.getByRole("link", { name: /Finance source/ }));
    await screen.findByDisplayValue("Finance source");
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete note" }));
    await user.keyboard("{Escape}");
    await switchNotebook(user, "Research");
    await act(async () => { finishDelete(); });
    expect(screen.getByText("Research notes · 1")).toBeTruthy();
    expect(screen.queryByLabelText("Note title")).toBeNull();
  });

  it("scopes the label, count and search to the selected notebook", async () => {
    notebookCatalog();
    const user = userEvent.setup();
    setup();
    expect(await screen.findByText("Research notes · 1")).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Finance source/ })).toBeNull();
    await switchNotebook(user, "Finance");
    expect(screen.getByText("Finance notes · 1")).toBeTruthy();
    expect(screen.getByRole("link", { name: /Finance source/ })).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Research source/ })).toBeNull();
    await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "Research");
    expect(screen.getByText("No matching notes")).toBeTruthy();
    await switchNotebook(user, "Research");
    expect((screen.getByRole("searchbox") as HTMLInputElement).value).toBe("");
    expect(screen.getByRole("link", { name: /Research source/ })).toBeTruthy();
  });

  it("preserves unsaved edits across notebook switches and route remounts", async () => {
    notebookCatalog();
    const user = userEvent.setup();
    const view = setup();
    await screen.findByText("Research notes · 1");
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Working draft");
    fireEvent.change(await screen.findByLabelText("Test note content"), { target: { value: "Keep these edits" } });
    await switchNotebook(user, "Finance");
    expect(screen.queryByRole("link", { name: /Working draft/ })).toBeNull();
    expect(api.createNotebookNote).not.toHaveBeenCalled();
    view.unmount();
    setup();
    expect(screen.getByText("Finance notes · 1")).toBeTruthy();
    await switchNotebook(user, "Research");
    expect(screen.getByText("Research notes · 2")).toBeTruthy();
    await user.click(screen.getByRole("link", { name: /Working draft/ }));
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("Working draft");
    expect((await screen.findByLabelText("Test note content") as HTMLTextAreaElement).value).toContain("Keep these edits");
  });

  it("saves new notes into the selected notebook without creating another notebook", async () => {
    const { secondBook } = notebookCatalog();
    const user = userEvent.setup();
    setup();
    await screen.findByText("Research notes · 1");
    await switchNotebook(user, "Finance");
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Finance draft");
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(api.createNotebookNote).toHaveBeenCalledWith(
      secondBook.notebookId, expect.objectContaining({ title: "Finance draft" }),
    ));
    expect(api.createNotebook).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog", { name: "Save note" })).toBeNull();
  });

  it("uses the owning notebook when opening a direct note URL", async () => {
    const { secondNote } = notebookCatalog();
    setup(`/hub/notebook/${secondNote.noteId}`);
    expect(await screen.findByDisplayValue("Finance source")).toBeTruthy();
    expect(screen.getByText("Finance notes · 1")).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Research source/ })).toBeNull();
  });

  it("keeps unassigned drafts separate and offers an explicit notebook choice when saving", async () => {
    notebookCatalog();
    const user = userEvent.setup();
    setup();
    await screen.findByText("Research notes · 1");
    await switchNotebook(user, "Unsaved notes");
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Unassigned draft");
    expect(screen.getByText("Unsaved notes · 1")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByRole("dialog", { name: "Save note" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await switchNotebook(user, "Finance");
    expect(screen.queryByRole("link", { name: /Unassigned draft/ })).toBeNull();
    await switchNotebook(user, "Unsaved notes");
    expect(screen.getByRole("link", { name: /Unassigned draft/ })).toBeTruthy();
  });

  it("keeps deletion in the selected notebook instead of opening another notebook's note", async () => {
    const { secondBook, secondNote } = notebookCatalog();
    const user = userEvent.setup();
    setup();
    await screen.findByText("Research notes · 1");
    await switchNotebook(user, "Finance");
    await user.click(screen.getByRole("link", { name: /Finance source/ }));
    await screen.findByDisplayValue("Finance source");
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete note" }));
    await waitFor(() => expect(api.deleteNotebookNote).toHaveBeenCalledWith(secondBook.notebookId, secondNote.noteId));
    expect(await screen.findByText("Finance notes · 0")).toBeTruthy();
    expect(screen.queryByLabelText("Note title")).toBeNull();
    expect(screen.queryByRole("link", { name: /Research source/ })).toBeNull();
  });
});
