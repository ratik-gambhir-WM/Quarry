import type { DataRoomFileEntry, DataRoomTreeNode } from "./dataRoom";

export type FileReviewStatus = "Needs attention" | "Reviewed" | "Pending";
export type ReviewSignalLevel = "high" | "low" | "medium" | "none";

export type FileReviewSummary = {
  impact: string;
  impactLevel: ReviewSignalLevel;
  keyFinding: string;
  opportunities: string;
  opportunityLevel: ReviewSignalLevel;
  risks: string;
  riskLevel: ReviewSignalLevel;
  status: FileReviewStatus;
};

export type FileReviewRow = FileReviewSummary & {
  fileName: string;
  fileType: "Document" | "PDF" | "Spreadsheet";
  folderLabel: string;
  id: string;
  node: DataRoomTreeNode;
  source: DataRoomFileEntry["source"];
};

export function buildFileReviewRows(
  entries: readonly DataRoomFileEntry[],
  summaries: readonly FileReviewSummary[],
): FileReviewRow[] {
  return entries.map((entry, index) => {
    const summary = summaries.length > 0 ? summaries[index % summaries.length] : pendingSummary;
    return {
      ...summary,
      fileName: entry.node.name,
      fileType: getFileType(entry.node.kind),
      folderLabel: entry.folderPath.join(" / ") || "Data Room",
      id: entry.node.id,
      node: entry.node,
      source: entry.source,
    };
  });
}

function getFileType(kind: DataRoomTreeNode["kind"]): FileReviewRow["fileType"] {
  if (kind === "pdf") return "PDF";
  if (kind === "sheet") return "Spreadsheet";
  return "Document";
}

const pendingSummary: FileReviewSummary = {
  impact: "Analysis pending",
  impactLevel: "none",
  keyFinding: "Review has not started",
  opportunities: "Analysis pending",
  opportunityLevel: "none",
  risks: "Analysis pending",
  riskLevel: "none",
  status: "Pending",
};
