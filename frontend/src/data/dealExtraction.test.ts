import { describe, expect, it } from "vitest";
import { buildWorkspaceDealFromPersisted, type SavedDeal } from "./dealExtraction";

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
  });
});
