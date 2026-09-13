// @vitest-environment happy-dom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getWorkspaceDataSource, useWorkspaceDeals } from "@/hooks/useWorkspaceDeals";

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
