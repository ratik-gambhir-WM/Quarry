import { describe, expect, it } from "vitest";
import {
  hasDataRoomFiles,
  isUnconfiguredDataRoomError,
  type DataRoomTreeNode,
} from "./dataRoom";

describe("hasDataRoomFiles", () => {
  it("reports an unconfigured or folder-only data room as empty", () => {
    const folderOnlyTree: DataRoomTreeNode[] = [
      {
        children: [
          {
            children: [],
            id: "empty-folder",
            kind: "folder",
            name: "Empty folder",
          },
        ],
        id: "root",
        kind: "folder",
        name: "Data Room",
      },
    ];

    expect(hasDataRoomFiles([])).toBe(false);
    expect(hasDataRoomFiles(folderOnlyTree)).toBe(false);
  });

  it("finds a file nested anywhere in the data-room tree", () => {
    const tree: DataRoomTreeNode[] = [
      {
        children: [
          {
            id: "financials/report.pdf",
            kind: "pdf",
            name: "report.pdf",
            relativePath: "financials/report.pdf",
          },
        ],
        id: "financials",
        kind: "folder",
        name: "Financials",
      },
    ];

    expect(hasDataRoomFiles(tree)).toBe(true);
  });
});

describe("isUnconfiguredDataRoomError", () => {
  it("recognizes the legacy API response for a deal without a data-room location", () => {
    expect(
      isUnconfiguredDataRoomError(
        new Error('no local data-room root is configured for deal "DEAL-123"'),
      ),
    ).toBe(true);
    expect(isUnconfiguredDataRoomError(new Error("the configured folder is unavailable"))).toBe(
      false,
    );
  });
});
