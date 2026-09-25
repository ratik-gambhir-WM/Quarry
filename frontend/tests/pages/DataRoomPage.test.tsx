// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DataRoomTreeNode } from "@/data/dataRoom";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { DataRoomPage } from "@/pages/DataRoomPage";

const {
  listDealDataRoom,
  listDealDocuments,
  previewDealDocument,
  previewModule,
} = vi.hoisted(
  () => ({
    listDealDataRoom: vi.fn(),
    listDealDocuments: vi.fn(),
    previewDealDocument: vi.fn(),
    previewModule: { loaded: false },
  }),
);

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { listDealDataRoom, listDealDocuments, previewDealDocument },
    platform: {},
    target: "web",
  },
}));

vi.mock("@/pages/DealRoomPage", () => ({
  useDealRoom: () => ({
    deal: workspaceDeals[0],
    deals: workspaceDeals,
    email: "analyst@example.com",
    navigationState: { email: "analyst@example.com" },
  }),
}));

vi.mock("@/components/data-room/DataRoomExplorer", () => ({
  DataRoomExplorer: ({
    dealRoomPath,
    documentSearch,
    onUploadNewFile,
  }: {
    dealRoomPath: string;
    documentSearch: { boundaryElement: HTMLElement | null; currentFileName: string };
    onUploadNewFile: () => void;
  }) => (
    <aside>
      Data room explorer
      <a href={dealRoomPath}>Back to Deal Room</a>
      <nav aria-label="Data room tools">
        <button onClick={onUploadNewFile} type="button">Upload files</button>
        <button type="button">Search document</button>
      <span>{documentSearch.currentFileName}</span>
      <span data-testid="search-boundary-class">
        {documentSearch.boundaryElement?.className}
      </span>
      </nav>
    </aside>
  ),
}));

vi.mock("@/components/data-room/FileReviewTable", () => ({
  FileReviewTable: ({
    onSelectFile,
  }: {
    onSelectFile: (file: DataRoomTreeNode) => void;
  }) => (
    <button
      onClick={() =>
        onSelectFile({
          id: "local:test.pdf",
          kind: "pdf",
          name: "test.pdf",
          relativePath: "test.pdf",
        })
      }
      type="button"
    >
      Open test document
    </button>
  ),
}));

vi.mock("@/components/data-room/DocumentPreviewPanel", () => ({
  get DocumentPreviewPanel() {
    previewModule.loaded = true;
    return ({ onClose }: { onClose: () => void }) => (
      <section aria-label="Selected document preview">
        <button onClick={onClose} type="button">Close preview</button>
      </section>
    );
  },
}));

afterEach(cleanup);

describe("DataRoomPage", () => {
  beforeEach(() => {
    listDealDataRoom.mockReset();
    listDealDocuments.mockReset();
    previewDealDocument.mockReset();
    previewModule.loaded = false;
    listDealDocuments.mockResolvedValue([]);
    listDealDataRoom.mockResolvedValue({
      dealId: "project-alpha",
      rootName: "Data Room",
      rootPath: "/synthetic/data-room",
      tree: [
        {
          id: "local:test.pdf",
          kind: "pdf",
          name: "test.pdf",
          relativePath: "test.pdf",
        },
      ],
    });
    previewDealDocument.mockResolvedValue({
      fileName: "test.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0xLjQ=",
      sourceKind: "native",
    });
  });

  it("renders Data Room tools in the explorer and keeps their search boundary above preview chrome", async () => {
    const user = userEvent.setup({ skipHover: true });
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/data-room"]}>
        <Routes>
          <Route element={<DataRoomPage />} path="/hub/deals/:dealId/data-room" />
        </Routes>
      </MemoryRouter>,
    );

    expect(document.querySelectorAll(".workspace-main-surface")).toHaveLength(1);
    expect(screen.queryByText("Data room explorer")).toBeNull();
    expect(document.querySelector("[data-workspace-sidebar-replacement]")).toBeNull();
    expect(previewModule.loaded).toBe(false);
    await user.click(await screen.findByRole("button", { name: "Open test document" }));

    expect(await screen.findByRole("region", { name: "Selected document preview" })).not.toBeNull();
    expect(previewModule.loaded).toBe(true);
    expect(screen.getByText("Data room explorer")).not.toBeNull();
    expect(document.querySelector("[data-workspace-sidebar-replacement]")).not.toBeNull();
    expect(screen.getByRole("link", { name: "Back to Deal Room" }).getAttribute("href"))
      .toBe("/hub/deals/project-alpha");
    expect(screen.getByRole("navigation", { name: "Data room tools" })).not.toBeNull();
    expect(screen.getByTestId("search-boundary-class").textContent).toContain("z-50");
    expect(screen.getByText("test.pdf")).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Upload files" }));
    expect((await screen.findByRole("dialog", { name: "Upload files" })).dataset.slot)
      .toBe("dialog-content");
    expect(screen.getByRole("button", { name: "Browse files on your Mac" }))
      .toBe(document.activeElement);

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await user.click(screen.getByRole("button", { name: "Close preview" }));
    expect(screen.queryByText("Data room explorer")).toBeNull();
    expect(document.querySelector("[data-workspace-sidebar-replacement]")).toBeNull();
  });
});
