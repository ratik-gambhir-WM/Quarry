// @vitest-environment happy-dom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DataRoomTreeNode } from "@/data/dataRoom";
import type { DocumentPreviewResponse } from "@/data/dataRoomPreview";
import { useDocumentSession } from "@/hooks/useDocumentSession";

const { getDealDocumentPdf, getDealDocumentText, previewDealDocument } =
  vi.hoisted(() => ({
    getDealDocumentPdf: vi.fn(),
    getDealDocumentText: vi.fn(),
    previewDealDocument: vi.fn(),
  }));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { getDealDocumentPdf, getDealDocumentText, previewDealDocument },
  },
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
    getDealDocumentPdf.mockResolvedValue({
      bytes: new Uint8Array([1]),
      mimeType: "application/pdf",
    });
    getDealDocumentText.mockResolvedValue({
      sourceKind: "pdf",
      text: "Extracted text",
    });
    const storedDocument = {
      ...localDocument,
      id: "stored:1",
      storedFileId: "file-1",
    };
    const { result } = renderHook(() => useDocumentSession("deal-1"));

    await act(async () => result.current.selectDocument(storedDocument));
    await waitFor(() =>
      expect(
        result.current.state.status === "open" &&
          result.current.state.preview.status,
      ).toBe("ready"),
    );
    await act(async () => result.current.requestRawText());

    expect(getDealDocumentPdf).toHaveBeenCalledWith("deal-1", "file-1");
    expect(getDealDocumentText).toHaveBeenCalledWith("deal-1", "file-1");
    expect(
      result.current.state.status === "open" &&
        result.current.state.rawText.status,
    ).toBe("ready");
  });

  it("keeps preview and text failures scoped to the current document", async () => {
    previewDealDocument.mockRejectedValue(new Error("Preview unavailable"));
    getDealDocumentPdf.mockResolvedValue({
      bytes: new Uint8Array([1]),
      mimeType: "application/pdf",
    });
    getDealDocumentText.mockRejectedValue(new Error("Text unavailable"));
    const { result } = renderHook(() => useDocumentSession("deal-1"));

    await act(async () => result.current.selectDocument(localDocument));
    expect(
      result.current.state.status === "open" && result.current.state.preview,
    ).toEqual({
      message: "Preview unavailable",
      status: "error",
    });

    const storedDocument = {
      ...localDocument,
      id: "stored:1",
      storedFileId: "file-1",
    };
    await act(async () => result.current.selectDocument(storedDocument));
    await act(async () => result.current.requestRawText());
    expect(
      result.current.state.status === "open" && result.current.state.rawText,
    ).toEqual({
      message: "Text unavailable",
      status: "error",
    });
  });
  it("reuses PDF and raw-text payloads, deduplicates tabs, and reloads after close", async () => {
    getDealDocumentPdf.mockResolvedValue({
      bytes: new Uint8Array([1, 2]),
      mimeType: "application/pdf",
    });
    getDealDocumentText.mockResolvedValue({
      sourceKind: "pdf",
      text: "cached",
    });
    previewDealDocument.mockResolvedValue({
      fileName: "file.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0=",
      sourceKind: "native",
    });
    const stored = { ...localDocument, id: "stored:1", storedFileId: "file-1" };
    const { result } = renderHook(() => useDocumentSession("deal-1"));
    await act(async () => result.current.selectDocument(stored));
    await act(async () => result.current.requestRawText());
    await act(async () => result.current.selectDocument(localDocument));
    await act(async () => result.current.selectDocument(stored));
    await act(async () => result.current.requestRawText());
    expect(result.current.documents).toHaveLength(2);
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(1);
    expect(getDealDocumentText).toHaveBeenCalledTimes(1);
    act(() => result.current.closeDocument(stored.id));
    await act(async () => result.current.selectDocument(stored));
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(2);
  });

  it("evicts the least recently viewed payload by bytes while keeping its tab", async () => {
    getDealDocumentPdf.mockResolvedValue({
      bytes: new Uint8Array(4),
      mimeType: "application/pdf",
    });
    const a = { ...localDocument, id: "a", storedFileId: "a" };
    const b = { ...a, id: "b", storedFileId: "b" };
    const c = { ...a, id: "c", storedFileId: "c" };
    const { result } = renderHook(() => useDocumentSession("deal-1", 8));
    for (const document of [a, b, a, c, a]) {
      await act(async () => result.current.selectDocument(document));
    }
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(3);
    expect(result.current.documents).toHaveLength(3);
    await act(async () => result.current.selectDocument(b));
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(4);
  });

  it("budgets raw text and bypasses oversized documents without evicting hot payloads", async () => {
    getDealDocumentPdf.mockImplementation((_deal, file) =>
      Promise.resolve({
        bytes: new Uint8Array(file === "large" ? 20 : 4),
        mimeType: "application/pdf",
      }),
    );
    getDealDocumentText.mockResolvedValue({ sourceKind: "pdf", text: "12345" });
    const a = { ...localDocument, id: "a", storedFileId: "a" };
    const large = { ...a, id: "large", storedFileId: "large" };
    const { result } = renderHook(() => useDocumentSession("deal-1", 8));
    await act(async () => result.current.selectDocument(a));
    await act(async () => result.current.selectDocument(large));
    await act(async () => result.current.selectDocument(a));
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(2);
    await act(async () => result.current.requestRawText());
    act(() => result.current.showFiles());
    await act(async () => result.current.selectDocument(a));
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(3);
    await act(async () => result.current.selectDocument(large));
    expect(getDealDocumentPdf).toHaveBeenCalledTimes(4);
  });

  it("caches background completions and deduplicates in-flight requests", async () => {
    const pending = deferred<DocumentPreviewResponse>();
    previewDealDocument.mockReturnValue(pending.promise);
    const { result } = renderHook(() => useDocumentSession("deal-1"));
    act(() => void result.current.selectDocument(localDocument));
    act(() => result.current.showFiles());
    act(() => void result.current.selectDocument(localDocument));
    expect(previewDealDocument).toHaveBeenCalledTimes(1);
    act(() => result.current.showFiles());
    pending.resolve({
      fileName: "file.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0=",
      sourceKind: "native",
    });
    await act(async () => pending.promise);
    expect(result.current.state.status).toBe("closed");
    await act(async () => result.current.selectDocument(localDocument));
    expect(
      result.current.state.status === "open" &&
        result.current.state.preview.status,
    ).toBe("ready");
    expect(previewDealDocument).toHaveBeenCalledTimes(1);
  });

  it("ignores a closed tab's response after the same file is reopened", async () => {
    const old = deferred<DocumentPreviewResponse>();
    const fresh = deferred<DocumentPreviewResponse>();
    previewDealDocument
      .mockReturnValueOnce(old.promise)
      .mockReturnValueOnce(fresh.promise);
    const { result } = renderHook(() => useDocumentSession("deal-1"));
    act(() => void result.current.selectDocument(localDocument));
    act(() => result.current.closeDocument());
    act(() => void result.current.selectDocument(localDocument));
    old.resolve({
      fileName: "old.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0=",
      sourceKind: "native",
    });
    await act(async () => old.promise);
    expect(
      result.current.state.status === "open" &&
        result.current.state.preview.status,
    ).toBe("loading");
    fresh.resolve({
      fileName: "fresh.pdf",
      mimeType: "application/pdf",
      pdfBase64: "JVBERi0=",
      sourceKind: "native",
    });
    await act(async () => fresh.promise);
    expect(
      result.current.state.status === "open" &&
        result.current.state.preview.status === "ready" &&
        result.current.state.preview.response.fileName,
    ).toBe("fresh.pdf");
  });

  it("retries failed previews and selects the neighbor when closing the active tab", async () => {
    previewDealDocument
      .mockRejectedValueOnce(new Error("Failed"))
      .mockResolvedValue({
        fileName: "file.pdf",
        mimeType: "application/pdf",
        pdfBase64: "JVBERi0=",
        sourceKind: "native",
      });
    const other = { ...localDocument, id: "other", relativePath: "other.pdf" };
    const { result } = renderHook(() => useDocumentSession("deal-1"));
    await act(async () => result.current.selectDocument(localDocument));
    await act(async () => result.current.selectDocument(localDocument));
    expect(
      result.current.state.status === "open" &&
        result.current.state.preview.status,
    ).toBe("ready");
    await act(async () => result.current.selectDocument(other));
    act(() => result.current.closeDocument(localDocument.id));
    expect(
      result.current.state.status === "open" &&
        result.current.state.document.id,
    ).toBe(other.id);
    act(() => result.current.closeDocument());
    expect(result.current.state.status).toBe("closed");
    expect(result.current.documents).toHaveLength(0);
  });
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}
