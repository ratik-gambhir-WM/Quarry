import { useCallback, useSyncExternalStore, type ReactElement } from "react";
import { sessionStore } from "@/auth/sessionStore";
import type { HybridDocumentHit } from "@/contracts/quarryApi";
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

export function DataRoomDocumentSearch(props: DataRoomDocumentSearchProps) {
  const session = useSyncExternalStore(sessionStore.subscribe, sessionStore.getSnapshot, sessionStore.getSnapshot);
  return <ScopedDataRoomDocumentSearch key={`${session.generation}:${props.dealId}`} {...props} />;
}

function ScopedDataRoomDocumentSearch({
  dealId, currentFileId, currentFileName, currentPageCount, files,
  onActivateResult, onOpenChange, open, hideTrigger, dialogTitle,
  onSelectionFocus, finalFocusEl, boundaryElement, trigger,
}: DataRoomDocumentSearchProps) {
  const mapHits = useCallback((hits: HybridDocumentHit[], query: string): DocumentSearchItem[] => {
    return hits.map((hit) => {
      const fileName = files?.find((file) => file.fileId === hit.fileId)?.name
        ?? (hit.fileId === currentFileId ? currentFileName : "Uploaded document");
      const page = hit.pageStart;
      const available = hit.fileId === currentFileId && page !== null
        && page >= 1 && page <= currentPageCount;
      const location = page === null ? `Passage ${hit.chunkIndex}`
        : hit.pageEnd !== null && hit.pageEnd !== page
          ? `Pages ${page}–${hit.pageEnd}` : `Page ${page}`;
      const excerpt = keywordSearchExcerpt(hit.text, query);
      let offset = 0;
      const secondaryMatchRanges: [number, number][] = [];
      for (const part of excerpt) {
        if (part.matched) secondaryMatchRanges.push([offset, offset + part.text.length]);
        offset += part.text.length;
      }
      return {
        id: hit.chunkId,
        matchSources: hit.matchSources,
        fileName,
        primaryText: fileName,
        primaryMatchRanges: keywordSearchMatchRanges(fileName, query),
        secondaryText: excerpt.map((part) => part.text).join(""),
        secondaryMatchRanges,
        tertiaryText: [hit.sectionPath.trim(), location].filter(Boolean).join(" · "),
        target: available ? { kind: "pdf-page", page } : undefined,
        disabledReason: available ? undefined : hit.fileId !== currentFileId
          ? "Open this document in the Data Room to navigate to this passage."
          : "Page navigation unavailable for this passage.",
      };
    });
  }, [currentFileId, currentFileName, currentPageCount, files]);

  const search = useCallback(async (query: string, signal?: AbortSignal) => {
    const { results } = await runtime.api.searchDealDocumentChunksHybrid(dealId, { queryText: query, limit: 30 }, signal);
    return mapHits(results, query);
  }, [dealId, mapHits]);
  const keywordFallback = useCallback(async (query: string) => {
    const hits = await runtime.api.searchDealDocumentChunksByKeyword(dealId, { queryText: query, limit: 30 });
    return mapHits(hits.map((hit): HybridDocumentHit => ({
      chunkId: hit.chunk_id, dealId: hit.deal_id, fileId: hit.file_id, versionId: hit.version_id,
      text: hit.text, chunkIndex: hit.chunk_index, pageStart: hit.page_start, pageEnd: hit.page_end,
      sectionPath: hit.section_path, matchSources: ["keyword"],
    })), query);
  }, [dealId, mapHits]);

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
      dialogDescription="Hybrid search finds keyword and semantic matches across uploaded passages in this deal. Press Enter or Search to submit."
      dialogTitle={dialogTitle ?? "Search documents"}
      open={open}
      hideTrigger={hideTrigger}
      finalFocusEl={finalFocusEl}
      search={search}
      submitSearch
      filterByMatchSource
      keywordFallback={keywordFallback}
      sourceLabel="This deal · Hybrid"
      onOpenChange={onOpenChange}
      onSelect={activateItem}
      onSelectionFocus={onSelectionFocus}
      placeholder="Ask a question or search passages…"
      boundaryElement={boundaryElement}
      trigger={trigger}
    />
  );
}
