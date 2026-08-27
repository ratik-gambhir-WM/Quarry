import type { PdfSource } from "../types";

/**
 * react-pdf accepts:
 *   - string (URL or data URI)
 *   - { data: Uint8Array | ArrayBuffer }
 *   - Blob (covers File, since File extends Blob)
 *
 * We unify all of those into something the <Document file={...}> prop accepts.
 */
export type NormalizedPdfSource =
  | string
  | { data: Uint8Array | ArrayBuffer }
  | Blob;

export function normalizeSource(
  input: PdfSource | null | undefined,
): NormalizedPdfSource | null {
  if (input == null) return null;
  if (typeof input === "string") return input;
  // pdf.js transfers typed-array buffers to its worker. Always give it an
  // owned copy so downloads, retries, and the caller's source remain usable.
  if (input instanceof ArrayBuffer) return { data: input.slice(0) };
  if (input instanceof Uint8Array) return { data: new Uint8Array(input) };
  if (input instanceof Blob) return input;
  return null;
}

export function isPdfFile(file: File): boolean {
  return (
    file.type === "application/pdf" ||
    file.name.toLowerCase().endsWith(".pdf")
  );
}

export function getSourceFilename(
  source: PdfSource | null | undefined,
): string | null {
  if (source == null) return null;
  if (typeof source === "string") {
    try {
      const url = new URL(source, "http://placeholder.local");
      const name = url.pathname.split("/").pop();
      return name && name.length > 0 ? decodeURIComponent(name) : null;
    } catch {
      return null;
    }
  }
  if (typeof File !== "undefined" && source instanceof File) {
    return source.name || null;
  }
  return null;
}
