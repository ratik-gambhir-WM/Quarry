// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DataRoomTreeNode } from "../data/dataRoom";
import { DataRoomPage } from "./DataRoomPage";

const { listDealDataRoom, listDealDocuments, listDeals, previewDealDocument } = vi.hoisted(
  () => ({
    listDealDataRoom: vi.fn(),
    listDealDocuments: vi.fn(),
    listDeals: vi.fn(),
    previewDealDocument: vi.fn(),
  }),
);

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { listDealDataRoom, listDealDocuments, listDeals, previewDealDocument },
    platform: {},
    target: "web",
  },
}));

vi.mock("../components/data-room/DataRoomExplorer", () => ({
  DataRoomExplorer: () => <aside>Data room explorer</aside>,
}));

vi.mock("../components/data-room/DataRoomArcMenu", () => ({
  DataRoomArcMenu: ({
    documentSearch,
  }: {
    documentSearch: { currentFileName: string; portalContainer: HTMLElement | null };
  }) => (
    <nav aria-label="Data room views">
      <button type="button">Search document</button>
      <span>{documentSearch.currentFileName}</span>
      <span data-testid="search-portal-class">
        {documentSearch.portalContainer?.className}
      </span>
    </nav>
  ),
}));

vi.mock("../components/data-room/FileReviewTable", () => ({
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

vi.mock("../components/data-room/DocumentPreviewPanel", () => ({
  DocumentPreviewPanel: () => <section aria-label="Selected document preview" />,
}));

afterEach(cleanup);

describe("DataRoomPage", () => {
  beforeEach(() => {
    listDealDataRoom.mockReset();
    listDealDocuments.mockReset();
    listDeals.mockReset();
    previewDealDocument.mockReset();
    listDeals.mockResolvedValue([]);
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

  it("keeps the arc menu mounted and its search portal above file-review chrome", async () => {
    const user = userEvent.setup({ skipHover: true });
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/data-room"]}>
        <Routes>
          <Route element={<DataRoomPage />} path="/hub/deals/:dealId/data-room" />
        </Routes>
      </MemoryRouter>,
    );

    const arcMenu = await screen.findByRole("navigation", { name: "Data room views" });
    const searchAction = screen.getByRole("button", { name: "Search document" });
    expect(screen.getByText("Data Room")).not.toBeNull();
    expect(screen.getByTestId("search-portal-class").textContent).toContain("z-50");
    await user.click(await screen.findByRole("button", { name: "Open test document" }));

    expect(await screen.findByRole("region", { name: "Selected document preview" })).not.toBeNull();
    expect(screen.getByRole("navigation", { name: "Data room views" })).toBe(arcMenu);
    expect(screen.getByRole("button", { name: "Search document" })).toBe(searchAction);
    expect(screen.getByText("test.pdf")).not.toBeNull();
  });
});
