"use client";

import type { ReactNode } from "react";
import { Separator } from "@/components/ui/separator";
import { TooltipProvider } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { usePdfViewer } from "../hooks/use-pdf-viewer-context";
import { PdfActionMenu } from "./pdf-action-menu";
import { PdfPageIndicator } from "./pdf-page-indicator";
import { PdfPageNav } from "./pdf-page-nav";
import { PdfZoomControls } from "./pdf-zoom-controls";

interface PdfToolbarProps {
  className?: string;
  onPrintAction?: () => void;
  printActionLabel?: ReactNode;
}

export function PdfToolbar({
  className,
  onPrintAction,
  printActionLabel,
}: PdfToolbarProps) {
  const { labels } = usePdfViewer();

  return (
    <TooltipProvider>
      <div
        role="toolbar"
        aria-label={labels.toolbarAriaLabel}
        className={cn(
          "flex items-center gap-1 border-b border-border bg-card px-2 py-1.5",
          className,
        )}
      >
        <PdfPageNav />
        <PdfPageIndicator />
        <Separator orientation="vertical" className="mx-1 h-5" />
        <PdfZoomControls />
        <Separator orientation="vertical" className="mx-1 h-5" />
        <div className="ml-auto flex items-center gap-0.5">
          <PdfActionMenu
            onPrintAction={onPrintAction}
            printActionLabel={printActionLabel}
          />
        </div>
      </div>
    </TooltipProvider>
  );
}
