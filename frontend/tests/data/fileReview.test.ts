import { describe, expect, it } from "vitest";
import { buildFileReviewRows, type FileReviewSummary } from "@/data/fileReview";

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
  it.each([undefined, "", "not-a-date"])("leaves unknown or invalid added date %s unavailable", (addedAt) => {
    const [row] = buildFileReviewRows([{
      folderPath: [], source: "local", node: { id: "file", kind: "doc", name: "notes.docx", addedAt },
    }], [summary]);
    expect(row.addedAt).toBeUndefined();
  });
  it.each([undefined, null, -1, NaN, Infinity, 1.5])("leaves unavailable or invalid size %s unknown", (sizeBytes) => {
    const [row] = buildFileReviewRows([{
      folderPath: [], source: "local", node: { id: "file", kind: "doc", name: "notes.docx", sizeBytes },
    }], [summary]);
    expect(row.sizeBytes).toBeUndefined();
  });

  it.each([0, 2048])("preserves known size %s including empty files", (sizeBytes) => {
    const [row] = buildFileReviewRows([{
      folderPath: [], source: "local", node: { id: "file", kind: "doc", name: "notes.docx", sizeBytes },
    }], [summary]);
    expect(row.sizeBytes).toBe(sizeBytes);
  });

  it("combines runtime file identity with clearly separate illustrative review content", () => {
    const node = {
      id: "file-1",
      kind: "sheet" as const,
      name: "model.xlsx",
      relativePath: "Finance/model.xlsx",
      addedAt: "2026-11-15T12:00:00Z",
    };
    const [row] = buildFileReviewRows(
      [{ folderPath: ["Finance"], node, source: "local" }],
      [summary],
    );

    expect(row).toMatchObject({
      fileName: "model.xlsx",
      addedAt: "2026-11-15T12:00:00Z",
      fileType: "Spreadsheet",
      folderLabel: "Finance",
      id: "file-1",
      keyFinding: "Growth is resilient",
      node,
      source: "local",
    });
  });
});
