"use client";

import "react-pdf/dist/Page/AnnotationLayer.css";
import "react-pdf/dist/Page/TextLayer.css";

import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";
import { Document } from "react-pdf";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { cn } from "@/lib/utils";

import { PdfViewerContext } from "./hooks/use-pdf-viewer-context";
import { usePdfDocument } from "./hooks/use-pdf-document";
import { usePdfDrop } from "./hooks/use-pdf-drop";
import { usePdfKeyboard } from "./hooks/use-pdf-keyboard";
import { usePdfPageTracker } from "./hooks/use-pdf-page-tracker";
import { usePdfPrint } from "./hooks/use-pdf-print";
import { usePdfSelection } from "./hooks/use-pdf-selection";
import { usePdfZoom } from "./hooks/use-pdf-zoom";
import { resolveVisiblePages } from "./hooks/use-pdf-virtualization";

import { PdfContextMenu } from "./parts/pdf-context-menu";
import { PdfDropOverlay } from "./parts/pdf-drop-overlay";
import { PdfEmptyState } from "./parts/pdf-empty-state";
import { PdfErrorState } from "./parts/pdf-error-state";
import { PdfLoadingState } from "./parts/pdf-loading-state";
import { PdfPage } from "./parts/pdf-page";
import { PdfPasswordPrompt } from "./parts/pdf-password-prompt";
import { PdfToolbar } from "./parts/pdf-toolbar";

import { downloadAsFile } from "./lib/download";
import { getSourceFilename, normalizeSource } from "./lib/normalize-source";
import { ensureWorkerConfigured } from "./lib/worker-config";

import {
  DEFAULT_PDF_VIEWER_LABELS,
  type PdfActions,
  type PdfDocumentState,
  type PdfFitMode,
  type PdfRotation,
  type PdfSource,
  type PdfStatus,
  type PdfViewerHandle,
  type PdfViewerLabels,
  type PdfViewerProps,
  type ResolvedPdfViewerLabels,
} from "./types";

const COMPACT_BREAKPOINT_PX = 480;
const PDF_PAGE_GAP_PX = 12;
const PDF_PAGE_CONTENT_PADDING_PX = 16;
const DEFAULT_PDF_PAGE_SIZE = { width: 612, height: 792 };

function buildPageLayout(
  numPages: number,
  pageSizes: ReadonlyArray<{ width: number; height: number } | null>,
  scale: number,
  rotation: PdfRotation,
): { offsets: number[]; heights: number[]; totalHeight: number } {
  const fallback = pageSizes[0] ?? DEFAULT_PDF_PAGE_SIZE;
  const offsets = new Array<number>(numPages);
  const heights = new Array<number>(numPages);
  let offset = 0;
  for (let index = 0; index < numPages; index += 1) {
    const size = pageSizes[index] ?? fallback;
    const height =
      (rotation % 180 === 0 ? size.height : size.width) * scale;
    offsets[index] = offset;
    heights[index] = height;
    offset += height + (index + 1 < numPages ? PDF_PAGE_GAP_PX : 0);
  }
  return { offsets, heights, totalHeight: offset };
}

function resolveLabels(
  override: PdfViewerLabels | undefined,
): ResolvedPdfViewerLabels {
  if (!override) return DEFAULT_PDF_VIEWER_LABELS;
  return { ...DEFAULT_PDF_VIEWER_LABELS, ...override };
}

export const PdfViewer = forwardRef<PdfViewerHandle, PdfViewerProps>(
  function PdfViewer(props, ref) {
    const {
      source,
      toolbar = true,
      renderToolbar,
      enableDragDrop = true,
      dragDropOverlay = true,
      initialPage = 1,
      initialScale = "fit-width",
      initialRotation = 0,
      virtualize = "auto",
      virtualizeThreshold = 50,
      enableContextMenu = true,
      renderContextMenu,
      onSearchSelection,
      password,
      renderPasswordPrompt,
      workerSrc,
      allowDownload = true,
      allowPrint = true,
      downloadFilename = "document.pdf",
      onLoad,
      onError,
      onPageChange,
      onScaleChange,
      onRotationChange,
      onSelection,
      onSourceChange,
      className,
      style,
      scrollContainerClassName,
      pageClassName,
      labels: labelsProp,
      ariaLabel,
    } = props;

    const labels = useMemo(() => resolveLabels(labelsProp), [labelsProp]);

    // Worker config
    useEffect(() => {
      ensureWorkerConfigured(workerSrc);
    }, [workerSrc]);

    // Internal source state — driven by `source` prop, mutated by drop.
    const [internalSource, setInternalSource] = useState<PdfSource | null>(
      source ?? null,
    );
    const lastPropSourceRef = useRef<PdfSource | null | undefined>(source);
    useEffect(() => {
      if (lastPropSourceRef.current !== source) {
        lastPropSourceRef.current = source;
        setInternalSource(source ?? null);
        onSourceChange?.({ source: source ?? null, reason: "prop" });
      }
    }, [source, onSourceChange]);

    // Notify on dropped-source change (when user drops a file)
    const replaceSource = (
      next: PdfSource | null,
      reason: "prop" | "drop",
    ) => {
      setInternalSource(next);
      onSourceChange?.({ source: next, reason });
    };

    // Document loading lifecycle
    const documentState = usePdfDocument({
      source: internalSource,
      password,
    });
    const normalizedSource = useMemo(
      () => normalizeSource(internalSource),
      [internalSource, documentState.retryToken],
    );

    // Load only the first page size eagerly for fit-scale. Remaining sizes are
    // captured as virtualized pages enter the mounted range.
    const [pageSizes, setPageSizes] = useState<
      Array<{ width: number; height: number } | null>
    >([]);
    useEffect(() => {
      const pdf = documentState.pdfDocument;
      if (!pdf) {
        setPageSizes([]);
        return;
      }
      let cancelled = false;
      setPageSizes(new Array(pdf.numPages).fill(null));
      void pdf
        .getPage(1)
        .then((firstPage) => {
          if (cancelled) return;
          const viewport = firstPage.getViewport({ scale: 1, rotation: 0 });
          setPageSizes((current) => {
            const next = [...current];
            next[0] = { width: viewport.width, height: viewport.height };
            return next;
          });
        })
        .catch(() => {
          // The visible Page component will report the size or load error.
        });

      return () => {
        cancelled = true;
      };
    }, [documentState.pdfDocument]);
    const recordPageSize = useCallback(
      (pageNumber: number, size: { width: number; height: number }) => {
        setPageSizes((current) => {
          const index = pageNumber - 1;
          const previous = current[index];
          if (
            index < 0 ||
            index >= current.length ||
            (previous?.width === size.width && previous.height === size.height)
          ) {
            return current;
          }
          const next = [...current];
          next[index] = size;
          return next;
        });
      },
      [],
    );

    // Refs
    const rootRef = useRef<HTMLElement>(null);
    const scrollContainerRef = useRef<HTMLDivElement>(null);

    // Compact mode tracking via ResizeObserver
    const [compact, setCompact] = useState(false);
    /*
     * v0.3.1 — open-state + anchor point for the `renderContextMenu` slot.
     * `null` means closed; the built-in Radix menu does not use this (it owns
     * its own positioning), so this is purely the consumer-slot path.
     */
    const [customMenuPos, setCustomMenuPos] = useState<{
      x: number;
      y: number;
    } | null>(null);

    /*
     * Escape closes the custom context menu.
     *
     * Review finding: this started as `onKeyDown` on the scroll container, which
     * only fires while THAT element has focus — and a context menu almost always
     * takes focus itself the moment it opens, so the handler could not fire in
     * the one situation it existed for. Listening at the document is what makes
     * the documented behaviour actually true.
     */
    useEffect(() => {
      if (!customMenuPos) return;
      const onKey = (event: KeyboardEvent) => {
        if (event.key === "Escape") setCustomMenuPos(null);
      };
      document.addEventListener("keydown", onKey);
      return () => document.removeEventListener("keydown", onKey);
    }, [customMenuPos]);
    useEffect(() => {
      const root = rootRef.current;
      if (!root) return;
      const ro = new ResizeObserver(() => {
        setCompact(root.clientWidth < COMPACT_BREAKPOINT_PX);
      });
      ro.observe(root);
      setCompact(root.clientWidth < COMPACT_BREAKPOINT_PX);
      return () => ro.disconnect();
    }, []);

    // Page state
    const [page, setPage] = useState<number>(Math.max(1, initialPage));
    const [rotation, setRotation] = useState<PdfRotation>(initialRotation);

    // Reset page + rotation on document change
    useEffect(() => {
      setPage(Math.max(1, initialPage));
      setRotation(initialRotation);
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [documentState.pdfDocument]);

    // Base page dimensions for fit-scale (use first page)
    const basePage = pageSizes[0] ?? null;

    // Zoom hook
    const zoom = usePdfZoom({
      containerRef: scrollContainerRef,
      basePageWidth: basePage?.width ?? 612,
      basePageHeight: basePage?.height ?? 792,
      rotation,
      initialScale,
      onScaleChange,
    });
    const pageLayout = useMemo(
      () =>
        buildPageLayout(
          documentState.numPages,
          pageSizes,
          zoom.scale,
          rotation,
        ),
      [documentState.numPages, pageSizes, zoom.scale, rotation],
    );

    // Page tracker
    const pageTracker = usePdfPageTracker({
      containerRef: scrollContainerRef,
      numPages: documentState.numPages,
      ready: documentState.status === "ready",
      pageOffsets: pageLayout.offsets,
      pageHeights: pageLayout.heights,
      contentPaddingTop: PDF_PAGE_CONTENT_PADDING_PX,
      onPageChange: (next) => {
        setPage((prev) => {
          if (prev !== next) {
            onPageChange?.({ page: next, source: internalSource });
          }
          return next;
        });
      },
    });

    // Print
    const printer = usePdfPrint({
      pdfDocument: documentState.pdfDocument,
    });

    // Selection
    const selectedText = usePdfSelection({
      rootRef: rootRef as unknown as { current: HTMLElement | null },
      onSelection,
    });

    // Drop
    const drop = usePdfDrop({
      enabled: enableDragDrop,
      onPdfDropped: (file) => {
        replaceSource(file, "drop");
      },
      onNonPdfDropped: () => {
        // Hook into a future toast — for now, no-op (label is exposed via ctx).
      },
    });

    // Status (drives UI surface)
    const status: PdfStatus = useMemo(() => {
      if (documentState.status === "idle") return "empty";
      if (documentState.status === "loading") return "loading";
      if (documentState.status === "password") return "password";
      if (documentState.status === "ready") return "ready";
      return "error";
    }, [documentState.status]);

    // Notify load + error
    const lastLoadedRef = useRef<PDFDocumentProxy | null>(null);
    useEffect(() => {
      if (
        documentState.status === "ready" &&
        documentState.pdfDocument &&
        lastLoadedRef.current !== documentState.pdfDocument
      ) {
        lastLoadedRef.current = documentState.pdfDocument;
        onLoad?.({
          numPages: documentState.numPages,
          pdfDocument: documentState.pdfDocument,
        });
      }
      if (documentState.status === "error" && documentState.error) {
        onError?.({
          error: documentState.error,
          source: internalSource,
        });
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [documentState.status, documentState.pdfDocument, documentState.error]);

    // Notify rotation change
    useEffect(() => {
      onRotationChange?.({ rotation });
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [rotation]);

    // Build actions
    const actions = useMemo<PdfActions>(
      () => ({
        goToPage: (n: number) => {
          const total = documentState.numPages;
          const clamped = Math.max(
            1,
            Math.min(total > 0 ? total : n, Math.floor(n)),
          );
          pageTracker.scrollToPage(clamped);
        },
        goToNextPage: () => {
          const total = documentState.numPages;
          if (page < total) pageTracker.scrollToPage(page + 1);
        },
        goToPrevPage: () => {
          if (page > 1) pageTracker.scrollToPage(page - 1);
        },
        setScale: (s) => zoom.setScale(s),
        zoomIn: () => zoom.zoomIn(),
        zoomOut: () => zoom.zoomOut(),
        resetZoom: () => zoom.resetZoom(),
        rotate: (delta = 90) => {
          setRotation((current) => {
            const next = (((current + delta) % 360) + 360) % 360;
            const snapped = (Math.round(next / 90) * 90) % 360;
            return snapped as PdfRotation;
          });
        },
        setRotation: (r) => setRotation(r),
        download: () => {
          const filename =
            getSourceFilename(internalSource) ?? downloadFilename;
          void downloadAsFile(internalSource, filename);
        },
        print: () => {
          if (!allowPrint) return;
          void printer.print();
        },
      }),
      [
        documentState.numPages,
        page,
        zoom,
        pageTracker,
        internalSource,
        downloadFilename,
        allowPrint,
        printer,
      ],
    );

    // Keyboard
    usePdfKeyboard({
      containerRef: scrollContainerRef,
      rootRef: rootRef as unknown as { current: HTMLElement | null },
      actions,
      status,
      allowDownload,
      allowPrint,
    });

    // Document state for context
    const docState: PdfDocumentState = useMemo(
      () => ({
        page,
        numPages: documentState.numPages,
        scale: zoom.scale,
        fitMode: zoom.fitMode as PdfFitMode,
        rotation,
        loading: status === "loading",
        ready: status === "ready",
        error: documentState.error,
        source: internalSource,
        selectedText,
        status,
      }),
      [
        page,
        documentState.numPages,
        zoom.scale,
        zoom.fitMode,
        rotation,
        status,
        documentState.error,
        internalSource,
        selectedText,
      ],
    );

    // Imperative handle
    useImperativeHandle(
      ref,
      () => ({
        actions,
        state: docState,
        pdfDocument: documentState.pdfDocument,
      }),
      [actions, docState, documentState.pdfDocument],
    );

    // Context value
    const contextValue = {
      ...docState,
      actions,
      labels,
      allowDownload,
      allowPrint,
      compact,
      pdfDocument: documentState.pdfDocument,
    };

    // Visible pages for rendering
    const visiblePages = useMemo(
      () =>
        resolveVisiblePages({
          numPages: documentState.numPages,
          currentPage: page,
          mode: virtualize,
          threshold: virtualizeThreshold,
        }),
      [documentState.numPages, page, virtualize, virtualizeThreshold],
    );

    // Toolbar slot context
    const toolbarCtx = {
      ...docState,
      actions,
      allowDownload,
      allowPrint,
      compact,
    };

    // Password prompt slot context
    const passwordCtx = {
      submit: documentState.submitPassword,
      cancel: documentState.cancelPassword,
      error: documentState.error,
      attempts: documentState.passwordAttempts,
    };

    // Context menu slot context
    const contextMenuCtx = {
      ...docState,
      actions,
      /*
       * v0.3.1 — real position + a close that closes.
       *
       * The custom slot used to render UNCONDITIONALLY at `{x:0, y:0}` with an
       * empty-bodied `closeMenu`, which made it unusable as a context menu: it
       * was always on screen, always in the corner, and could not be dismissed.
       * The built-in menu never hit this because Radix owns its own positioning
       * — only the consumer-facing slot was broken.
       */
      position: customMenuPos ?? { x: 0, y: 0 },
      closeMenu: () => setCustomMenuPos(null),
      onSearch: onSearchSelection
        ? (text: string) => onSearchSelection({ text })
        : null,
    };

    return (
      <PdfViewerContext.Provider value={contextValue}>
        <section
          ref={rootRef as React.RefObject<HTMLElement>}
          aria-label={ariaLabel ?? labels.viewerAriaLabel}
          className={cn(
            "relative flex h-full w-full flex-col overflow-hidden rounded-lg border border-border bg-background",
            className,
          )}
          style={style}
          data-pdf-viewer-root
        >
          {toolbar
            ? (renderToolbar?.(toolbarCtx) ?? <PdfToolbar />)
            : null}

          <div
            ref={scrollContainerRef}
            {...(enableDragDrop ? drop.dropProps : {})}
            onContextMenu={
              enableContextMenu && renderContextMenu
                ? (event) => {
                    event.preventDefault();
                    setCustomMenuPos({ x: event.clientX, y: event.clientY });
                  }
                : undefined
            }
            className={cn(
              "relative flex-1 overflow-auto bg-surface-container [touch-action:pan-x_pan-y]",
              "[&_.react-pdf__Page__textContent]:select-text",
              scrollContainerClassName,
            )}
            tabIndex={0}
          >
            {status === "empty" ? (
              <PdfEmptyState
                labels={labels}
                enableDragDrop={enableDragDrop}
              />
            ) : null}
            {status === "loading" ? <PdfLoadingState labels={labels} /> : null}
            {status === "error" && documentState.error ? (
              <PdfErrorState
                labels={labels}
                error={documentState.error}
                onRetry={documentState.retry}
              />
            ) : null}
            {/*
              Document wraps the page list so AnnotationLayer/TextLayer can
              read `pdf` from DocumentContext. `className="contents"` keeps
              its wrapper div transparent to layout.
            */}
            {normalizedSource ? (
              <Document
                key={documentState.retryToken}
                file={normalizedSource}
                loading={null}
                error={null}
                noData={null}
                externalLinkTarget="_blank"
                className="contents"
                {...documentState.documentCallbacks}
              >
                {status === "ready" && documentState.pdfDocument ? (
                  <PdfContextMenu
                    enabled={enableContextMenu && !renderContextMenu}
                    onSearchSelection={
                      onSearchSelection
                        ? (text) => onSearchSelection({ text })
                        : undefined
                    }
                    className="relative w-full p-4"
                  >
                    <div
                      className="relative w-full"
                      style={{ height: pageLayout.totalHeight }}
                    >
                      {Array.from(visiblePages, (pageNumber) => {
                        const index = pageNumber - 1;
                        return (
                          <div
                            key={pageNumber}
                            className="absolute inset-x-0"
                            style={{
                              top: pageLayout.offsets[index],
                              height: pageLayout.heights[index],
                            }}
                          >
                            <PdfPage
                              pdfDocument={documentState.pdfDocument!}
                              pageNumber={pageNumber}
                              scale={zoom.scale}
                              rotation={rotation}
                              basePageSize={pageSizes[index] ?? null}
                              onPageSize={recordPageSize}
                              className={pageClassName}
                            />
                          </div>
                        );
                      })}
                    </div>
                  </PdfContextMenu>
                ) : null}
              </Document>
            ) : null}
          </div>

          {status === "password"
            ? (renderPasswordPrompt?.(passwordCtx) ?? (
                <PdfPasswordPrompt
                  open={true}
                  labels={labels}
                  error={documentState.error}
                  attempts={documentState.passwordAttempts}
                  onSubmit={documentState.submitPassword}
                  onCancel={documentState.cancelPassword}
                />
              ))
            : null}

          {/* Custom context-menu slot (when consumer overrides) */}
          {status === "ready" && enableContextMenu && renderContextMenu && customMenuPos
            ? renderContextMenu(contextMenuCtx)
            : null}

          {drop.isDragging && enableDragDrop && dragDropOverlay ? (
            <PdfDropOverlay labels={labels} />
          ) : null}
        </section>
      </PdfViewerContext.Provider>
    );
  },
);

export default PdfViewer;
