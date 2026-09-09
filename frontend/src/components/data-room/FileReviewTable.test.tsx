// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DataRoomFileEntry } from "../../data/dataRoom";
import { FileReviewTable } from "./FileReviewTable";

const file: DataRoomFileEntry = {
  folderPath: ["Saved documents"],
  node: {
    id: "file-1",
    kind: "doc",
    name: "diligence-notes.docx",
    relativePath: "Saved documents/diligence-notes.docx",
  },
  source: "local",
};

afterEach(cleanup);

describe("FileReviewTable", () => {
  it("expands its left-aligned search control and restores focus when dismissed", async () => {
    const user = userEvent.setup({ skipHover: true });
    render(<FileReviewTable files={[file]} onSelectFile={vi.fn()} />);

    expect(screen.queryByText(/Illustrative AI review/)).toBeNull();
    const trigger = screen.getByRole("button", { name: "Open file review search" });
    expect(screen.queryByRole("searchbox", { name: "Search file reviews" })).toBeNull();

    await user.click(trigger);

    const search = screen.getByRole("searchbox", { name: "Search file reviews" });
    expect(document.activeElement).toBe(search);
    await user.type(search, "missing result");
    expect(screen.getByText("No file reviews match these filters.")).not.toBeNull();

    await user.keyboard("{Escape}");

    const restoredTrigger = screen.getByRole("button", { name: "Open file review search" });
    expect(document.activeElement).toBe(restoredTrigger);
  });
});
