"use client";

import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import type { ResolvedPdfViewerLabels } from "../types";

interface PdfLoadingStateProps {
  labels: ResolvedPdfViewerLabels;
  className?: string;
}

export function PdfLoadingState({ labels, className }: PdfLoadingStateProps) {
  return (
    <div
      className={cn(
        "flex min-h-full w-full justify-center overflow-hidden bg-surface-container p-4 sm:p-8",
        className,
      )}
      aria-busy="true"
      aria-live="polite"
      role="status"
    >
      <span className="sr-only">{labels.loading}</span>
      <div
        aria-hidden="true"
        className="aspect-[8.5/11] w-full overflow-hidden rounded-sm border border-outline-variant bg-surface-container-lowest p-8 shadow-sm sm:p-14"
      >
        <Skeleton className="h-5 w-2/5 motion-reduce:animate-none" />
        <div className="mt-10 space-y-4">
          <Skeleton className="h-3 w-full motion-reduce:animate-none" />
          <Skeleton className="h-3 w-[94%] motion-reduce:animate-none" />
          <Skeleton className="h-3 w-[97%] motion-reduce:animate-none" />
          <Skeleton className="h-3 w-4/5 motion-reduce:animate-none" />
        </div>
        <div className="mt-10 space-y-4">
          <Skeleton className="h-4 w-1/3 motion-reduce:animate-none" />
          <Skeleton className="h-3 w-full motion-reduce:animate-none" />
          <Skeleton className="h-3 w-[92%] motion-reduce:animate-none" />
          <Skeleton className="h-3 w-[96%] motion-reduce:animate-none" />
          <Skeleton className="h-3 w-3/4 motion-reduce:animate-none" />
        </div>
        <div className="mt-10 grid grid-cols-2 gap-5">
          <Skeleton className="h-24 w-full motion-reduce:animate-none" />
          <Skeleton className="h-24 w-full motion-reduce:animate-none" />
        </div>
      </div>
    </div>
  );
}
