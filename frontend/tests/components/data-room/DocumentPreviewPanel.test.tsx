// @vitest-environment happy-dom

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef, type ForwardedRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  PDFEditorHandle,
  PDFEditorProps,
} from "@/components/extend/pdf-editor";
import {
  DocumentPreviewPanel,
  type DocumentPreviewPanelHandle,
  type PreviewState,
  type RawTextState,
} from "@/components/data-room/DocumentPreviewPanel";

const pdfMock = vi.hoisted(() => ({
  scrollToPage: vi.fn(),
  importAnnotations: vi.fn(),
  mounts: 0,
  unmounts: 0,
}));

vi.mock("@/components/extend/pdf-editor", async () => {
  const React = await import("react");

  const PDFEditor = React.forwardRef(function MockPDFEditor(
    props: PDFEditorProps,
    ref: ForwardedRef<PDFEditorHandle>,
  ) {
    const viewportRef = React.useRef<HTMLDivElement>(null);
    React.useImperativeHandle(
      ref,
      (): PDFEditorHandle => ({
        applyRedactions: vi.fn(),
        download: vi.fn(),
        exportAnnotations: vi.fn().mockResolvedValue([]),
        getDocumentBuffer: vi.fn().mockResolvedValue(new ArrayBuffer(0)),
        getFormValues: vi.fn().mockReturnValue({}),
        getViewportElement: () => viewportRef.current,
        importAnnotations: pdfMock.importAnnotations,
        print: vi.fn(),
        redo: vi.fn(),
        scrollToPage: pdfMock.scrollToPage,
        setActiveTool: vi.fn(),
        setFormValues: vi.fn(),
        setMode: vi.fn(),
        undo: vi.fn(),
      }),
      [],
    );
    React.useEffect(() => {
      pdfMock.mounts += 1;
      return () => {
        pdfMock.unmounts += 1;
      };
    }, []);
    React.useEffect(() => {
      props.onDocumentLoadSuccess?.({
        documentId: "mock-pdf",
        fileName: props.fileName ?? "document.pdf",
        numPages: 3,
      });
    }, []);

    return (
      <div data-slot="pdf-editor" data-testid="pdf-editor">
        <div ref={viewportRef} tabIndex={0}>PDF canvas</div>
        {props.toolbarActions}
        <button
          onClick={() => props.onAnnotationsChange?.([{ id: "annotation-1" }] as never)}
          type="button"
        >
          Add annotation
        </button>
      </div>
    );
  });

  return { PDFEditor };
});

const document = {
  id: "synthetic-document",
  kind: "pdf" as const,
  name: "Synthetic_Terms.pdf",
  relativePath: "Synthetic_Terms.pdf",
  storedFileId: "synthetic-file-id",
};

const preview = {
  response: {
    fileName: document.name,
    mimeType: "application/pdf" as const,
    pdfBytes: new Uint8Array([37, 80, 68, 70, 45, 49]),
    sourceKind: "stored" as const,
  },
  status: "ready" as const,
};

function renderPreview(
  rawText: RawTextState = { status: "idle" },
  previewState: PreviewState = preview,
) {
  const onRequestRawText = vi.fn();
  const onPageCountChange = vi.fn();
  const onRequestedPageHandled = vi.fn();
  const previewRef = createRef<DocumentPreviewPanelHandle>();
  let activePreview = previewState;
  const renderPanel = (requestedPage: number | null, nextPreview = activePreview) => {
    activePreview = nextPreview;
    return (
      <DocumentPreviewPanel
        document={document}
        onClose={vi.fn()}
        onPageCountChange={onPageCountChange}
        onRequestRawText={onRequestRawText}
        onRequestedPageHandled={onRequestedPageHandled}
        preview={activePreview}
        rawText={rawText}
        ref={previewRef}
        requestedPage={requestedPage}
      />
    );
  };
  const view = render(renderPanel(null));
  return {
    ...view,
    onPageCountChange,
    onRequestRawText,
    onRequestedPageHandled,
    previewRef,
    resolvePreview() {
      view.rerender(renderPanel(null, preview));
    },
    requestPage(page: number) {
      view.rerender(renderPanel(page));
    },
  };
}

beforeEach(() => {
  pdfMock.scrollToPage.mockReset();
  pdfMock.importAnnotations.mockReset();
  pdfMock.mounts = 0;
  pdfMock.unmounts = 0;
});

afterEach(cleanup);

describe("DocumentPreviewPanel navigation contract", () => {
  it("shows the document header and loading state while preview bytes load", () => {
    const { container, onPageCountChange } = renderPreview(
      { status: "idle" },
      { status: "loading" },
    );

    expect(screen.getByText(document.name)).toBeTruthy();
    expect(screen.getByRole("status").textContent).toContain("Loading document preview");
    expect(screen.getByRole("button", { name: "Close document preview" }).hasAttribute("disabled"))
      .toBe(false);
    expect(container.querySelector("section > header")).toBeTruthy();
    expect(screen.queryByTestId("pdf-editor")).toBeNull();
    expect(onPageCountChange).toHaveBeenCalledWith(0);
  });

  it("mounts the editor when preview bytes resolve", () => {
    const { resolvePreview } = renderPreview(
      { status: "idle" },
      { status: "loading" },
    );

    resolvePreview();

    expect(screen.getByTestId("pdf-editor")).toBeTruthy();
    expect(pdfMock.mounts).toBe(1);
    expect(pdfMock.unmounts).toBe(0);
  });

  it("applies an externally requested page exactly once without remounting", async () => {
    const { container, onRequestRawText, onRequestedPageHandled, requestPage } = renderPreview();
    const viewer = container.querySelector("[data-testid='pdf-editor']");

    requestPage(1);

    expect(container.querySelector("[data-testid='pdf-editor']")).toBe(viewer);
    await waitFor(() => expect(pdfMock.scrollToPage).toHaveBeenCalledTimes(1));
    expect(pdfMock.scrollToPage).toHaveBeenCalledWith(1);
    expect(onRequestedPageHandled).toHaveBeenCalledTimes(1);
    expect(pdfMock.mounts).toBe(1);
    expect(pdfMock.unmounts).toBe(0);
    expect(onRequestRawText).not.toHaveBeenCalled();
  });

  it("reports the loaded page count through its reusable preview contract", () => {
    const { onPageCountChange } = renderPreview();

    expect(onPageCountChange).toHaveBeenCalledWith(3);
  });

  it("returns from raw text before applying an externally requested page", async () => {
    const user = userEvent.setup();
    const rawText: RawTextState = {
      response: {
        fileName: document.name,
        sourceKind: "pdf",
        text: "Synthetic extracted text.",
      },
      status: "ready",
    };
    const { onRequestRawText, requestPage } = renderPreview(rawText);
    await user.click(screen.getByRole("button", { name: "Show raw text" }));
    expect(screen.getByText("Synthetic extracted text.")).toBeTruthy();

    requestPage(1);

    await waitFor(() => expect(screen.getByTestId("pdf-editor")).toBeTruthy());
    expect(pdfMock.scrollToPage).toHaveBeenCalledTimes(1);
    expect(pdfMock.scrollToPage).toHaveBeenCalledWith(1);
    expect(pdfMock.mounts).toBe(1);
    expect(pdfMock.unmounts).toBe(0);
    expect(onRequestRawText).not.toHaveBeenCalled();
  });

  it("restores annotations from memory when the same document is reopened", async () => {
    const user = userEvent.setup();
    const firstView = renderPreview();

    await user.click(screen.getByRole("button", { name: "Add annotation" }));
    firstView.unmount();
    renderPreview();

    await waitFor(() => expect(pdfMock.importAnnotations).toHaveBeenCalledTimes(1));
    expect(pdfMock.importAnnotations.mock.calls[0]?.[0]).toEqual([{ id: "annotation-1" }]);
  });

  it("exposes viewer focus without exposing the PDF viewer implementation", async () => {
    const { previewRef } = renderPreview();

    act(() => previewRef.current?.focusViewer());

    const canvas = screen.getByText("PDF canvas");
    await waitFor(() => expect(canvas).toBe(canvas.ownerDocument.activeElement));
  });
});
