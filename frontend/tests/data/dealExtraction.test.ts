import { describe, expect, it } from "vitest";
import {
  buildDealResources,
  buildWorkspaceDealFromExtractionResult,
  buildWorkspaceDealFromPersisted,
  type SaveDealMetadataResponse,
  type SavedDeal,
  type SavedDealMetadata,
} from "@/data/dealExtraction";

const savedDeal: SavedDeal = {
  closeDate: "2026-09-12",
  dealId: "DEAL-123",
  dealName: "Acme",
  dealSponsor: "Thoma Bravo",
  primaryBuyer: "Cvs",
  startDate: "2026-08-03",
  status: "Active",
  targetCompany: "Target",
  transactionType: "Acquisition",
  userId: 1,
};

const savedMetadata: SavedDealMetadata = {
  dealId: savedDeal.dealId,
  factSheetLink: "https://example.com/fact-sheet",
  keyQuestionsJson: '["First question?","Second question?","Second question?"]',
  localPath: null,
  rlLink: "https://example.com/request-list",
  sharepointLink: "https://northwind.sharepoint.com/sites/acme",
  sowLink: "https://example.com/sow",
  userId: savedDeal.userId,
};

describe("buildWorkspaceDealFromPersisted", () => {
  it("builds a company-focused diligence description from the deal details", () => {
    const workspaceDeal = buildWorkspaceDealFromPersisted(savedDeal, null);

    expect(workspaceDeal.room.summary).toContain(
      "Target is the target company in this acquisition opportunity for Cvs",
    );
    expect(workspaceDeal.room.summary).toContain(
      "Thoma Bravo serving as the deal sponsor",
    );
    expect(workspaceDeal.room.summary).toContain(
      "diligence materials, findings, and open questions",
    );
    expect(workspaceDeal.portfolio).toEqual({
      closeDate: "2026-09-12",
      dealSponsor: "Thoma Bravo",
      primaryBuyer: "Cvs",
      startDate: "2026-08-03",
      status: "Active",
      targetCompany: "Target",
      transactionType: "Acquisition",
    });
  });

  it.each(["Closed", " completed "])("normalizes %s as complete", (status) => {
    const workspaceDeal = buildWorkspaceDealFromPersisted({ ...savedDeal, status }, null);

    expect(workspaceDeal.complete).toBe(true);
  });

  it("preserves extracted question order and fresh SOW metadata", () => {
    const result: SaveDealMetadataResponse = {
      deal: savedDeal,
      extraction: { keyQuestions: ["First question?", "Second question?", "Second question?"] },
      files: [
        {
          filename: "Acme SOW.pdf",
          path: "Acme SOW.pdf",
          relativePath: "Acme SOW.pdf",
          sizeBytes: 1200,
        },
      ],
      metadata: savedMetadata,
    };

    const workspaceDeal = buildWorkspaceDealFromExtractionResult(result, "Acme SOW.pdf");

    expect(workspaceDeal.room.keyQuestions).toEqual([
      "First question?",
      "Second question?",
      "Second question?",
    ]);
    expect(workspaceDeal.room.resources).toEqual([
      {
        availability: "available",
        href: "https://example.com/sow",
        id: "sow",
        label: "SOW",
        sourceName: "Acme SOW.pdf",
      },
      {
        availability: "available",
        href: "https://example.com/fact-sheet",
        id: "fact-sheet",
        label: "Fact Sheet",
      },
      {
        availability: "available",
        href: "https://northwind.sharepoint.com/sites/acme",
        id: "sharepoint",
        label: "SharePoint VDR",
      },
      {
        availability: "available",
        href: "https://example.com/request-list",
        id: "request-list",
        label: "Request List",
      },
    ]);
  });

  it("maps persisted questions and saved resource links after reload", () => {
    const workspaceDeal = buildWorkspaceDealFromPersisted(savedDeal, savedMetadata);

    expect(workspaceDeal.room.keyQuestions).toEqual([
      "First question?",
      "Second question?",
      "Second question?",
    ]);
    expect(workspaceDeal.room.resources[0]).toEqual({
      availability: "available",
      href: "https://example.com/sow",
      id: "sow",
      label: "SOW",
    });
    expect(workspaceDeal.room.resources[3]).toEqual({
      availability: "available",
      href: "https://example.com/request-list",
      id: "request-list",
      label: "Request List",
    });
  });

  it("keeps missing or unsafe optional resource values inactive", () => {
    expect(buildDealResources(null).map((resource) => resource.availability)).toEqual([
      "unavailable",
      "unavailable",
      "unavailable",
      "unavailable",
    ]);

    for (const sharepointLink of [
      "http://northwind.sharepoint.com/sites/acme",
      "https://example.com/?next=.sharepoint.com/",
      "not a URL",
    ]) {
      const resources = buildDealResources({ ...savedMetadata, sharepointLink });
      expect(resources[2]).toEqual({
        availability: "unavailable",
        href: undefined,
        id: "sharepoint",
        label: "SharePoint VDR",
      });
    }

    for (const unsafeLink of [
      "http://example.com/resource",
      "javascript:alert(1)",
      "https://user:password@example.com/resource",
      "not a URL",
    ]) {
      const resources = buildDealResources({
        ...savedMetadata,
        factSheetLink: unsafeLink,
        rlLink: unsafeLink,
        sowLink: unsafeLink,
      });
      expect(resources[0].availability).toBe("unavailable");
      expect(resources[1].availability).toBe("unavailable");
      expect(resources[3].availability).toBe("unavailable");
    }
  });
});
