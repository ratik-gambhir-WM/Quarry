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

  it("saves the core deal and optional metadata through two separate requests", async () => {
    const input = {
      closeDate: "2026-05-01",
      dealId: "DEAL-000184",
      dealName: "Acme acquisition of WidgetCo",
      dealSponsor: "Thoma Bravo",
      localPath: null,
      primaryBuyer: "CVS",
      sharepointLink: null,
      startDate: "2026-02-14",
      status: "Active",
      targetCompany: "WidgetCo",
      transactionType: "Acquisition",
      userEmail: "analyst@example.com",
    };
    const deal = {
      closeDate: input.closeDate,
      dealId: input.dealId,
      dealName: input.dealName,
      dealSponsor: input.dealSponsor,
      primaryBuyer: input.primaryBuyer,
      startDate: input.startDate,
      status: input.status,
      targetCompany: input.targetCompany,
      transactionType: input.transactionType,
      userId: 1,
    };
    const metadata = {
      dealId: input.dealId,
      keyQuestionsJson: "[]",
      localPath: null,
      sharepointLink: input.sharepointLink,
      userId: 1,
    };
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(Response.json({ deal, metadata }, { status: 201 }))
      .mockResolvedValueOnce(
        Response.json({ deal, extraction: { keyQuestions: [] }, files: [], metadata }),
      );
    vi.stubGlobal("fetch", fetchMock);

    await httpQuarryApi.createDeal(input);
    await httpQuarryApi.saveDealMetadata(input.dealId, []);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "/api/v1/deals",
      expect.objectContaining({ body: JSON.stringify(input), method: "POST" }),
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/v1/deals/DEAL-000184/metadata",
      expect.objectContaining({ body: expect.any(FormData), method: "POST" }),
    );
  });

  it("uses the authoritative deal path for document uploads", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      Response.json({ documents: [], failed: 0, skipped: 0, succeeded: 0, total: 0 }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await httpQuarryApi.processDocuments("DEAL / 1", " analyst@example.com ", []);

    expect(fetchMock).toHaveBeenCalledWith(
      "/api/v1/deals/DEAL%20%2F%201/documents/process",
      expect.objectContaining({ body: expect.any(FormData), method: "POST" }),
    );
    const request = fetchMock.mock.calls[0][1] as RequestInit;
    expect((request.body as FormData).get("userId")).toBe("analyst@example.com");
  });

  it("lists stored deal documents and reads PDF bytes and raw text", async () => {
    const pdfBytes = new TextEncoder().encode("%PDF-1.4\npreview");
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        Response.json([{ displayName: "Report.pdf", fileId: "file / 1" }]),
      )
      .mockResolvedValueOnce(
        new Response(pdfBytes, {
          headers: { "content-type": "application/pdf" },
          status: 200,
        }),
      )
      .mockResolvedValueOnce(
        Response.json({
          fileName: "Report.pdf",
          sourceKind: "pdf",
          text: "Raw report text",
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    const documents = await httpQuarryApi.listDealDocuments("DEAL / 1");
    const pdf = await httpQuarryApi.getDealDocumentPdf("DEAL / 1", documents[0].fileId);
    const rawText = await httpQuarryApi.getDealDocumentText("DEAL / 1", documents[0].fileId);

    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "/api/v1/deals/DEAL%20%2F%201/documents",
      undefined,
    );
    expect(fetchMock).toHaveBeenNthCalledWith(
      2,
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/pdf",
    );
    expect(pdf.mimeType).toBe("application/pdf");
    expect(Array.from(pdf.bytes)).toEqual(Array.from(pdfBytes));
    expect(fetchMock).toHaveBeenNthCalledWith(
      3,
      "/api/v1/deals/DEAL%20%2F%201/documents/file%20%2F%201/text",
      undefined,
    );
    expect(rawText.text).toBe("Raw report text");
  });
});
