import type { DataRoomTreeNode } from "./dataRoom";

export type DealDataRoom = {
  dealId: string;
  rootName: string;
  rootPath: string;
  tree: DataRoomTreeNode[];
};

export type DocumentPreviewResponse = {
  fileName: string;
  mimeType: "application/pdf";
  pdfBase64: string;
  sourceKind: "native" | `converted-from-${"docx" | "xlsx" | "pptx"}`;
};
