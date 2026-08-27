"use client";

import { useCallback, useEffect, useRef } from "react";

interface UsePdfPageTrackerArgs {
  containerRef: { current: HTMLElement | null };
  numPages: number;
  ready: boolean;
  pageOffsets: readonly number[];
  pageHeights: readonly number[];
  contentPaddingTop: number;
  onPageChange: (page: number) => void;
}

export interface UsePdfPageTrackerReturn {
  scrollToPage: (page: number, behavior?: ScrollBehavior) => void;
}

export function pageAtOffset(
  pageOffsets: readonly number[],
  pageHeights: readonly number[],
  offset: number,
): number {
  if (pageOffsets.length === 0) return 1;
  let low = 0;
  let high = pageOffsets.length - 1;
  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    if (pageOffsets[middle] <= offset) {
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  const index = Math.max(0, Math.min(pageOffsets.length - 1, high));
  if (
    index + 1 < pageOffsets.length &&
    offset > pageOffsets[index] + (pageHeights[index] ?? 0)
  ) {
    return index + 2;
  }
  return index + 1;
}

/**
 * Tracks the current page from precomputed layout offsets. It does not query or
 * scan page DOM nodes, so virtualized documents can mount only a small range.
 */
export function usePdfPageTracker(
  args: UsePdfPageTrackerArgs,
): UsePdfPageTrackerReturn {
  const {
    containerRef,
    numPages,
    ready,
    pageOffsets,
    pageHeights,
    contentPaddingTop,
    onPageChange,
  } = args;
  const onPageChangeRef = useRef(onPageChange);
  onPageChangeRef.current = onPageChange;

  useEffect(() => {
    if (!ready || numPages <= 0) return;
    const container = containerRef.current;
    if (!container) return;

    let frame: number | null = null;
    const update = () => {
      frame = null;
      const probeOffset = Math.max(
        0,
        container.scrollTop + 80 - contentPaddingTop,
      );
      onPageChangeRef.current(
        pageAtOffset(pageOffsets, pageHeights, probeOffset),
      );
    };
    const onScroll = () => {
      if (frame != null) return;
      frame = requestAnimationFrame(update);
    };

    update();
    container.addEventListener("scroll", onScroll, { passive: true });
    const observer = new ResizeObserver(onScroll);
    observer.observe(container);
    return () => {
      container.removeEventListener("scroll", onScroll);
      observer.disconnect();
      if (frame != null) cancelAnimationFrame(frame);
    };
  }, [
    containerRef,
    numPages,
    ready,
    pageOffsets,
    pageHeights,
    contentPaddingTop,
  ]);

  const scrollToPage = useCallback(
    (page: number, behavior: ScrollBehavior = "smooth") => {
      const container = containerRef.current;
      if (!container || pageOffsets.length === 0) return;
      const clamped = Math.max(1, Math.min(numPages, Math.floor(page)));
      const top = contentPaddingTop + (pageOffsets[clamped - 1] ?? 0) - 8;
      container.scrollTo({ top: Math.max(0, top), behavior });
    },
    [containerRef, contentPaddingTop, numPages, pageOffsets],
  );

  return { scrollToPage };
}
