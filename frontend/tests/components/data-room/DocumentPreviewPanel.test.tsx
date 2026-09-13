// @vitest-environment happy-dom

import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRef, type ForwardedRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  PdfToolbarContext,
  PdfViewerHandle,
  PdfViewerProps,
} from "@/components/pdf-viewer";
import {
  DocumentPreviewPanel,
  type DocumentPreviewPanelHandle,
  type PreviewState,
  type RawTextState,
} from "@/components/data-room/DocumentPreviewPanel";

const pdfMock = vi.hoisted(() => ({
  goToPage: vi.fn(),
  mounts: 0,
  unmounts: 0,
  pendingSource: false,
}));

vi.mock("@/components/pdf-viewer", async () => {
  const React = await import("react");

  const PdfViewer = React.forwardRef(function MockPdfViewer(
    props: PdfViewerProps,
    ref: ForwardedRef<PdfViewerHandle>,
  ) {
    pdfMock.pendingSource = props.pendingSource ?? false;
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
      return () => {
        pdfMock.unmounts += 1;
      };
    }, []);
    React.useEffect(() => {
      if (!props.pendingSource) {
        props.onLoad?.({ numPages: 3, pdfDocument: {} as never });
      }
    }, [props.pendingSource]);

    return (
      <div
        aria-label={props.ariaLabel}
        data-pdf-viewer-root
        data-testid="pdf-viewer"
        role="region"
      >
        <div tabIndex={0}>PDF canvas</div>
        {props.renderToolbar?.({} as PdfToolbarContext)}
        {props.pendingSource ? <div data-testid="pdf-page-skeleton" /> : null}
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
      <div aria-label="PDF viewer controls" role="toolbar">
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
  pdfMock.goToPage.mockReset();
  pdfMock.mounts = 0;
  pdfMock.unmounts = 0;
  pdfMock.pendingSource = false;
});

afterEach(cleanup);

describe("DocumentPreviewPanel navigation contract", () => {
  it("mounts the document viewer header and page skeleton while preview bytes load", () => {
    const { container, onPageCountChange } = renderPreview(
      { status: "idle" },
      { status: "loading" },
    );

    expect(
      screen.getByRole("region", { name: `PDF document viewer: ${document.name}` }),
    ).toBeTruthy();
    expect(screen.getByRole("toolbar", { name: "PDF viewer controls" })).toBeTruthy();
    expect(screen.getByText(document.name)).toBeTruthy();
    expect(screen.getByTestId("pdf-page-skeleton")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Close document preview" }).hasAttribute("disabled"))
      .toBe(false);
    expect(container.querySelector("section > header")).toBeNull();
    expect(pdfMock.pendingSource).toBe(true);
    expect(onPageCountChange).toHaveBeenCalledWith(0);
  });

  it("replaces the page skeleton without remounting the viewer shell", () => {
    const { container, resolvePreview } = renderPreview(
      { status: "idle" },
      { status: "loading" },
    );
    const viewer = container.querySelector("[data-testid='pdf-viewer']");

    resolvePreview();

    expect(container.querySelector("[data-testid='pdf-viewer']")).toBe(viewer);
    expect(screen.queryByTestId("pdf-page-skeleton")).toBeNull();
    expect(pdfMock.pendingSource).toBe(false);
    expect(pdfMock.mounts).toBe(1);
    expect(pdfMock.unmounts).toBe(0);
  });

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
