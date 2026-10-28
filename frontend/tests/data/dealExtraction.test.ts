import { testDeal, testParty } from "@/test/fixtures/deals";
import { describe, expect, it } from "vitest";
import {
  buildDealResources,
  buildWorkspaceDealFromExtractionResult,
  buildWorkspaceDealFromPersisted,
  type SaveDealMetadataResponse,
  type SavedDeal,
  type SavedDealMetadata,
} from "@/data/dealExtraction";

const savedDeal: SavedDeal = testDeal({ dealId: "DEAL-123", dealName: "Acme", startDate: "2026-08-03", expectedCloseDate: "2026-09-12", status: "prepare", transactionType: "strategic_acquisition", parties: [testParty("Target", "target"), testParty("Cvs", "buyer"), testParty("Thoma Bravo", "pe_sponsor")] });

const savedMetadata: SavedDealMetadata = {
  dealId: savedDeal.dealId,
  factSheetLink: "https://example.com/fact-sheet",
  keyQuestionsJson: '["First question?","Second question?","Second question?"]',
  localPath: null,
  rlLink: "https://example.com/request-list",
  sharepointLink: "https://northwind.sharepoint.com/sites/acme",
  sowLink: "https://example.com/sow",
};

describe("buildWorkspaceDealFromPersisted", () => {
  it("builds a company-focused diligence description from the deal details", () => {
    const workspaceDeal = buildWorkspaceDealFromPersisted(savedDeal, null);

    expect(workspaceDeal.room.summary).toContain("Target: Target");
    expect(workspaceDeal.room.summary).toContain("Buyer: Cvs");
    expect(workspaceDeal.room.summary).toContain("PE sponsor: Thoma Bravo");
    expect(workspaceDeal.portfolio.parties).toEqual(savedDeal.parties);
    expect(workspaceDeal.room.metrics).toContainEqual({ label: "Actual close", value: "Unavailable" });
  });

  it.each(["closed", "pencils_down"] as const)("marks %s as historic", (status) => {
    expect(buildWorkspaceDealFromPersisted({ ...savedDeal, status }, null).terminal).toBe(true);
  });
  it("does not infer engagement completion from a transaction close", () => {
    expect(buildWorkspaceDealFromPersisted({ ...savedDeal, actualCloseDate: "2026-09-01", status: "prepare" }, null).terminal).toBe(false);
  });
  it("describes an unknown pipeline without placeholder companies", () => {
    const workspace = buildWorkspaceDealFromPersisted(testDeal(), null);
    expect(workspace.room.summary).toContain("Companies are unspecified");
    expect(workspace.portfolio.parties).toEqual([]);
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
