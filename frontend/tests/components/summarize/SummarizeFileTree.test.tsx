// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SummarizeFileTree } from "@/components/summarize/SummarizeFileTree";
import { buildFileTree } from "@/data/summarize";

afterEach(cleanup);

describe("SummarizeFileTree", () => {
  it("exposes folder expansion and selected-file actions", () => {
    const onClear = vi.fn();
    const onSelectAll = vi.fn();
    const onToggleFile = vi.fn();
    const onToggleFolder = vi.fn();
    const fileTree = buildFileTree([
      {
        filename: "report.pdf",
        mimeType: "application/pdf",
        path: "Folder/report.pdf",
        relativePath: "Folder/report.pdf",
        sizeBytes: 1024,
        supported: true,
      },
    ], "Folder");

    render(
      <SummarizeFileTree
        expandedFolderIds={new Set([fileTree.id, `${fileTree.id}/Folder`])}
        fileTree={fileTree}
        onClear={onClear}
        onSelectAll={onSelectAll}
        onToggleFile={onToggleFile}
        onToggleFolder={onToggleFolder}
        selectedFileCount={1}
        selectedFilePaths={new Set(["Folder/report.pdf"])}
        supportedFileCount={1}
      />,
    );

    expect(screen.getByText("1 of 1 selected")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /report\.pdf/u }));
    fireEvent.click(screen.getByRole("button", { name: "Select All" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    fireEvent.click(screen.getAllByRole("button", { name: /Folder/u })[0]);

    expect(onToggleFile).toHaveBeenCalledWith("Folder/report.pdf");
    expect(onSelectAll).toHaveBeenCalledTimes(1);
    expect(onClear).toHaveBeenCalledTimes(1);
    expect(onToggleFolder).toHaveBeenCalledWith(fileTree.id);
  });
});
