import { useMemo, type ReactElement } from "react";
import { buildDocumentSearchFixtureResults } from "../../../fixtures/data-room/documentSearch";
import DocumentSearch from "./DocumentSearch";
import {
  formatDocumentSearchFileName,
  isDocumentSearchResultAvailable,
  type DocumentSearchItem,
  type DocumentSearchResult,
} from "./documentSearchModel";

export type DataRoomDocumentSearchProps = {
  currentFileName: string;
  currentPageCount: number;
  finalFocusEl?: () => HTMLElement | null;
  onActivateResult: (result: DocumentSearchResult) => void;
  onOpenChange?: (open: boolean) => void;
  onSelectionFocus?: () => void;
  portalContainer?: HTMLElement | null;
  trigger?: ReactElement;
};

export function DataRoomDocumentSearch({
  currentFileName,
  currentPageCount,
  finalFocusEl,
  onActivateResult,
  onOpenChange,
  onSelectionFocus,
  portalContainer,
  trigger,
}: DataRoomDocumentSearchProps) {
  const results = useMemo(
    () => buildDocumentSearchFixtureResults(currentFileName),
    [currentFileName],
  );
  const items = useMemo<DocumentSearchItem[]>(
    () =>
      results.map((result) => {
        const available = isDocumentSearchResultAvailable({
          currentFileName,
          numPages: currentPageCount,
          result,
        });

        return {
          disabledReason: available ? undefined : "Preview navigation unavailable",
          id: result.id,
          primaryText: formatDocumentSearchFileName(result.fileName),
          searchText: `${result.fileName} ${result.location} ${result.excerpt}`,
          secondaryText: result.excerpt,
          tertiaryText: available
            ? `${result.location} · Open page ${result.target?.page}`
            : result.location,
        };
      }),
    [currentFileName, currentPageCount, results],
  );

  function activateItem(item: DocumentSearchItem) {
    const result = results.find((candidate) => candidate.id === item.id);

    if (result) {
      onActivateResult(result);
    }
  }

  return (
    <DocumentSearch
      buttonProps={{ "aria-label": "Search document", iconOnly: true }}
      dialogDescription={`Search local mock excerpts for ${currentFileName}.`}
      dialogTitle={`Search ${currentFileName}`}
      finalFocusEl={finalFocusEl}
      items={items}
      onOpenChange={onOpenChange}
      onSelect={activateItem}
      onSelectionFocus={onSelectionFocus}
      placeholder="Search files and excerpts…"
      portalContainer={portalContainer}
      trigger={trigger}
    />
  );
}
