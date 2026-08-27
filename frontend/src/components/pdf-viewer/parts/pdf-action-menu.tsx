"use client";

import type { ReactNode } from "react";
import { Download, FileText, MoreVertical, Printer, RotateCw } from "lucide-react";
import { buttonVariants } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { usePdfViewer } from "../hooks/use-pdf-viewer-context";

interface PdfActionMenuProps {
  /** Force compact mode (overflow). Default: respects viewer's `compact` flag. */
  compact?: boolean;
  className?: string;
  onPrintAction?: () => void;
  printActionLabel?: ReactNode;
}

export function PdfActionMenu({
  compact,
  className,
  onPrintAction,
  printActionLabel,
}: PdfActionMenuProps) {
  const ctx = usePdfViewer();
  const { actions, labels, status, allowDownload, allowPrint } = ctx;
  const useCompact = compact ?? ctx.compact;
  const disabled = status !== "ready";
  const showPrintAction = allowPrint || Boolean(onPrintAction);
  const resolvedPrintAction = onPrintAction ?? actions.print;
  const resolvedPrintLabel = printActionLabel ?? labels.print;

  // F-cross-13: no `asChild` — triggers render their own <button> in both
  // backends; Button look inlined via buttonVariants.
  if (useCompact) {
    return (
      <DropdownMenu>
        <DropdownMenuTrigger
          type="button"
          disabled={disabled}
          aria-label={typeof labels.more === "string" ? labels.more : "More actions"}
          className={cn(
            buttonVariants({ variant: "ghost", size: "icon-sm" }),
            className,
          )}
        >
          <MoreVertical />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={() => actions.rotate()}>
            <RotateCw aria-hidden="true" />
            {labels.rotate}
          </DropdownMenuItem>
          {allowDownload ? (
            <DropdownMenuItem onSelect={() => actions.download()}>
              <Download aria-hidden="true" />
              {labels.download}
            </DropdownMenuItem>
          ) : null}
          {showPrintAction ? (
            <DropdownMenuItem onSelect={resolvedPrintAction}>
              {onPrintAction ? (
                <FileText aria-hidden="true" />
              ) : (
                <Printer aria-hidden="true" />
              )}
              {resolvedPrintLabel}
            </DropdownMenuItem>
          ) : null}
        </DropdownMenuContent>
      </DropdownMenu>
    );
  }

  return (
    <div className={cn("flex items-center gap-0.5", className)}>
      <Tooltip>
        <TooltipTrigger
          type="button"
          className={buttonVariants({ variant: "ghost", size: "icon-sm" })}
          onClick={() => actions.rotate()}
          disabled={disabled}
          aria-label={typeof labels.rotate === "string" ? labels.rotate : "Rotate"}
        >
          <RotateCw />
        </TooltipTrigger>
        <TooltipContent>{labels.rotate}</TooltipContent>
      </Tooltip>
      {allowDownload ? (
        <Tooltip>
          <TooltipTrigger
            type="button"
            className={buttonVariants({ variant: "ghost", size: "icon-sm" })}
            onClick={() => actions.download()}
            disabled={disabled}
            aria-label={typeof labels.download === "string" ? labels.download : "Download"}
          >
            <Download />
          </TooltipTrigger>
          <TooltipContent>{labels.download}</TooltipContent>
        </Tooltip>
      ) : null}
      {showPrintAction ? (
        <Tooltip>
          <TooltipTrigger
            type="button"
            className={buttonVariants({ variant: "ghost", size: "icon-sm" })}
            onClick={resolvedPrintAction}
            disabled={disabled}
            aria-label={
              typeof resolvedPrintLabel === "string"
                ? resolvedPrintLabel
                : "Document action"
            }
          >
            {onPrintAction ? <FileText /> : <Printer />}
          </TooltipTrigger>
          <TooltipContent>{resolvedPrintLabel}</TooltipContent>
        </Tooltip>
      ) : null}
    </div>
  );
}
