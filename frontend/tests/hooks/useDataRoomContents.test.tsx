// @vitest-environment happy-dom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDataRoomContents } from "@/hooks/useDataRoomContents";

const { listDealDataRoom, listDealDocuments } = vi.hoisted(() => ({
  listDealDataRoom: vi.fn(),
  listDealDocuments: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({ runtime: { api: { listDealDataRoom, listDealDocuments } } }));

afterEach(cleanup);

describe("useDataRoomContents", () => {
  beforeEach(() => {
    listDealDataRoom.mockReset();
    listDealDocuments.mockReset();
    listDealDataRoom.mockResolvedValue({
      dealId: "deal-1",
      rootName: "Data Room",
      rootPath: "/synthetic/deal-1",
      tree: [{ id: "local:file.pdf", kind: "pdf", name: "file.pdf", relativePath: "file.pdf" }],
    });
    listDealDocuments.mockResolvedValue([{ displayName: "saved.xlsx", fileId: "stored-1" }]);
  });

  it("merges stored documents before local nodes and refreshes stored data independently", async () => {
    const { result } = renderHook(() => useDataRoomContents("deal-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.explorerNodes.map((node) => node.id)).toEqual(["stored-documents", "local:file.pdf"]);
    expect(result.current.reviewFiles).toHaveLength(2);
    expect(result.current.rootPath).toBe("/synthetic/deal-1");

    act(() => result.current.reloadStored());
    await waitFor(() => expect(listDealDocuments).toHaveBeenCalledTimes(2));
    expect(listDealDataRoom).toHaveBeenCalledTimes(1);
  });

  it("normalizes an unconfigured local root without hiding stored documents", async () => {
    listDealDataRoom.mockRejectedValue(new Error("no local data-room root is configured"));
    const { result } = renderHook(() => useDataRoomContents("deal-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.isUnavailable).toBe(false);
    expect(result.current.hasFiles).toBe(true);
  });

  it("preserves the aggregate unavailable state when either source fails", async () => {
    listDealDocuments.mockRejectedValue(new Error("Stored documents unavailable"));
    const { result } = renderHook(() => useDataRoomContents("deal-1"));
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    expect(result.current.isUnavailable).toBe(true);
    expect(result.current.errorMessage).toContain("Stored documents unavailable");
  });
});
