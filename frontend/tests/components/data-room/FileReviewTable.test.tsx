// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DataRoomFileEntry } from "@/data/dataRoom";
import { FileReviewTable } from "@/components/data-room/FileReviewTable";

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
  it("embeds the review grid directly in the Data Room surface", () => {
    render(<FileReviewTable files={[file]} onSelectFile={vi.fn()} />);

    const grid = document.querySelector('[data-slot="data-grid"]');

    expect(grid?.className).toContain("border-t");
    expect(grid?.parentElement?.className).not.toContain("rounded-2xl");
    expect(grid?.parentElement?.className).not.toContain(
      "bg-surface-container-lowest",
    );
  });

  it("expands its left-aligned search control and restores focus when dismissed", async () => {
    const user = userEvent.setup({ skipHover: true });
    render(<FileReviewTable files={[file]} onSelectFile={vi.fn()} />);

    expect(screen.queryByText(/Illustrative AI review/)).toBeNull();
    const trigger = screen.getByRole("button", {
      name: "Open file review search",
    });
    expect(
      screen.queryByRole("searchbox", { name: "Search file reviews" }),
    ).toBeNull();

    await user.click(trigger);

    const search = screen.getByRole("searchbox", {
      name: "Search file reviews",
    });
    expect(document.activeElement).toBe(search);
    await user.type(search, "missing result");
    expect(
      screen.getByText("No file reviews match these filters."),
    ).not.toBeNull();

    await user.keyboard("{Escape}");

    const restoredTrigger = screen.getByRole("button", {
      name: "Open file review search",
    });
    expect(document.activeElement).toBe(restoredTrigger);
  });

  it("recognizes image and Markdown extensions despite generic document kinds and preserves file activation", async () => {
    const user = userEvent.setup();
    const onSelectFile = vi.fn();
    const files = ["notes.docx", "photo.JPG", "diagram.png", "README.md"].map((name, index): DataRoomFileEntry => ({
      ...file,
      node: { ...file.node, id: `typed-file-${index}`, name },
    }));
    render(<FileReviewTable files={files} onSelectFile={onSelectFile} />);
    await user.selectOptions(screen.getByLabelText("File type"), "Image");
    expect(screen.getByRole("button", { name: "photo.JPG" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "diagram.png" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "notes.docx" })).toBeNull();
    expect(screen.queryByRole("button", { name: "README.md" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "diagram.png" }));
    expect(onSelectFile).toHaveBeenCalledWith(files[2].node);
    await user.selectOptions(screen.getByLabelText("File type"), "Markdown");
    expect(screen.getByRole("button", { name: "README.md" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "photo.JPG" })).toBeNull();
  });
});
