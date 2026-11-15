import type { DataRoomFileEntry, DataRoomTreeNode } from "./dataRoom";
import { getFileType, type FileType } from "./fileTypes";

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
  addedAt?: string;
  fileName: string;
  fileType: FileType;
  folderLabel: string;
  id: string;
  node: DataRoomTreeNode;
  sizeBytes?: number;
  source: DataRoomFileEntry["source"];
};

export function buildFileReviewRows(
  entries: readonly DataRoomFileEntry[],
  summaries: readonly FileReviewSummary[],
): FileReviewRow[] {
  return entries.map((entry, index) => {
    const summary =
      summaries.length > 0
        ? summaries[index % summaries.length]
        : pendingSummary;
    return {
      ...summary,
      addedAt: entry.node.addedAt && Number.isFinite(Date.parse(entry.node.addedAt))
        ? entry.node.addedAt
        : undefined,
      fileName: entry.node.name,
      fileType: getFileType(entry.node.name, entry.node.kind),
      folderLabel: entry.folderPath.join(" / ") || "Data Room",
      id: entry.node.id,
      node: entry.node,
      sizeBytes:
        typeof entry.node.sizeBytes === "number" &&
        Number.isSafeInteger(entry.node.sizeBytes) && entry.node.sizeBytes >= 0
          ? entry.node.sizeBytes
          : undefined,
      source: entry.source,
    };
  });
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
