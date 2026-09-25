// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { DataRoomExplorer } from "@/components/data-room/DataRoomExplorer";

afterEach(cleanup);

describe("DataRoomExplorer", () => {
  it("places evenly distributed Data Room tools above Saved documents", () => {
    render(
      <MemoryRouter>
        <DataRoomExplorer
          dealName="Project Alpha"
          dealRoomPath="/hub/deals/project-alpha"
          documentSearch={{
            currentFileName: "Synthetic_Terms.pdf",
            currentPageCount: 3,
            onActivateResult: vi.fn(),
          }}
          nodes={[
            {
              defaultExpanded: true,
              id: "saved-documents",
              kind: "folder",
              name: "Saved documents",
              relativePath: "Saved documents",
            },
          ]}
          onSelectFile={vi.fn()}
          onUploadNewFile={vi.fn()}
        />
      </MemoryRouter>,
    );

    const tools = screen.getByRole("toolbar", { name: "Data room tools" });
    const savedDocuments = screen.getByRole("button", { name: "Saved documents" });

    expect(screen.queryByRole("button", { name: "New analysis" })).toBeNull();
    expect(tools.className).toContain("grid-cols-4");
    expect(tools.compareDocumentPosition(savedDocuments) & Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
  });
});
