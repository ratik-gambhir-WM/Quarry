import type { DataRoomTreeNode } from "./dataRoom";

export type DealDataRoom = {
  dealId: string;
  rootName: string;
  rootPath: string;
  tree: DataRoomTreeNode[];
};

type DocumentPreviewMetadata = {
  fileName: string;
  mimeType: "application/pdf";
  sourceKind: "native" | "stored" | `converted-from-${"docx" | "xlsx" | "pptx"}`;
};

export type DocumentPreviewResponse = DocumentPreviewMetadata &
  (
    | { pdfBase64: string; pdfBytes?: never }
    | { pdfBase64?: never; pdfBytes: Uint8Array }
  );
