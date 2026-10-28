// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HashRouter, MemoryRouter, Route, Routes, useNavigate } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { sessionStore } from "@/auth/sessionStore";
import { UserProvider } from "@/auth/UserProvider";
import { testSession } from "@/test/fixtures/auth";
import { WorkspaceProvider } from "@/app/WorkspaceProvider";
import { NotebookPage } from "@/pages/NotebookPage";

vi.mock("@quarry/runtime", () => ({ runtime: { api: { listDeals: async () => [] } } }));
vi.mock("@/components/notebook/NotebookEditor", () => ({
  NotebookEditor: ({ blocks, onChange }: { blocks: unknown[]; onChange: (blocks: unknown[]) => void }) => (
    <textarea aria-label="Test note content" defaultValue={JSON.stringify(blocks)}
      onChange={(event) => onChange([{ type: "paragraph", content: event.currentTarget.value }])} />
  ),
}));

beforeEach(() => { sessionStore.set(testSession); });
afterEach(() => { cleanup(); sessionStore.set(null); });
function HistoryControls() {
  const navigate = useNavigate();
  return <><button onClick={() => navigate(-1)}>History back</button><button onClick={() => navigate(1)}>History forward</button></>;
}
function setup(path = "/hub/notebook") {
  return render(<MemoryRouter initialEntries={[path]}><UserProvider><WorkspaceProvider>
    <Routes><Route path="/hub/notebook/:noteId?" element={<NotebookPage />} /></Routes>
    <HistoryControls />
  </WorkspaceProvider></UserProvider></MemoryRouter>);
}

describe("Notebook UI", () => {
  it("starts empty and explains that notes are session drafts", async () => {
    setup();
    expect(screen.getByRole("heading", { name: "A little room to think" })).toBeTruthy();
    expect(screen.getByText("Notes stay in this session only. Reloading or signing out clears them.")).toBeTruthy();
    expect(screen.queryByLabelText("Note title")).toBeNull();
  });

  it("creates, edits, searches, and restores the selected note with browser history", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    const title = await screen.findByLabelText("Note title");
    expect(document.activeElement).toBe(title);
    await user.type(title, "First idea");
    const body = await screen.findByLabelText("Test note content");
    fireEvent.change(body, { target: { value: "Remember this" } });
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Second idea");
    await user.type(screen.getByRole("searchbox", { name: "Search notes" }), "First");
    expect(screen.getByRole("link", { name: /First idea/ })).toBeTruthy();
    expect(screen.queryByRole("link", { name: /Second idea/ })).toBeNull();
    await user.click(screen.getByRole("link", { name: /First idea/ }));
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("First idea");
    expect((await screen.findByLabelText("Test note content") as HTMLTextAreaElement).value).toContain("Remember this");
    await user.click(screen.getByRole("button", { name: "History back" }));
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("Second idea");
    await user.click(screen.getByRole("button", { name: "History forward" }));
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("First idea");
  });

  it("shows a missing-note state for a direct URL without making a blank note", () => {
    setup("/hub/notebook/missing");
    expect(screen.getByRole("heading", { name: "Note unavailable" })).toBeTruthy();
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
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("Keep this draft");
  });

  it("supports desktop hash routes and direct URLs within the same session", async () => {
    window.history.replaceState(null, "", "/#/hub/notebook");
    const user = userEvent.setup();
    const view = render(<HashRouter><UserProvider><WorkspaceProvider><Routes>
      <Route path="/hub/notebook/:noteId?" element={<NotebookPage />} />
    </Routes></WorkspaceProvider></UserProvider></HashRouter>);
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Desktop draft");
    expect(window.location.hash).toMatch(/^#\/hub\/notebook\/[a-f0-9-]+$/);
    view.unmount();
    render(<HashRouter><UserProvider><WorkspaceProvider><Routes>
      <Route path="/hub/notebook/:noteId?" element={<NotebookPage />} />
    </Routes></WorkspaceProvider></UserProvider></HashRouter>);
    expect((screen.getByLabelText("Note title") as HTMLInputElement).value).toBe("Desktop draft");
    window.history.replaceState(null, "", "/");
  });

  it("focuses the title after creating from the mobile drawer and restores its trigger on Escape", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getByRole("button", { name: "Notes" }));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Notes" })));
    await user.click(screen.getByRole("button", { name: "Notes" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "New note" }));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByLabelText("Note title")));
  });

  it("supports mobile note selection and confirms deletion", async () => {
    const user = userEvent.setup();
    setup();
    await user.click(screen.getAllByRole("button", { name: "New note" })[0]);
    await user.type(screen.getByLabelText("Note title"), "Draft to delete");
    await user.click(screen.getByRole("button", { name: "Notes" }));
    const drawer = screen.getByRole("dialog", { name: "Notebook" });
    await user.click(within(drawer).getByRole("link", { name: /Draft to delete/ }));
    expect(screen.queryByRole("dialog")).toBeNull();
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.getByLabelText("Note title")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Delete note" }));
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Delete note" }));
    expect(screen.getByRole("heading", { name: "A little room to think" })).toBeTruthy();
  });
});
