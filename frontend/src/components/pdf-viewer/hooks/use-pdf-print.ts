"use client";

import { useCallback, useRef, useState } from "react";
import type { PDFDocumentProxy } from "pdfjs-dist";

const MAX_PRINT_BYTES = 64 * 1024 * 1024;
const MAX_PRINT_PAGES = 1_000;
const PRINT_LOAD_TIMEOUT_MS = 15_000;
const PRINT_CLEANUP_TIMEOUT_MS = 60_000;

interface UsePdfPrintArgs {
  pdfDocument: PDFDocumentProxy | null;
}

export interface UsePdfPrintReturn {
  print: () => Promise<void>;
  isPrinting: boolean;
}

/**
 * Prints the original PDF bytes in a temporary same-origin blob iframe.
 * This preserves vector content and avoids retaining one high-DPI PNG for
 * every page, which can exhaust renderer memory on large documents.
 */
export function usePdfPrint(args: UsePdfPrintArgs): UsePdfPrintReturn {
  const { pdfDocument } = args;
  const [isPrinting, setIsPrinting] = useState(false);
  const printingRef = useRef(false);

  const print = useCallback(async () => {
    if (printingRef.current || !pdfDocument) return;
    if (pdfDocument.numPages > MAX_PRINT_PAGES) {
      throw new Error(
        `This PDF has ${pdfDocument.numPages} pages; printing is limited to ${MAX_PRINT_PAGES} pages.`,
      );
    }

    printingRef.current = true;
    setIsPrinting(true);
    let iframe: HTMLIFrameElement | null = null;
    let objectUrl: string | null = null;
    let cleanedUp = false;

    const cleanup = () => {
      if (cleanedUp) return;
      cleanedUp = true;
      if (iframe?.parentNode) iframe.parentNode.removeChild(iframe);
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      printingRef.current = false;
      setIsPrinting(false);
    };

    try {
      const data = await pdfDocument.getData();
      if (data.byteLength > MAX_PRINT_BYTES) {
        throw new Error(
          `This PDF is ${data.byteLength} bytes; printing is limited to ${MAX_PRINT_BYTES} bytes.`,
        );
      }

      const blob = new Blob([new Uint8Array(data)], {
        type: "application/pdf",
      });
      objectUrl = URL.createObjectURL(blob);
      iframe = document.createElement("iframe");
      iframe.setAttribute("aria-hidden", "true");
      iframe.style.position = "fixed";
      iframe.style.right = "0";
      iframe.style.bottom = "0";
      iframe.style.width = "0";
      iframe.style.height = "0";
      iframe.style.border = "0";
      iframe.style.opacity = "0";
      iframe.style.pointerEvents = "none";

      await new Promise<void>((resolve, reject) => {
        if (!iframe || !objectUrl) {
          reject(new Error("Unable to create the PDF print frame."));
          return;
        }
        const timeout = window.setTimeout(() => {
          reject(new Error("Timed out while preparing the PDF for printing."));
        }, PRINT_LOAD_TIMEOUT_MS);
        iframe.onload = () => {
          window.clearTimeout(timeout);
          resolve();
        };
        iframe.onerror = () => {
          window.clearTimeout(timeout);
          reject(new Error("The browser could not load the PDF for printing."));
        };
        iframe.src = objectUrl;
        document.body.appendChild(iframe);
      });

      const win = iframe.contentWindow;
      if (!win) throw new Error("The PDF print frame is unavailable.");
      const onAfterPrint = () => {
        win.removeEventListener("afterprint", onAfterPrint);
        cleanup();
      };
      win.addEventListener("afterprint", onAfterPrint);
      win.focus();
      win.print();
      window.setTimeout(cleanup, PRINT_CLEANUP_TIMEOUT_MS);
    } catch (error) {
      cleanup();
      throw error;
    }
  }, [pdfDocument]);

  return { print, isPrinting };
}
