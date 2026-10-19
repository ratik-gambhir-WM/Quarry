// @vitest-environment happy-dom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getWorkspaceDataSource, useWorkspaceDeals } from "@/hooks/useWorkspaceDeals";
import type { PersistedDeal } from "@/contracts/quarryApi";
import { testDeal, testMetadata, testParty } from "@/test/fixtures/deals";

const { listDeals } = vi.hoisted(() => ({ listDeals: vi.fn() }));

vi.mock("@quarry/runtime", () => ({ runtime: { api: { listDeals } } }));

afterEach(cleanup);

describe("useWorkspaceDeals", () => {
  beforeEach(() => listDeals.mockReset());

  it("keeps API failures distinct from successful demo data and retries", async () => {
    listDeals.mockRejectedValueOnce(new Error("Deals service unavailable")).mockResolvedValueOnce([]);
    const { result } = renderHook(() => useWorkspaceDeals("api"));

    await waitFor(() => expect(result.current.resource.status).toBe("error"));
    expect(result.current.resource).toEqual({ message: "Deals service unavailable", status: "error" });

    act(() => result.current.retry());
    await waitFor(() => expect(result.current.resource.status).toBe("success"));
    expect(result.current.resource).toEqual({ deals: [], source: "server", status: "success" });
    expect(listDeals).toHaveBeenCalledTimes(2);
  });

  it("loads fixtures only in explicit demo mode", async () => {
    const { result } = renderHook(() => useWorkspaceDeals("demo"));
    await waitFor(() => expect(result.current.resource.status).toBe("success"));

    expect(result.current.resource.status === "success" && result.current.resource.source).toBe("demo");
    expect(result.current.resource.status === "success" && result.current.resource.deals.length).toBeGreaterThan(0);
    expect(listDeals).not.toHaveBeenCalled();
  });

  it("rejects invalid composition values", () => {
    expect(() => getWorkspaceDataSource("fixtures")).toThrow("Invalid VITE_WORKSPACE_DATA_SOURCE");
  });

  it("retains uploaded SOW details when saving metadata and then editing the deal", async () => {
    listDeals.mockResolvedValue([]);
    const { result } = renderHook(() => useWorkspaceDeals("api"));
    await waitFor(() => expect(result.current.resource.status).toBe("success"));
    const deal = testDeal();
    const metadata = testMetadata();
    act(() => result.current.saveDealResult({
      deal, metadata, extraction: { keyQuestions: ["Question?"] },
      files: [{ filename: "sow.pdf", path: "sow.pdf", relativePath: "sow.pdf", sizeBytes: 10 }],
    }, "sow.pdf"));
    const saved = result.current.resource;
    if (saved.status !== "success") throw new Error("expected loaded deals");
    expect(saved.deals[0]?.room.resources.find((r) => r.id === "sow")).toMatchObject({ availability: "available", sourceName: "sow.pdf" });
    act(() => result.current.saveDealResult({ deal: { ...deal, dealName: "Edited deal" }, metadata }));
    const edited = result.current.resource;
    if (edited.status !== "success") throw new Error("expected loaded deals");
    expect(edited.deals[0]?.room.name).toBe("Edited deal");
    expect(edited.deals[0]?.room.resources.find((r) => r.id === "sow")).toMatchObject({ availability: "available", sourceName: "sow.pdf" });
  });

  it("updates saved links without losing session SOW details or persisted questions", async () => {
    listDeals.mockResolvedValue([]);
    const { result } = renderHook(() => useWorkspaceDeals("api"));
    await waitFor(() => expect(result.current.resource.status).toBe("success"));
    const deal = testDeal();
    const metadata = { ...testMetadata(), keyQuestionsJson: '["Saved question?"]' };
    act(() => result.current.saveDealResult({ deal, metadata, extraction: { keyQuestions: ["Saved question?"] },
      files: [{ filename: "sow.pdf", path: "sow.pdf", relativePath: "sow.pdf", sizeBytes: 10 }],
    }, "sow.pdf"));
    act(() => result.current.saveDealResult({ deal, metadata: { ...metadata, sowLink: "https://example.com/new-sow" },
      extraction: { keyQuestions: [] }, files: [],
    }));
    const saved = result.current.resource;
    if (saved.status !== "success") throw new Error("expected loaded deals");
    expect(saved.deals[0]?.room.keyQuestions).toEqual(["Saved question?"]);
    expect(saved.deals[0]?.room.resources.find((r) => r.id === "sow")).toMatchObject({
      availability: "available", sourceName: "sow.pdf", href: "https://example.com/new-sow",
    });
  });

  it.each([false, true])("preserves company edits across a pending list (saved aggregate: %s)", async (saveAggregate) => {
    const request = deferred<PersistedDeal[]>();
    listDeals.mockReturnValueOnce(request.promise);
    const party = testParty("Old company", "target");
    const deal = testDeal({ parties: [party] });
    const other = testDeal({ dealId: "DEAL-OTHER", parties: [party] });
    const unrelated = testDeal({ dealId: "DEAL-UNRELATED" });
    const { result } = renderHook(() => useWorkspaceDeals("api"));
    if (saveAggregate) {
      act(() => result.current.saveDealResult({ deal, metadata: testMetadata() }));
    }
    act(() => result.current.saveCompanyResult({ ...party.company, name: "Renamed company" }));
    if (saveAggregate) {
      const resource = result.current.resource;
      expect(resource.status === "success" && resource.deals[0]?.room.summary).toContain("Renamed company");
    }
    await act(async () => {
      request.resolve([deal, other, unrelated].map((entry) => ({ ...entry, metadata: null })));
      await request.promise;
    });
    const resource = result.current.resource;
    expect(resource.status).toBe("success");
    if (resource.status !== "success") throw new Error("expected loaded deals");
    expect(resource.deals).toHaveLength(3);
    for (const linked of resource.deals.filter((entry) => entry.room.id !== unrelated.dealId)) {
      expect(linked.persisted?.parties[0]?.company.name).toBe("Renamed company");
      expect(linked.room.summary).toContain("Renamed company");
    }
    expect(resource.deals.find((entry) => entry.room.id === unrelated.dealId)?.persisted).toEqual({ ...unrelated, metadata: null });
  });

  it("ignores an API completion after the selected data source changes", async () => {
    const request = deferred<[]>();
    listDeals.mockReturnValue(request.promise);
    const { rerender, result } = renderHook(
      ({ source }: { source: string }) => useWorkspaceDeals(source),
      { initialProps: { source: "api" } },
    );

    rerender({ source: "demo" });
    await waitFor(() => expect(result.current.resource.status).toBe("success"));
    request.resolve([]);
    await act(async () => request.promise);

    expect(result.current.resource.status === "success" && result.current.resource.source).toBe("demo");
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

it("keeps a saved aggregate when an older list finishes, while retaining other deals", async () => {
  const request = deferred<import("@/contracts/quarryApi").PersistedDeal[]>();
  listDeals.mockReturnValueOnce(request.promise);
  const { testDeal, testMetadata } = await import("@/test/fixtures/deals");
  const older = testDeal({ dealName: "Old" });
  const saved = testDeal({ dealName: "Edited" });
  const other = testDeal({ dealId: "DEAL-OTHER" });
  const { result } = renderHook(() => useWorkspaceDeals("api"));
  act(() => result.current.saveDealResult({ deal: saved, metadata: testMetadata() }));
  request.resolve([{ ...older, metadata: null }, { ...other, metadata: null }]);
  await waitFor(() => {
    const resource = result.current.resource;
    expect(resource.status === "success" && resource.deals.map((d) => d.room.name)).toEqual(["Edited", "Project Alpha"]);
  });
});
