import { describe, expect, it } from "vitest";
import { isDocumentPreviewResponse } from "@/data/dataRoomPreview";

describe("isDocumentPreviewResponse", () => {
  it("accepts non-empty byte and base64 PDF responses", () => {
    expect(
      isDocumentPreviewResponse({
        fileName: "report.pdf",
        mimeType: "application/pdf",
        pdfBytes: new Uint8Array([1]),
        sourceKind: "native",
      }),
    ).toBe(true);
    expect(
      isDocumentPreviewResponse({
        fileName: "report.pdf",
        mimeType: "application/pdf",
        pdfBase64: "JVBERi0=",
        sourceKind: "stored",
      }),
    ).toBe(true);
  });

  it("rejects empty payloads and non-PDF metadata", () => {
    expect(
      isDocumentPreviewResponse({
        fileName: "report.pdf",
        mimeType: "application/pdf",
        pdfBytes: new Uint8Array(),
        sourceKind: "native",
      }),
    ).toBe(false);
    expect(
      isDocumentPreviewResponse({
        fileName: "report.txt",
        mimeType: "text/plain",
        pdfBase64: "content",
        sourceKind: "native",
      }),
    ).toBe(false);
  });
});
