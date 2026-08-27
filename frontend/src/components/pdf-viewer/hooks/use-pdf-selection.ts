"use client";

import { useEffect, useRef, useState } from "react";

interface UsePdfSelectionArgs {
  rootRef: { current: HTMLElement | null };
  onSelection?: (args: { text: string }) => void;
}

/**
 * Tracks selected text inside the viewer. Debounced 150ms so we don't
 * fire on every mousemove during drag-select.
 */
export function usePdfSelection(args: UsePdfSelectionArgs): string {
  const { rootRef, onSelection } = args;
  const [selectedText, setSelectedText] = useState("");
  const selectedTextRef = useRef("");

  useEffect(() => {
    const root = rootRef.current;
    if (!root) return;

    let timeout: ReturnType<typeof setTimeout> | null = null;
    const updateSelectedText = (text: string) => {
      if (selectedTextRef.current === text) return;
      selectedTextRef.current = text;
      setSelectedText(text);
      onSelection?.({ text });
    };

    const onSelectionChange = () => {
      if (timeout != null) clearTimeout(timeout);
      timeout = setTimeout(() => {
        const sel = typeof window !== "undefined" ? window.getSelection() : null;
        if (!sel || sel.rangeCount === 0) {
          updateSelectedText("");
          return;
        }
        const range = sel.getRangeAt(0);
        const inViewer =
          root === range.commonAncestorContainer ||
          (range.commonAncestorContainer instanceof Node &&
            root.contains(range.commonAncestorContainer));
        if (!inViewer) {
          updateSelectedText("");
          return;
        }
        const text = sel.toString();
        updateSelectedText(text);
      }, 150);
    };

    document.addEventListener("selectionchange", onSelectionChange);
    return () => {
      document.removeEventListener("selectionchange", onSelectionChange);
      if (timeout != null) clearTimeout(timeout);
    };
  }, [rootRef, onSelection]);

  return selectedText;
}
