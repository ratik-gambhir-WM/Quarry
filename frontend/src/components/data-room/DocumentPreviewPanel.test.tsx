// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ForwardedRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  PdfToolbarContext,
  PdfViewerHandle,
  PdfViewerProps,
} from "../pdf-viewer";
import { DocumentPreviewPanel, type RawTextState } from "./DocumentPreviewPanel";

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
  const view = render(
    <DocumentPreviewPanel
      document={document}
      onClose={vi.fn()}
      onRequestRawText={onRequestRawText}
      preview={preview}
      rawText={rawText}
    />,
  );
  return { ...view, onRequestRawText };
}

beforeEach(() => {
  pdfMock.goToPage.mockReset();
  pdfMock.mounts = 0;
  pdfMock.unmounts = 0;
});

afterEach(cleanup);

describe("DocumentPreviewPanel search", () => {
  it("places the only search trigger beside the selected filename", () => {
    renderPreview();
    const heading = screen.getByRole("heading", { name: document.name });
    expect(
      within(heading.parentElement!).getByRole("button", { name: "Search document" }),
    ).toBeTruthy();
    expect(within(heading.parentElement!).queryByText("Search")).toBeNull();
    expect(screen.queryByLabelText("Open document search")).toBeNull();
  });

  it("keeps the mounted viewer and position unchanged when search is cancelled", async () => {
    const user = userEvent.setup();
    const { container, onRequestRawText } = renderPreview();
    const viewer = container.querySelector("[data-testid='pdf-viewer']");

    await user.click(screen.getByRole("button", { name: "Search document" }));
    expect(container.querySelector("[data-testid='pdf-viewer']")).toBe(viewer);
    await user.keyboard("{Escape}");

    expect(container.querySelector("[data-testid='pdf-viewer']")).toBe(viewer);
    expect(pdfMock.goToPage).not.toHaveBeenCalled();
    expect(pdfMock.mounts).toBe(1);
    expect(pdfMock.unmounts).toBe(0);
    expect(onRequestRawText).not.toHaveBeenCalled();
  });

  it("navigates a supported result exactly once and keeps the document selected", async () => {
    const user = userEvent.setup();
    renderPreview();
    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(screen.getByRole("searchbox", { name: "Search document" }), "Synthetic");
    await user.click(
      screen.getByRole("option", { name: /Synthetic Terms\.pdf.*Open page 1/ }),
    );

    expect(pdfMock.goToPage).toHaveBeenCalledTimes(1);
    expect(pdfMock.goToPage).toHaveBeenCalledWith(1);
    expect(screen.getByRole("heading", { name: document.name })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("returns from raw text before applying a page target", async () => {
    const user = userEvent.setup();
    const rawText: RawTextState = {
      response: {
        fileName: document.name,
        sourceKind: "pdf",
        text: "Synthetic extracted text.",
      },
      status: "ready",
    };
    const { onRequestRawText } = renderPreview(rawText);
    await user.click(screen.getByRole("button", { name: "Show raw text" }));
    expect(screen.getByText("Synthetic extracted text.")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Search document" }));
    await user.type(screen.getByRole("searchbox", { name: "Search document" }), "Synthetic");
    await user.click(
      screen.getByRole("option", { name: /Synthetic Terms\.pdf.*Open page 1/ }),
    );

    expect(screen.getByTestId("pdf-viewer")).toBeTruthy();
    expect(pdfMock.goToPage).toHaveBeenCalledTimes(1);
    expect(pdfMock.goToPage).toHaveBeenCalledWith(1);
    expect(onRequestRawText).not.toHaveBeenCalled();
  });
});
