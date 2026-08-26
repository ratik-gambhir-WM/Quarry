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
      getPdf: vi.fn(),
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

  it("uses the deal-scoped document routes for lists, PDF bytes, and raw text", async () => {
    const get = vi
      .fn()
      .mockResolvedValueOnce([{ displayName: "Report.pdf", fileId: "file / 1" }])
      .mockResolvedValueOnce({
        fileName: "Report.pdf",
        sourceKind: "pdf",
        text: "Raw report text",
      });
    const getPdf = vi.fn().mockResolvedValue(new Uint8Array([37, 80, 68, 70, 45]).buffer);
    const api = createTauriQuarryApi({
      get,
      getPdf,
      post: vi.fn(),
      postMultipart: vi.fn(),
      subscribeJob: vi.fn(),
    });

    const documents = await api.listDealDocuments("DEAL / 1");
    const pdf = await api.getDealDocumentPdf("DEAL / 1", documents[0].fileId);
    const rawText = await api.getDealDocumentText("DEAL / 1", documents[0].fileId);

    expect(get).toHaveBeenCalledWith("/api/v1/deals/DEAL%20%2F%201/documents");
    expect(getPdf).toHaveBeenCalledWith(
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/pdf",
    );
    expect(Array.from(pdf.bytes)).toEqual([37, 80, 68, 70, 45]);
    expect(get).toHaveBeenNthCalledWith(
      2,
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/text",
    );
    expect(rawText.text).toBe("Raw report text");
  });
});
