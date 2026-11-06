import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  type RefObject,
} from "react";
import type { AnnotationTransferItem } from "@embedpdf/plugin-annotation/react";
import previewLoadingMark from "../../assets/quarry-preview-mark.svg";
import type { DataRoomTreeNode } from "../../data/dataRoom";
import type {
  DocumentPreviewResponse,
  PreviewState,
} from "../../data/dataRoomPreview";
import { PDFEditor, type PDFEditorHandle } from "../extend/pdf-editor";
import { Icon } from "../ui/Icon";

const inMemoryAnnotations = new Map<string, AnnotationTransferItem[]>();

type DocumentPreviewPanelProps = {
  document: DataRoomTreeNode;
  onClose: () => void;
  onPageCountChange: (pageCount: number) => void;
  onRetry?: () => void;
  onRequestedPageHandled: () => void;
  preview: PreviewState;
  requestedPage: number | null;
};

export type DocumentPreviewPanelHandle = {
  focusViewer: () => void;
};

export type { PreviewState } from "../../data/dataRoomPreview";

export const DocumentPreviewPanel = forwardRef<
  DocumentPreviewPanelHandle,
  DocumentPreviewPanelProps
>(function DocumentPreviewPanel(
  {
    document,
    onClose,
    onPageCountChange,
    onRetry,
    onRequestedPageHandled,
    preview,
    requestedPage,
  },
  ref,
) {
  const editorRef = useRef<PDFEditorHandle>(null);

  useEffect(() => {
    if (requestedPage === null) {
      return;
    }
    if (!editorRef.current) {
      return;
    }
    const frame = requestAnimationFrame(() => {
      editorRef.current?.scrollToPage(requestedPage);
      onRequestedPageHandled();
    });

    return () => cancelAnimationFrame(frame);
  }, [onRequestedPageHandled, requestedPage]);

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
          editorRef.current
            ?.getViewportElement()
            ?.focus({ preventScroll: true });
        });
      },
    }),
    [],
  );

  return (
    <section className="glass-panel workspace-pane relative flex h-full min-h-0 min-w-0 flex-1 basis-0 flex-col overflow-hidden rounded-none border-y-0">
      {preview.status !== "ready" ? (
        <DocumentPreviewHeader
          document={document}
          onClose={onClose}
          subtitle="Document preview"
        />
      ) : null}

      <div className="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
        {preview.status === "error" ? (
          <div className="flex min-h-0 flex-1 flex-col items-center justify-center">
            <PreviewMessage
              detail={preview.message}
              title={
                document.error ? "File is inaccessible" : "Preview unavailable"
              }
              tone="error"
            />
            {onRetry ? (
              <button
                type="button"
                className="m-4 rounded px-3 py-2 text-sm text-primary focus-visible:ring-2 focus-visible:ring-ring"
                onClick={onRetry}
              >
                Try again
              </button>
            ) : null}
          </div>
        ) : null}

        {preview.status === "loading" ? <PdfPreviewLoading /> : null}

        {preview.status === "ready" ? (
          <PdfEditorViewer
            documentId={document.id}
            documentName={document.name}
            onLoad={onPageCountChange}
            response={preview.response}
            editorRef={editorRef}
          />
        ) : null}

      </div>
    </section>
  );
});

function DocumentPreviewHeader({
  document,
  onClose,
  subtitle,
}: {
  document: DataRoomTreeNode;
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

function PdfPreviewLoading() {
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
        <p className="mt-5 text-base font-semibold text-text-main">
          Loading document preview
        </p>
        <p className="mt-1 text-sm leading-6 text-muted">
          Preparing the PDF editor…
        </p>
      </div>
    </div>
  );
}

function PdfEditorViewer({
  documentId,
  documentName,
  onLoad,
  response,
  editorRef,
}: {
  documentId: string;
  documentName: string;
  onLoad: (numPages: number) => void;
  response: DocumentPreviewResponse;
  editorRef: RefObject<PDFEditorHandle | null>;
}) {
  const decodedPdf = useMemo(() => buildPdfSource(response), [response]);
  const documentLoadedRef = useRef(false);
  const annotationImportFrameRef = useRef<number | null>(null);

  useEffect(
    () => () => {
      if (annotationImportFrameRef.current !== null) {
        cancelAnimationFrame(annotationImportFrameRef.current);
      }
    },
    [],
  );

  if ("message" in decodedPdf) {
    return (
      <PreviewMessage
        detail={decodedPdf.message}
        title="PDF data is invalid"
        tone="error"
      />
    );
  }

  return (
    <div
      aria-label={`PDF document editor: ${response.fileName ?? documentName}`}
      className="min-h-0 min-w-0 flex-1 bg-surface-container [html[data-theme=dark]_&]:bg-black"
      role="region"
    >
      <PDFEditor
        className="h-full min-h-0 rounded-none border-0"
        defaultMode="annotate"
        features={{
          attachments: false,
          capture: false,
          forms: false,
          fullscreen: false,
          print: false,
          redact: false,
          security: false,
          sign: false,
          stamps: false,
        }}
        fileName={response.fileName ?? documentName}
        onAnnotationsChange={(items) => {
          if (documentLoadedRef.current) {
            inMemoryAnnotations.set(documentId, items);
          }
        }}
        onDocumentLoadSuccess={({ numPages }) => {
          documentLoadedRef.current = true;
          onLoad(numPages);
          const annotations = inMemoryAnnotations.get(documentId);
          if (annotations?.length) {
            annotationImportFrameRef.current = requestAnimationFrame(() => {
              editorRef.current?.importAnnotations(annotations);
              annotationImportFrameRef.current = null;
            });
          }
        }}
        persistSignatures={false}
        ref={editorRef}
        showUpload={false}
        signatureFontsStylesheetUrl={null}
        src={decodedPdf.source}
      />
    </div>
  );
}

type PreviewMessageProps = {
  detail: string;
  title: string;
  tone?: "default" | "error";
};

function PreviewMessage({
  detail,
  title,
  tone = "default",
}: PreviewMessageProps) {
  return (
    <div className="flex min-h-0 flex-1 items-center justify-center bg-surface-container p-8">
      <div
        className={`max-w-[36rem] rounded-2xl border bg-surface-container-lowest p-7 text-center shadow-sm ${
          tone === "error" ? "border-error/25" : "border-outline-variant"
        }`}
      >
        <span
          className={`mx-auto mb-4 flex h-12 w-12 items-center justify-center rounded-full ${
            tone === "error"
              ? "bg-error/10 text-error"
              : "bg-primary/10 text-primary"
          }`}
        >
          <Icon className="h-6 w-6" name={tone === "error" ? "alert" : "pdf"} />
        </span>
        <h2 className="text-lg font-semibold text-text-main">{title}</h2>
        <p className="mt-2 break-words text-sm leading-6 text-muted">
          {detail}
        </p>
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

function buildPdfSource(
  response: DocumentPreviewResponse,
): { source: Uint8Array } | { message: string } {
  if (response.mimeType !== "application/pdf") {
    return {
      message: `Expected application/pdf data, but received ${response.mimeType || "an unknown type"}.`,
    };
  }

  try {
    const data = response.pdfBytes ?? decodeBase64(response.pdfBase64);
    if (
      data.length < 5 ||
      String.fromCharCode(...data.subarray(0, 5)) !== "%PDF-"
    ) {
      return {
        message: "The preview payload does not contain a valid PDF header.",
      };
    }
    return { source: data };
  } catch (error) {
    return {
      message: `The preview payload could not be decoded: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
}

function iconNameForNode(
  kind: DataRoomTreeNode["kind"],
): "doc" | "pdf" | "sheet" {
  if (kind === "pdf") {
    return "pdf";
  }
  if (kind === "sheet") {
    return "sheet";
  }
  return "doc";
}
