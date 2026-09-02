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
  leadingContent?: ReactNode;
  onPrintAction?: () => void;
  printActionLabel?: ReactNode;
  trailingContent?: ReactNode;
}

export function PdfToolbar({
  className,
  leadingContent,
  onPrintAction,
  printActionLabel,
  trailingContent,
}: PdfToolbarProps) {
  const { labels } = usePdfViewer();

  return (
    <TooltipProvider>
      <div
        role="toolbar"
        aria-label={labels.toolbarAriaLabel}
        className={cn(
          "grid min-h-12 min-w-0 grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-2 border-b border-border bg-card px-2 py-1.5",
          className,
        )}
      >
        <div className="min-w-0">{leadingContent}</div>
        <div className="flex shrink-0 items-center justify-center gap-1">
          <PdfPageNav />
          <PdfPageIndicator />
          <Separator orientation="vertical" className="mx-1 h-5" />
          <PdfZoomControls />
        </div>
        <div className="flex min-w-0 items-center justify-end gap-1">
          <PdfActionMenu
            onPrintAction={onPrintAction}
            printActionLabel={printActionLabel}
          />
          {trailingContent}
        </div>
      </div>
    </TooltipProvider>
  );
}
