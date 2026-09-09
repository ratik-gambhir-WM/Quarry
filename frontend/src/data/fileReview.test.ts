import { describe, expect, it } from "vitest";
import { buildFileReviewRows, type FileReviewSummary } from "./fileReview";

const summary: FileReviewSummary = {
  impact: "Supports the thesis",
  impactLevel: "high",
  keyFinding: "Growth is resilient",
  opportunities: "Cross-sell",
  opportunityLevel: "medium",
  risks: "Concentration",
  riskLevel: "high",
  status: "Needs attention",
};

describe("buildFileReviewRows", () => {
  it("combines runtime file identity with clearly separate illustrative review content", () => {
    const node = { id: "file-1", kind: "sheet" as const, name: "model.xlsx", relativePath: "Finance/model.xlsx" };
    const [row] = buildFileReviewRows(
      [{ folderPath: ["Finance"], node, source: "local" }],
      [summary],
    );

    expect(row).toMatchObject({
      fileName: "model.xlsx",
      fileType: "Spreadsheet",
      folderLabel: "Finance",
      id: "file-1",
      keyFinding: "Growth is resilient",
      node,
      source: "local",
    });
  });
});
