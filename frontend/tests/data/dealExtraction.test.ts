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
  keyQuestionsJson: '["First question?","Second question?","Second question?"]',
  localPath: null,
  sharepointLink: "https://northwind.sharepoint.com/sites/acme",
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
        id: "sow",
        label: "SOW",
        sourceName: "Acme SOW.pdf",
      },
      { availability: "coming-soon", id: "fact-sheet", label: "Fact Sheet" },
      {
        availability: "available",
        href: "https://northwind.sharepoint.com/sites/acme",
        id: "sharepoint",
        label: "SharePoint VDR",
      },
    ]);
  });

  it("maps persisted questions identically without claiming a reload-safe SOW", () => {
    const workspaceDeal = buildWorkspaceDealFromPersisted(savedDeal, savedMetadata);

    expect(workspaceDeal.room.keyQuestions).toEqual([
      "First question?",
      "Second question?",
      "Second question?",
    ]);
    expect(workspaceDeal.room.resources[0]).toEqual({
      availability: "unavailable",
      id: "sow",
      label: "SOW",
    });
  });

  it("keeps missing or unsafe optional resource values inactive", () => {
    expect(buildDealResources(null).map((resource) => resource.availability)).toEqual([
      "unavailable",
      "coming-soon",
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
  });
});
