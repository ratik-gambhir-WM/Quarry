import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type RefObject,
} from "react";
import PdfWorkerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import previewLoadingMark from "../../assets/quarry-preview-mark.svg";
import type { DealDocumentText } from "../../contracts/quarryApi";
import type { DataRoomTreeNode } from "../../data/dataRoom";
import type { DocumentPreviewResponse } from "../../data/dataRoomPreview";
import { PdfToolbar, PdfViewer as ShadcnPdfViewer } from "../pdf-viewer";
import type { PdfViewerHandle } from "../pdf-viewer";
import { Icon } from "../ui/Icon";

type DocumentPreviewPanelProps = {
  document: DataRoomTreeNode;
  onClose: () => void;
  onPageCountChange: (pageCount: number) => void;
  onRequestRawText: () => void;
  onRequestedPageHandled: () => void;
  preview: PreviewState;
  rawText: RawTextState;
  requestedPage: number | null;
};

export type DocumentPreviewPanelHandle = {
  focusViewer: () => void;
};

export type PreviewState =
  | { status: "loading" }
  | { message: string; status: "error" }
  | { response: DocumentPreviewResponse; status: "ready" };

export type RawTextState =
  | { status: "idle" }
  | { status: "loading" }
  | { message: string; status: "error" }
  | { response: DealDocumentText; status: "ready" };

export const DocumentPreviewPanel = forwardRef<
  DocumentPreviewPanelHandle,
  DocumentPreviewPanelProps
>(function DocumentPreviewPanel(
  {
    document,
    onClose,
    onPageCountChange,
    onRequestRawText,
    onRequestedPageHandled,
    preview,
    rawText,
    requestedPage,
  },
  ref,
) {
  const [viewMode, setViewMode] = useState<"preview" | "raw-text">("preview");
  const previewBodyRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<PdfViewerHandle>(null);
  const canShowRawText = Boolean(document.storedFileId);

  useEffect(() => {
    if (requestedPage === null) {
      return;
    }
    if (viewMode !== "preview") {
      setViewMode("preview");
      return;
    }
    if (!viewerRef.current) {
      return;
    }
    const frame = requestAnimationFrame(() => {
      viewerRef.current?.actions.goToPage(requestedPage);
      onRequestedPageHandled();
    });

    return () => cancelAnimationFrame(frame);
  }, [onRequestedPageHandled, requestedPage, viewMode]);

  useEffect(() => {
    if (preview.status !== "ready") {
      onPageCountChange(0);
    }
  }, [onPageCountChange, preview.status]);

  useImperativeHandle(
    ref,
    () => ({
      focusViewer() {
        requestAnimationFrame(() => {
          previewBodyRef.current
            ?.querySelector<HTMLElement>("[data-pdf-viewer-root] [tabindex='0']")
            ?.focus({ preventScroll: true });
        });
      },
    }),
    [],
  );

  function showRawText() {
    setViewMode("raw-text");
    if (rawText.status === "idle") {
      onRequestRawText();
    }
  }

  return (
    <section className="glass-panel workspace-pane relative flex h-full min-h-0 min-w-0 flex-1 basis-0 flex-col overflow-hidden rounded-none border-y-0">
      {viewMode === "raw-text" || preview.status === "error" ? (
        <DocumentPreviewHeader
          document={document}
          onBackToPreview={viewMode === "raw-text" ? () => setViewMode("preview") : undefined}
          onClose={onClose}
          subtitle={
            viewMode === "raw-text"
              ? rawText.status === "ready"
                ? `Raw text · ${rawText.response.sourceKind.toUpperCase()}`
                : "Raw document text"
              : "Document preview"
          }
        />
      ) : null}

      <div
        className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
        ref={previewBodyRef}
      >
        {viewMode === "raw-text" ? (
          <RawTextViewer rawText={rawText} />
        ) : (
          <>
            {preview.status === "error" ? (
              <PreviewMessage
                detail={preview.message}
                title={document.error ? "File is inaccessible" : "Preview unavailable"}
                tone="error"
              />
            ) : null}

            {preview.status === "loading" || preview.status === "ready" ? (
              <PdfViewer
                documentName={document.name}
                onClose={onClose}
                onLoad={onPageCountChange}
                onShowRawText={canShowRawText ? showRawText : undefined}
                response={preview.status === "ready" ? preview.response : undefined}
                viewerRef={viewerRef}
              />
            ) : null}
          </>
        )}
      </div>
    </section>
  );
});

function DocumentPreviewHeader({
  document,
  onBackToPreview,
  onClose,
  subtitle,
}: {
  document: DataRoomTreeNode;
  onBackToPreview?: () => void;
  onClose: () => void;
  subtitle: string;
}) {
  return (
    <header className="flex h-12 min-w-0 shrink-0 items-center justify-between gap-4 overflow-hidden border-b border-border bg-card px-5">
      <div className="flex min-w-0 flex-1 items-center gap-3 overflow-hidden">
        <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-primary/8 text-primary">
          <Icon className="h-5 w-5" name={iconNameForNode(document.kind)} />
        </span>
        <div className="min-w-0 flex-1 overflow-hidden">
          <h1
            className="min-w-0 truncate whitespace-nowrap text-[13px] font-semibold text-text-main"
            title={document.name}
          >
            {document.name}
          </h1>
          <p className="block max-w-full truncate whitespace-nowrap text-[11px] font-bold uppercase tracking-[0.12em] text-muted">
            {subtitle}
          </p>
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-3">
        {onBackToPreview ? (
          <button
            className="rounded-full border border-outline-variant bg-surface-container-lowest px-4 py-2 text-[12px] font-semibold text-muted transition hover:bg-surface-container hover:text-text-main"
            onClick={onBackToPreview}
            type="button"
          >
            Back to preview
          </button>
        ) : null}
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
      </div>
    </header>
  );
}

function RawTextViewer({ rawText }: { rawText: RawTextState }) {
  if (rawText.status === "idle" || rawText.status === "loading") {
    return <RawTextLoading />;
  }
  if (rawText.status === "error") {
    return <PreviewMessage detail={rawText.message} title="Raw text unavailable" tone="error" />;
  }

  return (
    <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-auto bg-surface-container px-6 py-8">
      <article className="mx-auto max-w-5xl rounded-2xl border border-outline-variant bg-surface-container-lowest shadow-sm">
        <header className="border-b border-outline-variant px-6 py-4">
          <p className="text-[11px] font-bold uppercase tracking-[0.12em] text-primary">
            Extracted from {rawText.response.sourceKind.toUpperCase()}
          </p>
          <h2 className="mt-1 truncate text-base font-semibold text-text-main" title={rawText.response.fileName}>
            {rawText.response.fileName}
          </h2>
        </header>
        <pre className="whitespace-pre-wrap break-words px-6 py-5 font-mono text-[13px] leading-6 text-text-main">
          {rawText.response.text}
        </pre>
      </article>
    </div>
  );
}

function RawTextLoading() {
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
        <p className="mt-5 text-base font-semibold text-text-main">Loading raw text</p>
        <p className="mt-1 text-sm leading-6 text-muted">
          Extracting the document’s raw text…
        </p>
      </div>
    </div>
  );
}

function PdfViewer({
  documentName,
  onClose,
  onLoad,
  onShowRawText,
  response,
  viewerRef,
}: {
  documentName: string;
  onClose: () => void;
  onLoad: (numPages: number) => void;
  onShowRawText?: () => void;
  response?: DocumentPreviewResponse;
  viewerRef: RefObject<PdfViewerHandle | null>;
}) {
  const decodedPdf = useMemo(() => (response ? buildPdfSource(response) : null), [response]);

  if (decodedPdf && "message" in decodedPdf) {
    return <PreviewMessage detail={decodedPdf.message} title="PDF data is invalid" tone="error" />;
  }

  return (
    <div className="min-h-0 min-w-0 flex-1 bg-surface-container [html[data-theme=dark]_&]:bg-black">
      <ShadcnPdfViewer
        allowPrint={false}
        ariaLabel={`PDF document viewer: ${response?.fileName ?? documentName}`}
        className="h-full min-h-0 rounded-none border-0"
        downloadFilename={response?.fileName ?? documentName}
        enableDragDrop={false}
        onLoad={({ numPages: loadedPageCount }) => onLoad(loadedPageCount)}
        ref={viewerRef}
        pendingSource={!response}
        renderToolbar={() => (
          <PdfToolbar
            leadingContent={
              <div className="flex min-w-0 items-center gap-2">
                <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/8 text-primary">
                  <Icon className="h-4 w-4" name="pdf" />
                </span>
                <h1
                  className="min-w-0 truncate whitespace-nowrap text-[13px] font-semibold text-text-main"
                  title={documentName}
                >
                  {documentName}
                </h1>
              </div>
            }
            onPrintAction={onShowRawText}
            printActionLabel="Show raw text"
            trailingContent={
              <button
                aria-label="Close document preview"
                className="flex h-8 shrink-0 items-center gap-2 rounded-full border border-outline-variant bg-surface-container-lowest px-3 text-[11px] font-semibold text-muted transition hover:bg-surface-container hover:text-text-main"
                onClick={onClose}
                type="button"
              >
                <span aria-hidden="true" className="text-base leading-none">
                  ×
                </span>
                Close
              </button>
            }
          />
        )}
        scrollContainerClassName="workspace-scrollbar-hidden"
        source={decodedPdf && "source" in decodedPdf ? decodedPdf.source : null}
        workerSrc={PdfWorkerUrl}
      />
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
  | { source: Uint8Array }
  | { message: string } {
  if (response.mimeType !== "application/pdf") {
    return { message: `Expected application/pdf data, but received ${response.mimeType || "an unknown type"}.` };
  }

  try {
    const data = response.pdfBytes ?? decodeBase64(response.pdfBase64);
    if (data.length < 5 || String.fromCharCode(...data.subarray(0, 5)) !== "%PDF-") {
      return { message: "The preview payload does not contain a valid PDF header." };
    }
    return { source: data };
  } catch (error) {
    return {
      message: `The preview payload could not be decoded: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
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
