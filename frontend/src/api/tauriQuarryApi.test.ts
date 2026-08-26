import { describe, expect, it, vi } from "vitest";
import { createTauriQuarryApi } from "./tauriQuarryApi";

describe("createTauriQuarryApi", () => {
  it("sends multipart document uploads to the deal-scoped path", async () => {
    const postMultipart = vi.fn().mockResolvedValue({
      documents: [],
      failed: 0,
      skipped: 0,
      succeeded: 0,
      total: 0,
    });
    const api = createTauriQuarryApi({
      get: vi.fn(),
      post: vi.fn(),
      postMultipart,
      subscribeJob: vi.fn(),
    });

    await api.processDocuments("DEAL / 1", " analyst@example.com ", []);

    expect(postMultipart).toHaveBeenCalledWith({
      fields: [{ name: "userId", value: "analyst@example.com" }],
      files: [],
      path: "/api/v1/deals/DEAL%20%2F%201/documents/process",
    });
  });
});
