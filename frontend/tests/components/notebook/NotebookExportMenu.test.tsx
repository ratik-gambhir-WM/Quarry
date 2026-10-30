// @vitest-environment happy-dom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NotebookExportMenu } from "@/components/notebook/NotebookExportMenu";
import type { NotebookDraft } from "@/data/notebook/notebookSession";
const { saveFile, createNotebookExport } = vi.hoisted(() => ({
  saveFile: vi.fn(),
  createNotebookExport: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({ runtime: { platform: { saveFile } } }));
vi.mock("@/components/notebook/exportNotebook", () => ({
  createNotebookExport,
}));
const note: NotebookDraft = {
  id: "first",
  title: "Draft",
  blocks: [],
  contentSchemaVersion: 1,
  createdAt: "now",
  updatedAt: "now",
};
afterEach(cleanup);
beforeEach(() => {
  vi.resetAllMocks();
  createNotebookExport.mockReturnValue({
    contents: "# Draft",
    suggestedName: "Draft.md",
  });
  saveFile.mockResolvedValue(true);
});
describe("Notebook export menu", () => {
  it("disables export without a selected note", () => {
    render(<NotebookExportMenu />);
    expect(
      (screen.getByRole("button", { name: "Export" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });
  it.each(["Markdown", "HTML"])(
    "exports a snapshot as %s through the selected platform",
    async (format) => {
      const user = userEvent.setup();
      render(<NotebookExportMenu note={note} />);
      await user.click(screen.getByRole("button", { name: "Export" }));
      await user.click(
        screen.getByRole("menuitem", { name: new RegExp(format) }),
      );
      await waitFor(() => expect(saveFile).toHaveBeenCalledOnce());
      expect(createNotebookExport).toHaveBeenCalledWith(
        note,
        format.toLowerCase(),
      );
      expect(createNotebookExport.mock.calls[0][0]).not.toBe(note);
      expect(screen.getByRole("status").textContent).toBe("Export created.");
    },
  );
  it("retains the draft and permits retry after a failed save", async () => {
    saveFile.mockRejectedValueOnce(new Error("Could not write the export."));
    const user = userEvent.setup();
    render(<NotebookExportMenu note={note} />);
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: /Markdown/ }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(note.title).toBe("Draft");
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: /HTML/ }));
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Export created."),
    );
  });
  it("blocks duplicate exports and ignores completion after switching notes", async () => {
    let finish: (saved: boolean) => void = () => {};
    saveFile.mockReturnValue(
      new Promise<boolean>((resolve) => {
        finish = resolve;
      }),
    );
    const user = userEvent.setup();
    const view = render(<NotebookExportMenu key="first" note={note} />);
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: /Markdown/ }));
    await waitFor(() => expect(saveFile).toHaveBeenCalledOnce());
    expect(
      (screen.getByRole("button", { name: "Exporting…" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    view.rerender(
      <NotebookExportMenu key="second" note={{ ...note, id: "second" }} />,
    );
    await act(async () => {
      finish(true);
    });
    expect(screen.getByRole("status").textContent).toBe("");
    expect(saveFile).toHaveBeenCalledOnce();
  });
  it("reports a cancelled native save without an error", async () => {
    saveFile.mockResolvedValue(false);
    const user = userEvent.setup();
    render(<NotebookExportMenu note={note} />);
    await user.click(screen.getByRole("button", { name: "Export" }));
    await user.click(screen.getByRole("menuitem", { name: /Markdown/ }));
    await waitFor(() =>
      expect(screen.getByRole("status").textContent).toBe("Export cancelled."),
    );
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
