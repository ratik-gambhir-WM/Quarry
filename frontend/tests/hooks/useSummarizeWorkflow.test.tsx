// @vitest-environment happy-dom

import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSummarizeWorkflow } from "@/hooks/useSummarizeWorkflow";

const { summarizePath, summarizeSelected, summarizeUpload } = vi.hoisted(() => ({
  summarizePath: vi.fn(),
  summarizeSelected: vi.fn(),
  summarizeUpload: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { summarizePath, summarizeSelected, summarizeUpload } },
}));

afterEach(cleanup);

describe("useSummarizeWorkflow", () => {
  beforeEach(() => {
    summarizePath.mockReset();
    summarizeSelected.mockReset();
    summarizeUpload.mockReset();
  });

  it("maps a manual path to the path API and exposes successful output", async () => {
    summarizePath.mockResolvedValue("# Summary");
    const { result } = renderHook(() => useSummarizeWorkflow());
    act(() => result.current.updateManualPath("/synthetic/report.pdf"));
    await act(async () => result.current.summarize());

    expect(summarizePath).toHaveBeenCalledWith("/synthetic/report.pdf");
    expect(result.current.request).toEqual({ status: "success", summary: "# Summary" });
  });

  it("does not let an older completion overwrite a newer selection", async () => {
    const request = deferred<string>();
    summarizePath.mockReturnValue(request.promise);
    const { result } = renderHook(() => useSummarizeWorkflow());
    act(() => result.current.updateManualPath("/synthetic/old.pdf"));
    act(() => void result.current.summarize());
    expect(result.current.request.status).toBe("submitting");

    act(() => result.current.updateManualPath("/synthetic/new.pdf"));
    request.resolve("Old summary");
    await act(async () => request.promise);

    expect(result.current.request).toEqual({ status: "idle" });
    expect(result.current.selectedPath).toBe("/synthetic/new.pdf");
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
