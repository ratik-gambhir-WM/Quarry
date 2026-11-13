import { parseDiligenceCanvasDocument } from "../contracts/diligenceCanvas";

/** Persistence metadata stays outside the reusable canvas document. */
export function parseTemplateResponse(value: unknown) {
  if (!value || typeof value !== "object" || !("revision" in value) ||
      typeof value.revision !== "number" || !Number.isSafeInteger(value.revision) ||
      value.revision < 1 || !("document" in value)) {
    throw new Error("The server returned an invalid template revision.");
  }
  return parseDiligenceCanvasDocument(value.document);
}
