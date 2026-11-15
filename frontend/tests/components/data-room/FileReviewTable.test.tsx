// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DataRoomFileEntry } from "@/data/dataRoom";
import { FileReviewTable } from "@/components/data-room/FileReviewTable";
import { formatShortUtcDate } from "@/lib/formatters";

const file: DataRoomFileEntry = {
  folderPath: ["Saved documents"],
  node: {
    id: "folder/file 1",
    kind: "doc",
    name: "diligence-notes.docx",
    relativePath: "Saved documents/diligence-notes.docx",
  },
  source: "local",
};

afterEach(cleanup);

describe("FileReviewTable", () => {
  it("shows metadata columns and reveals labelled analysis only when expanded", async () => {
    const user = userEvent.setup();
    render(<FileReviewTable files={[{ ...file, node: { ...file.node, sizeBytes: 2048, addedAt: "2026-11-15T12:00:00Z" } }]} onSelectFile={vi.fn()} />);

    const headers = screen.getAllByRole("columnheader");
    expect(headers.map((header) => header.textContent)).toEqual([
      "File", "File type", "File size", "File category", "Date added", "RL ID",
    ]);
    expect(screen.getByText("2.0 KB")).toBeTruthy();
    expect(screen.getByText("N/A")).toBeTruthy();
    expect(screen.getByText(formatShortUtcDate("2026-11-15T12:00:00Z"))).toBeTruthy();
    expect(screen.getByText("Coming soon")).toBeTruthy();
    expect(screen.queryByText("Key findings")).toBeNull();

    const toggle = screen.getByRole("button", { name: "Show review for diligence-notes.docx" });
    expect(toggle.getAttribute("aria-expanded")).toBe("false");
    await user.click(toggle);
    const details = screen.getByRole("region", { name: "Review for diligence-notes.docx" });
    expect(toggle.getAttribute("aria-controls")).toBe(details.id);
    expect(details.id).not.toMatch(/\s/);
    for (const name of ["Key findings", "Risks", "Opportunities", "Key question impact"]) {
      expect(within(details).getByText(name)).toBeTruthy();
    }
    expect(within(details).getByText(/Sample analysis, not generated from this file/)).toBeTruthy();
    await user.click(toggle);
    expect(screen.queryByRole("region", { name: "Review for diligence-notes.docx" })).toBeNull();
    expect(document.activeElement).toBe(toggle);
  });

  it("sorts dates by instant across offsets, leaves unknown dates last and preserves expansion", async () => {
    const user = userEvent.setup();
    const datedFiles = [
      { ...file, node: { ...file.node, id: "a", name: "A.pdf", addedAt: "2026-12-31T23:00:00-03:00" } },
      { ...file, node: { ...file.node, id: "b", name: "B.pdf", addedAt: "2027-01-01T01:00:00Z" } },
      { ...file, node: { ...file.node, id: "c", name: "C.pdf" } },
    ];
    render(<FileReviewTable files={datedFiles} onSelectFile={vi.fn()} />);
    expect(screen.getByText("—")).toBeTruthy();
    expect(screen.getAllByText("Coming soon")).toHaveLength(3);
    expect(screen.queryByRole("button", { name: "RL ID" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "Show review for A.pdf" }));
    for (const [direction, expected] of [
      ["Asc", ["B.pdf", "A.pdf", "C.pdf"]],
      ["Desc", ["A.pdf", "B.pdf", "C.pdf"]],
    ] as const) {
      await user.click(screen.getByRole("button", { name: "Date added" }));
      await user.click(screen.getByRole("menuitem", { name: direction }));
      expect(screen.getAllByRole("button", { name: /^[ABC]\.pdf$/ }).map((button) => button.textContent)).toEqual(expected);
      expect(screen.getByRole("region", { name: "Review for A.pdf" })).toBeTruthy();
    }
  });

  it("opens independent rows by metadata click or keyboard and keeps file activation separate", async () => {
    const user = userEvent.setup();
    const onSelectFile = vi.fn();
    const second = { ...file, node: { ...file.node, id: "file-2", name: "second.pdf" } };
    render(<FileReviewTable files={[file, second]} onSelectFile={onSelectFile} />);

    await user.click(screen.getAllByText("N/A")[0]);
    const firstToggle = screen.getByRole("button", { name: "Hide review for diligence-notes.docx" });
    expect(document.activeElement).toBe(firstToggle);
    await user.keyboard("{ArrowLeft}");
    expect(firstToggle.getAttribute("aria-expanded")).toBe("false");
    await user.keyboard("{ArrowRight}");
    expect(firstToggle.getAttribute("aria-expanded")).toBe("true");
    await user.click(screen.getByRole("button", { name: "Show review for second.pdf" }));
    expect(screen.getAllByRole("region")).toHaveLength(2);
    await user.click(screen.getByRole("button", { name: "second.pdf" }));
    expect(onSelectFile).toHaveBeenCalledWith(second.node);
    expect(screen.getAllByRole("region")).toHaveLength(2);
  });

  it("sorts file sizes numerically and retains expanded identity through filters and updates", async () => {
    const user = userEvent.setup();
    const smaller = { ...file, node: { ...file.node, id: "small", name: "small.pdf", sizeBytes: 512 } };
    const larger = { ...file, node: { ...file.node, id: "large", name: "large.docx", sizeBytes: 2048 } };
    const onSelectFile = vi.fn();
    const { rerender } = render(<FileReviewTable files={[larger, smaller]} onSelectFile={onSelectFile} />);
    await user.click(screen.getByRole("button", { name: "Show review for small.pdf" }));
    await user.click(screen.getByRole("button", { name: "File size" }));
    await user.click(screen.getByRole("menuitem", { name: "Desc" }));
    const names = screen.getAllByRole("button", { name: /^(small.pdf|large.docx)$/ });
    expect(names[0].textContent).toBe("large.docx");
    await user.click(screen.getByRole("button", { name: "File size" }));
    await user.click(screen.getByRole("menuitem", { name: "Asc" }));
    expect(screen.getAllByRole("button", { name: /^(small.pdf|large.docx)$/ })[0].textContent).toBe("small.pdf");
    await user.selectOptions(screen.getByLabelText("File type"), "Document");
    expect(screen.queryByRole("region")).toBeNull();
    await user.selectOptions(screen.getByLabelText("File type"), "All types");
    rerender(<FileReviewTable files={[smaller, larger]} onSelectFile={onSelectFile} />);
    expect(screen.getByRole("region", { name: "Review for small.pdf" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Show review for large.docx" })).toBeTruthy();
  });

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
