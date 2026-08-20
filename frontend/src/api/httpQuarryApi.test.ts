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
});
