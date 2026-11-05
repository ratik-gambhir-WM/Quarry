import type { DocumentPreviewResponse, RawTextState } from "./dataRoomPreview";

export const DOCUMENT_PREVIEW_CACHE_BYTES = 64 * 1024 * 1024;

export type CachedDocumentPreview = {
  response: DocumentPreviewResponse;
  rawText: RawTextState;
};

/** Session-local LRU; the active viewer owns its payload independently of this budget. */
export class DocumentPreviewCache {
  private entries = new Map<
    string,
    { value: CachedDocumentPreview; bytes: number }
  >();
  private bytes = 0;

  constructor(private readonly limit = DOCUMENT_PREVIEW_CACHE_BYTES) {}

  get(id: string) {
    const entry = this.entries.get(id);
    if (!entry) return undefined;
    this.entries.delete(id);
    this.entries.set(id, entry);
    return entry.value;
  }

  set(id: string, value: CachedDocumentPreview) {
    const { response, rawText } = value;
    // JS strings are budgeted conservatively at two bytes per UTF-16 code unit.
    const bytes =
      (response.pdfBytes?.byteLength ?? (response.pdfBase64?.length ?? 0) * 2) +
      (rawText.status === "ready" ? rawText.response.text.length * 2 : 0);
    this.delete(id);
    if (bytes > this.limit) return;
    while (this.bytes + bytes > this.limit) {
      const oldest = this.entries.keys().next().value;
      if (oldest === undefined) break;
      this.delete(oldest);
    }
    this.entries.set(id, { value, bytes });
    this.bytes += bytes;
  }

  delete(id: string) {
    this.bytes -= this.entries.get(id)?.bytes ?? 0;
    this.entries.delete(id);
  }

  clear() {
    this.entries.clear();
    this.bytes = 0;
  }
}
