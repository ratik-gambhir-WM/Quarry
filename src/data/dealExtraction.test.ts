import { describe, expect, it } from "vitest";
import { buildWorkspaceDealFromPersisted } from "./dealExtraction";

describe("buildWorkspaceDealFromPersisted", () => {
  it("restores key questions and legacy thesis from persisted metadata", () => {
    const deal = buildWorkspaceDealFromPersisted(
      {
        buyerOrPlatformCompany: null,
        carveOutBusiness: null,
        createdAt: "2026-08-09T00:00:00Z",
        dealName: "Project Persisted",
        dealType: "Sell-side",
        id: 9,
        mainDataRoomFolder: "/private/data-room",
        parentOrSellerCompany: null,
        peFirm: "West Monroe Capital",
        status: "active",
        targetCompany: "Target Co",
        updatedAt: "2026-08-09T00:00:00Z",
      },
      {
        createdAt: "2026-08-09T00:00:00Z",
        dataRoomSizeBytes: 1_048_576,
        dealId: 9,
        documentCount: 3,
        id: 2,
        legacyInvestmentThesis: "Preserved native context",
        keyQuestionsJson: '["Question one?",42,"Question two?"]',
        updatedAt: "2026-08-09T00:00:00Z",
      },
    );

    expect(deal.room.id).toBe("9");
    expect(deal.room.keyQuestions).toEqual(["Question one?", "Question two?"]);
    expect(deal.room.thesis).toBe("Preserved native context");
    expect(deal.room.metrics[0]).toEqual({ label: "Files Analyzed", value: "3" });
  });
});
