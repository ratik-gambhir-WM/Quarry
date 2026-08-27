"use client";

import { Page } from "react-pdf";
import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import { cn } from "@/lib/utils";
import type { PdfRotation } from "../types";

interface PdfPageProps {
  pdfDocument: PDFDocumentProxy;
  pageNumber: number;
  scale: number;
  rotation: PdfRotation;
  /** Known intrinsic page size at scale=1, used while the page is loading. */
  basePageSize: { width: number; height: number } | null;
  onPageSize?: (pageNumber: number, size: { width: number; height: number }) => void;
  className?: string;
}

export function PdfPage({
  pdfDocument,
  pageNumber,
  scale,
  rotation,
  basePageSize,
  onPageSize,
  className,
}: PdfPageProps) {
  return (
    <div
      data-pdf-page={pageNumber}
      className={cn("relative mx-auto", className)}
    >
      <Page
        pdf={pdfDocument}
        pageNumber={pageNumber}
        scale={scale}
        rotate={rotation}
        renderTextLayer={true}
        renderAnnotationLayer={true}
        onLoadSuccess={(loadedPage: PDFPageProxy) => {
          const viewport = loadedPage.getViewport({ scale: 1, rotation: 0 });
          onPageSize?.(pageNumber, {
            width: viewport.width,
            height: viewport.height,
          });
        }}
        loading={
          <div
            className="rounded-md border border-border bg-surface-container"
            style={{
              width: basePageSize ? basePageSize.width * scale : 600,
              height: basePageSize ? basePageSize.height * scale : 800,
            }}
          />
        }
        className="overflow-hidden rounded-md shadow-sm ring-1 ring-border"
      />
    </div>
  );
}
