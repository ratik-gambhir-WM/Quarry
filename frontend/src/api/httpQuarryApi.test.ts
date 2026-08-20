import { afterEach, describe, expect, it, vi } from "vitest";
import { httpQuarryApi } from "./httpQuarryApi";

describe("httpQuarryApi", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uses the versioned API contract", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response("[]", {
        headers: { "content-type": "application/json" },
        status: 200,
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await httpQuarryApi.listDeals();

    expect(fetchMock).toHaveBeenCalledWith("/api/v1/deals", undefined);
  });
});
