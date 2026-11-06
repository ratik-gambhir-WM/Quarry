import { useCallback, type ReactElement } from "react";
import { runtime } from "@quarry/runtime";
import { keywordSearchExcerpt, keywordSearchMatchRanges } from "@/data/documents/keywordSearch";
import DocumentSearch from "./DocumentSearch";
import type { DocumentSearchItem, DocumentSearchResult } from "./documentSearchModel";

export type DataRoomDocumentSearchProps = {
  dealId: string;
  currentFileId?: string;
  currentFileName: string;
  currentPageCount: number;
  files?: { fileId: string; name: string }[];
  finalFocusEl?: () => HTMLElement | null;
  onActivateResult: (result: DocumentSearchResult) => void;
  onOpenChange?: (open: boolean) => void;
  open?: boolean;
  hideTrigger?: boolean;
  dialogTitle?: string;
  onSelectionFocus?: () => void;
  boundaryElement?: HTMLElement | null;
  trigger?: ReactElement;
};

export function DataRoomDocumentSearch({
  dealId, currentFileId, currentFileName, currentPageCount, files,
  onActivateResult, onOpenChange, open, hideTrigger, dialogTitle,
  onSelectionFocus, finalFocusEl, boundaryElement, trigger,
}: DataRoomDocumentSearchProps) {
  const search = useCallback(async (query: string): Promise<DocumentSearchItem[]> => {
    const hits = await runtime.api.searchDealDocumentChunksByKeyword(dealId, {
      queryText: query,
      limit: 30,
    });
    return hits.map((hit) => {
      const fileName = files?.find((file) => file.fileId === hit.file_id)?.name
        ?? (hit.file_id === currentFileId ? currentFileName : "Uploaded document");
      const page = hit.page_start;
      const available = hit.file_id === currentFileId && page !== null
        && page >= 1 && page <= currentPageCount;
      const location = page === null ? `Passage ${hit.chunk_index}`
        : hit.page_end !== null && hit.page_end !== page
          ? `Pages ${page}–${hit.page_end}` : `Page ${page}`;
      const excerpt = keywordSearchExcerpt(hit.text, query);
      let offset = 0;
      const secondaryMatchRanges: [number, number][] = [];
      for (const part of excerpt) {
        if (part.matched) secondaryMatchRanges.push([offset, offset + part.text.length]);
        offset += part.text.length;
      }
      return {
        id: hit.chunk_id,
        fileName,
        primaryText: fileName,
        primaryMatchRanges: keywordSearchMatchRanges(fileName, query),
        secondaryText: excerpt.map((part) => part.text).join(""),
        secondaryMatchRanges,
        tertiaryText: [hit.section_path.trim(), location].filter(Boolean).join(" · "),
        target: available ? { kind: "pdf-page", page } : undefined,
        disabledReason: available ? undefined : hit.file_id !== currentFileId
          ? "Open this document in the Data Room to navigate to this passage."
          : "Page navigation unavailable for this passage.",
      };
    });
  }, [dealId, currentFileId, currentFileName, currentPageCount, files]);

  function activateItem(item: DocumentSearchItem) {
    if (!item.target || item.disabledReason) return;
    onActivateResult({
      id: item.id,
      fileName: item.fileName ?? item.primaryText,
      excerpt: item.secondaryText ?? "",
      location: item.tertiaryText ?? "",
      target: item.target,
    });
  }

  return (
    <DocumentSearch
      buttonProps={{ "aria-label": "Search document", iconOnly: true }}
      dialogDescription="Keyword search across uploaded documents in this deal. Results update as you type."
      dialogTitle={dialogTitle ?? "Search documents"}
      open={open}
      hideTrigger={hideTrigger}
      finalFocusEl={finalFocusEl}
      search={search}
      sourceLabel="This deal · Keyword"
      onOpenChange={onOpenChange}
      onSelect={activateItem}
      onSelectionFocus={onSelectionFocus}
      placeholder="Search files and excerpts…"
      boundaryElement={boundaryElement}
      trigger={trigger}
    />
  );
}
