// @vitest-environment happy-dom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DataRoomTreeNode } from "@/data/dataRoom";
import type { DocumentPreviewResponse } from "@/data/dataRoomPreview";
import { useDocumentSession } from "@/hooks/useDocumentSession";

const { getDealDocumentPdf, getDealDocumentText, previewDealDocument } = vi.hoisted(() => ({
  getDealDocumentPdf: vi.fn(),
  getDealDocumentText: vi.fn(),
  previewDealDocument: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { getDealDocumentPdf, getDealDocumentText, previewDealDocument } },
}));

const localDocument: DataRoomTreeNode = {
  id: "local:file.pdf",
  kind: "pdf",
  name: "file.pdf",
  relativePath: "file.pdf",
};

afterEach(cleanup);

describe("useDocumentSession", () => {
  beforeEach(() => {
    getDealDocumentPdf.mockReset();
    getDealDocumentText.mockReset();
    previewDealDocument.mockReset();
  });

  it("releases the preview response on close and ignores a stale completion", async () => {
    const request = deferred<DocumentPreviewResponse>();
    previewDealDocument.mockReturnValue(request.promise);
    const { result } = renderHook(() => useDocumentSession("deal-1"));

    act(() => void result.current.selectDocument(localDocument));
    expect(result.current.state.status).toBe("open");
    act(() => result.current.closeDocument());
    expect(result.current.state).toEqual({ status: "closed" });

    request.resolve({
      fileName: "file.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0xLjQ=",
      sourceKind: "native",
    });
    await act(async () => request.promise);
    expect(result.current.state).toEqual({ status: "closed" });
  });

  it("loads stored PDF bytes and raw text for the current document", async () => {
    getDealDocumentPdf.mockResolvedValue({ bytes: new Uint8Array([1]), mimeType: "application/pdf" });
    getDealDocumentText.mockResolvedValue({ sourceKind: "pdf", text: "Extracted text" });
    const storedDocument = { ...localDocument, id: "stored:1", storedFileId: "file-1" };
    const { result } = renderHook(() => useDocumentSession("deal-1"));

    await act(async () => result.current.selectDocument(storedDocument));
    await waitFor(() => expect(result.current.state.status === "open" && result.current.state.preview.status).toBe("ready"));
    await act(async () => result.current.requestRawText());

    expect(getDealDocumentPdf).toHaveBeenCalledWith("deal-1", "file-1");
    expect(getDealDocumentText).toHaveBeenCalledWith("deal-1", "file-1");
    expect(result.current.state.status === "open" && result.current.state.rawText.status).toBe("ready");
  });

  it("keeps preview and text failures scoped to the current document", async () => {
    previewDealDocument.mockRejectedValue(new Error("Preview unavailable"));
    getDealDocumentPdf.mockResolvedValue({ bytes: new Uint8Array([1]), mimeType: "application/pdf" });
    getDealDocumentText.mockRejectedValue(new Error("Text unavailable"));
    const { result } = renderHook(() => useDocumentSession("deal-1"));

    await act(async () => result.current.selectDocument(localDocument));
    expect(result.current.state.status === "open" && result.current.state.preview).toEqual({
      message: "Preview unavailable",
      status: "error",
    });

    const storedDocument = { ...localDocument, id: "stored:1", storedFileId: "file-1" };
    await act(async () => result.current.selectDocument(storedDocument));
    await act(async () => result.current.requestRawText());
    expect(result.current.state.status === "open" && result.current.state.rawText).toEqual({
      message: "Text unavailable",
      status: "error",
    });
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
