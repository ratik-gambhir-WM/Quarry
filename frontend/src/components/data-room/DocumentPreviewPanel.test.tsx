// @vitest-environment happy-dom

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef, type ForwardedRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  PdfToolbarContext,
  PdfViewerHandle,
  PdfViewerProps,
} from "../pdf-viewer";
import {
  DocumentPreviewPanel,
  type DocumentPreviewPanelHandle,
  type RawTextState,
} from "./DocumentPreviewPanel";

const pdfMock = vi.hoisted(() => ({
  goToPage: vi.fn(),
  mounts: 0,
  unmounts: 0,
}));

vi.mock("../pdf-viewer", async () => {
  const React = await import("react");

  const PdfViewer = React.forwardRef(function MockPdfViewer(
    props: PdfViewerProps,
    ref: ForwardedRef<PdfViewerHandle>,
  ) {
    React.useImperativeHandle(
      ref,
      (): PdfViewerHandle =>
        ({
          actions: {
            download: vi.fn(),
            goToNextPage: vi.fn(),
            goToPage: pdfMock.goToPage,
            goToPrevPage: vi.fn(),
            print: vi.fn(),
            resetZoom: vi.fn(),
            rotate: vi.fn(),
            setRotation: vi.fn(),
            setScale: vi.fn(),
            zoomIn: vi.fn(),
            zoomOut: vi.fn(),
          },
          pdfDocument: null,
          state: {
            error: null,
            fitMode: null,
            loading: false,
            numPages: 3,
            page: 1,
            ready: true,
            rotation: 0,
            scale: 1,
            selectedText: "",
            source: null,
            status: "ready",
          },
        }),
      [],
    );
    React.useEffect(() => {
      pdfMock.mounts += 1;
      props.onLoad?.({ numPages: 3, pdfDocument: {} as never });
      return () => {
        pdfMock.unmounts += 1;
      };
    }, []);

    return (
      <div data-pdf-viewer-root data-testid="pdf-viewer">
        <div tabIndex={0}>PDF canvas</div>
        {props.renderToolbar?.({} as PdfToolbarContext)}
      </div>
    );
  });

  return {
    PdfToolbar: ({
      leadingContent,
      onPrintAction,
      printActionLabel,
      trailingContent,
    }: {
      leadingContent?: React.ReactNode;
      onPrintAction?: () => void;
      printActionLabel?: string;
      trailingContent?: React.ReactNode;
    }) =>
      <div>
        {leadingContent}
        {onPrintAction ? (
          <button onClick={onPrintAction} type="button">
            {printActionLabel}
          </button>
        ) : null}
        {trailingContent}
      </div>,
    PdfViewer,
  };
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

function renderPreview(rawText: RawTextState = { status: "idle" }) {
  const onRequestRawText = vi.fn();
  const onPageCountChange = vi.fn();
  const onRequestedPageHandled = vi.fn();
  const previewRef = createRef<DocumentPreviewPanelHandle>();
  const renderPanel = (requestedPage: number | null) => (
    <DocumentPreviewPanel
      document={document}
      onClose={vi.fn()}
      onPageCountChange={onPageCountChange}
      onRequestRawText={onRequestRawText}
      onRequestedPageHandled={onRequestedPageHandled}
      preview={preview}
      rawText={rawText}
      ref={previewRef}
      requestedPage={requestedPage}
    />
  );
  const view = render(renderPanel(null));
  return {
    ...view,
    onPageCountChange,
    onRequestRawText,
    onRequestedPageHandled,
    previewRef,
    requestPage(page: number) {
      view.rerender(renderPanel(page));
    },
  };
}

beforeEach(() => {
  pdfMock.goToPage.mockReset();
  pdfMock.mounts = 0;
  pdfMock.unmounts = 0;
});

afterEach(cleanup);

describe("DocumentPreviewPanel navigation contract", () => {
  it("applies an externally requested page exactly once without remounting", async () => {
    const { container, onRequestRawText, onRequestedPageHandled, requestPage } = renderPreview();
    const viewer = container.querySelector("[data-testid='pdf-viewer']");

    requestPage(1);

    expect(container.querySelector("[data-testid='pdf-viewer']")).toBe(viewer);
    await waitFor(() => expect(pdfMock.goToPage).toHaveBeenCalledTimes(1));
    expect(pdfMock.goToPage).toHaveBeenCalledWith(1);
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

    await waitFor(() => expect(screen.getByTestId("pdf-viewer")).toBeTruthy());
    expect(pdfMock.goToPage).toHaveBeenCalledTimes(1);
    expect(pdfMock.goToPage).toHaveBeenCalledWith(1);
    expect(onRequestRawText).not.toHaveBeenCalled();
  });

  it("exposes viewer focus without exposing the PDF viewer implementation", async () => {
    const { previewRef } = renderPreview();

    act(() => previewRef.current?.focusViewer());

    const canvas = screen.getByText("PDF canvas");
    await waitFor(() => expect(canvas).toBe(canvas.ownerDocument.activeElement));
  });
});
