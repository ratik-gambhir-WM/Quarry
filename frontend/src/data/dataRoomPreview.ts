import type { DataRoomTreeNode } from "./dataRoom";
import type { DealDocumentText } from "../contracts/quarryApi";

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

export type PreviewState =
  | { status: "loading" }
  | { message: string; status: "error" }
  | { response: DocumentPreviewResponse; status: "ready" };

export type RawTextState =
  | { status: "idle" }
  | { status: "loading" }
  | { message: string; status: "error" }
  | { response: DealDocumentText; status: "ready" };

export function isDocumentPreviewResponse(value: unknown): value is DocumentPreviewResponse {
  if (!value || typeof value !== "object") {
    return false;
  }

  const response = value as Partial<DocumentPreviewResponse>;
  const hasPdfBytes = response.pdfBytes instanceof Uint8Array && response.pdfBytes.byteLength > 0;
  const hasPdfBase64 = typeof response.pdfBase64 === "string" && response.pdfBase64.length > 0;
  return (
    typeof response.fileName === "string"
    && response.mimeType === "application/pdf"
    && (hasPdfBytes || hasPdfBase64)
    && typeof response.sourceKind === "string"
  );
}
