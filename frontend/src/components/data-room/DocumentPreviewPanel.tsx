import {
  useCallback,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { Document, Page, pdfjs } from "react-pdf";
import PdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?worker&inline";
import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";
import previewLoadingMark from "../../assets/quarry-preview-mark.svg";
import type { DataRoomTreeNode } from "../../data/dataRoom";
import type { DocumentPreviewResponse } from "../../data/dataRoomPreview";
import { Icon } from "../ui/Icon";

pdfjs.GlobalWorkerOptions.workerPort = new PdfWorker();

const PDF_VIEWPORT_PADDING = 32;
const DEFAULT_PAGE_SIZE = { height: 792, width: 612 };

type DocumentPreviewPanelProps = {
  document: DataRoomTreeNode;
  onClose: () => void;
  preview: PreviewState;
};

export type PreviewState =
  | { status: "loading" }
  | { message: string; status: "error" }
  | { response: DocumentPreviewResponse; status: "ready" };

export function DocumentPreviewPanel({
  document,
  onClose,
  preview,
}: DocumentPreviewPanelProps) {
  return (
    <section className="glass-panel relative flex h-full min-h-0 min-w-0 flex-1 basis-0 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-16 min-w-0 shrink-0 items-center justify-between gap-4 overflow-hidden border-b border-outline-variant bg-background px-5">
        <div className="flex min-w-0 flex-1 items-center gap-3 overflow-hidden">
          <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary/8 text-primary">
            <Icon className="h-5 w-5" name={iconNameForNode(document.kind)} />
          </span>
          <div className="min-w-0 flex-1 overflow-hidden">
            <h1 className="block max-w-full truncate whitespace-nowrap text-[16px] font-semibold text-text-main" title={document.name}>
              {document.name}
            </h1>
            <p className="block max-w-full truncate whitespace-nowrap text-[11px] font-bold uppercase tracking-[0.12em] text-muted">
              {preview.status === "ready"
                ? preview.response.sourceKind === "native"
                  ? "Native PDF"
                  : `PDF · converted from ${preview.response.sourceKind.replace("converted-from-", "").toUpperCase()}`
                : "Document preview"}
            </p>
          </div>
        </div>
        <button
          aria-label="Close document preview"
          className="flex h-9 shrink-0 items-center gap-2 rounded-full border border-outline-variant bg-surface-container-lowest px-4 text-[12px] font-semibold text-muted transition hover:bg-surface-container hover:text-text-main"
          onClick={onClose}
          type="button"
        >
          <span aria-hidden="true" className="text-lg leading-none">
            ×
          </span>
          Close
        </button>
      </header>

      {preview.status === "loading" ? (
        <PreviewLoading
          detail={
            document.name.toLowerCase().endsWith(".pdf")
              ? "Reading PDF from the deal data room…"
              : "Converting the Office document to PDF…"
          }
        />
      ) : null}

      {preview.status === "error" ? (
        <PreviewMessage
          detail={preview.message}
          title={document.error ? "File is inaccessible" : "Preview unavailable"}
          tone="error"
        />
      ) : null}

      {preview.status === "ready" ? <PdfViewer response={preview.response} /> : null}
    </section>
  );
}

function PdfViewer({ response }: { response: DocumentPreviewResponse }) {
  const [numPages, setNumPages] = useState(0);
  const [pageNumber, setPageNumber] = useState(1);
  const [zoom, setZoom] = useState(1);
  const [renderError, setRenderError] = useState("");
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE);
  const [viewportSize, setViewportSize] = useState({ height: 0, width: 0 });
  const resizeObserverRef = useRef<ResizeObserver | null>(null);
  const decodedPdf = useMemo(() => buildPdfSource(response), [response]);
  const previousPage = useCallback(
    () => setPageNumber((current) => Math.max(1, current - 1)),
    [],
  );
  const nextPage = useCallback(
    () => setPageNumber((current) => Math.min(numPages, current + 1)),
    [numPages],
  );
  const zoomOut = useCallback(
    () => setZoom((current) => Math.max(0.6, Number((current - 0.2).toFixed(1)))),
    [],
  );
  const zoomIn = useCallback(
    () => setZoom((current) => Math.min(2, Number((current + 0.2).toFixed(1)))),
    [],
  );
  const updatePageSize = useCallback((page: PDFPageProxy) => {
    const viewport = page.getViewport({ scale: 1 });
    setPageSize((current) =>
      current.width === viewport.width && current.height === viewport.height
        ? current
        : { height: viewport.height, width: viewport.width },
    );
  }, []);
  const handleLoadSuccess = useCallback(
    async (pdf: PDFDocumentProxy) => {
      setNumPages(pdf.numPages);
      setPageNumber(1);
      setRenderError("");
      updatePageSize(await pdf.getPage(1));
    },
    [updatePageSize],
  );
  const handleLoadError = useCallback((error: Error) => setRenderError(error.message), []);
  const setViewportNode = useCallback((node: HTMLDivElement | null) => {
    resizeObserverRef.current?.disconnect();
    resizeObserverRef.current = null;

    if (!node) {
      return;
    }

    const measure = () => {
      const next = { height: node.clientHeight, width: node.clientWidth };
      setViewportSize((current) =>
        current.width === next.width && current.height === next.height ? current : next,
      );
    };
    measure();

    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(measure);
      observer.observe(node);
      resizeObserverRef.current = observer;
    }
  }, []);
  const pageLayout = useMemo(
    () => calculatePageLayout(viewportSize, pageSize, zoom),
    [pageSize, viewportSize, zoom],
  );

  return (
    <>
      <div className="flex min-h-[52px] items-center justify-center gap-2 border-b border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] text-muted">
        <ToolbarButton
          disabled={pageNumber <= 1}
          label="Previous page"
          onClick={previousPage}
        >
          <Icon className="h-4 w-4" name="chevronLeft" />
        </ToolbarButton>
        <span className="min-w-[92px] text-center font-semibold text-text-main">
          Page {pageNumber} of {numPages || "…"}
        </span>
        <ToolbarButton
          disabled={!numPages || pageNumber >= numPages}
          label="Next page"
          onClick={nextPage}
        >
          <Icon className="h-4 w-4" name="chevronRight" />
        </ToolbarButton>
        <span className="mx-2 h-6 w-px bg-outline-variant" />
        <ToolbarButton
          disabled={zoom <= 0.6}
          label="Zoom out"
          onClick={zoomOut}
        >
          <span aria-hidden="true" className="text-lg leading-none">
            −
          </span>
        </ToolbarButton>
        <span className="min-w-[48px] text-center font-semibold text-text-main">{Math.round(zoom * 100)}%</span>
        <ToolbarButton
          disabled={zoom >= 2}
          label="Zoom in"
          onClick={zoomIn}
        >
          <Icon className="h-4 w-4" name="plus" />
        </ToolbarButton>
      </div>

      <div
        className="workspace-scrollbar-hidden min-h-0 min-w-0 flex-1 overflow-auto bg-surface-container [html[data-theme=dark]_&]:bg-black"
        ref={setViewportNode}
      >
        {"message" in decodedPdf ? (
          <PreviewMessage detail={decodedPdf.message} title="PDF data is invalid" tone="error" />
        ) : renderError ? (
          <PreviewMessage detail={renderError} title="PDF renderer could not open this document" tone="error" />
        ) : (
          <div
            className="flex items-center justify-center"
            style={pageLayout.stageStyle}
          >
            <Document
              className="flex h-full min-h-0 w-full min-w-0 items-center justify-center"
              error={null}
              file={decodedPdf.source}
              loading={<PreviewLoading detail="Rendering PDF pages…" />}
              onLoadError={handleLoadError}
              onLoadSuccess={handleLoadSuccess}
              onSourceError={handleLoadError}
            >
              <div className="w-fit overflow-hidden rounded-lg bg-white shadow-[0_16px_50px_rgba(7,1,84,0.16)]">
                <Page
                  onLoadSuccess={updatePageSize}
                  pageNumber={pageNumber}
                  renderAnnotationLayer={false}
                  renderTextLayer
                  width={pageLayout.pageWidth}
                />
              </div>
            </Document>
          </div>
        )}
      </div>
    </>
  );
}

type ToolbarButtonProps = {
  children: ReactNode;
  disabled?: boolean;
  label: string;
  onClick: () => void;
};

function ToolbarButton({ children, disabled = false, label, onClick }: ToolbarButtonProps) {
  return (
    <button
      aria-label={label}
      className="flex h-8 w-8 items-center justify-center rounded-lg border border-outline-variant bg-background text-primary transition hover:bg-surface-container disabled:cursor-not-allowed disabled:opacity-35"
      disabled={disabled}
      onClick={onClick}
      title={label}
      type="button"
    >
      {children}
    </button>
  );
}

function PreviewLoading({ detail }: { detail: string }) {
  return (
    <div
      aria-live="polite"
      className="flex min-h-0 flex-1 items-center justify-center bg-surface-container p-8 [html[data-theme=dark]_&]:bg-black"
      role="status"
    >
      <div className="text-center">
        <img
          alt=""
          aria-hidden="true"
          className="mx-auto h-14 w-14 animate-spin motion-reduce:animate-none [animation-duration:1.4s] [html[data-theme=dark]_&]:brightness-0 [html[data-theme=dark]_&]:invert"
          src={previewLoadingMark}
        />
        <p className="mt-5 text-base font-semibold text-text-main">Loading preview</p>
        <p className="mt-1 text-sm leading-6 text-muted">{detail}</p>
      </div>
    </div>
  );
}

type PreviewMessageProps = {
  detail: string;
  title: string;
  tone?: "default" | "error";
};

function PreviewMessage({ detail, title, tone = "default" }: PreviewMessageProps) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center bg-surface-container p-8">
      <div
        className={`max-w-xl rounded-2xl border bg-surface-container-lowest p-7 text-center shadow-sm ${
          tone === "error" ? "border-error/25" : "border-outline-variant"
        }`}
      >
        <span
          className={`mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full ${
            tone === "error" ? "bg-error/10 text-error" : "bg-primary/10 text-primary"
          }`}
        >
          <Icon className="h-6 w-6" name={tone === "error" ? "alert" : "pdf"} />
        </span>
        <h2 className="text-lg font-semibold text-text-main">{title}</h2>
        <p className="mt-2 break-words text-sm leading-6 text-muted">{detail}</p>
      </div>
    </div>
  );
}

function decodeBase64(value: string) {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

function buildPdfSource(response: DocumentPreviewResponse):
  | { source: { data: Uint8Array } }
  | { message: string } {
  if (response.mimeType !== "application/pdf") {
    return { message: `Expected application/pdf data, but received ${response.mimeType || "an unknown type"}.` };
  }

  try {
    const data = decodeBase64(response.pdfBase64);
    if (data.length < 5 || String.fromCharCode(...data.subarray(0, 5)) !== "%PDF-") {
      return { message: "The preview payload does not contain a valid PDF header." };
    }
    return { source: { data } };
  } catch (error) {
    return {
      message: `The preview payload could not be decoded: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function calculatePageLayout(
  viewport: { height: number; width: number },
  page: { height: number; width: number },
  zoom: number,
): { pageWidth: number; stageStyle: CSSProperties } {
  const availableWidth = Math.max(1, viewport.width - PDF_VIEWPORT_PADDING);
  const availableHeight = Math.max(1, viewport.height - PDF_VIEWPORT_PADDING);
  const fitScale = Math.min(availableWidth / page.width, availableHeight / page.height);
  const pageWidth = Math.max(1, Math.round(page.width * fitScale * zoom));
  const pageHeight = Math.max(1, Math.round(page.height * fitScale * zoom));

  return {
    pageWidth,
    stageStyle: {
      height: Math.max(viewport.height, pageHeight + PDF_VIEWPORT_PADDING),
      width: Math.max(viewport.width, pageWidth + PDF_VIEWPORT_PADDING),
    },
  };
}

function iconNameForNode(kind: DataRoomTreeNode["kind"]): "doc" | "pdf" | "sheet" {
  if (kind === "pdf") {
    return "pdf";
  }
  if (kind === "sheet") {
    return "sheet";
  }
  return "doc";
}
