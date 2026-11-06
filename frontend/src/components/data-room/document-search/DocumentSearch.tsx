import { XIcon } from "lucide-react";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactElement,
} from "react";
import { Button } from "@/components/ui/button";
import { getFloatingPanelInitialPosition } from "@/components/ui/floating-panel-position";
import {
  FloatingPanel,
  FloatingPanelCloseTrigger,
  FloatingPanelContent,
  FloatingPanelControl,
  FloatingPanelHeader,
  FloatingPanelTitle,
  FloatingPanelTrigger,
} from "@/components/ui/floating-panel";
import {
  type DocumentSearchButtonProps,
  DocumentSearchButton,
} from "./DocumentSearchButton";
import { SearchDialogContent } from "@/components/ui/stealth/search-dialog";
import { filterDocumentSearchItems, type DocumentSearchItem } from "./documentSearchModel";
import { useLiveDocumentSearch, type DocumentSearchRequest } from "./useLiveDocumentSearch";
import { useDocumentSearchKeyboardNavigation } from "./useDocumentSearchKeyboardNavigation";

export type DocumentSearchConfig = {
  buttonProps?: Omit<DocumentSearchButtonProps, "showShortcut">;
  buttonText?: string;
  dialogDescription?: string;
  dialogTitle?: string;
  enableKeyboardShortcut?: boolean;
  finalFocusEl?: () => HTMLElement | null;
  items?: DocumentSearchItem[];
  search?: DocumentSearchRequest;
  sourceLabel?: string;
  onOpenChange?: (open: boolean) => void;
  open?: boolean;
  hideTrigger?: boolean;
  showDescription?: boolean;
  onSelect: (item: DocumentSearchItem) => void;
  onSelectionFocus?: () => void;
  placeholder?: string;
  boundaryElement?: HTMLElement | null;
  trigger?: ReactElement;
};

const DOCUMENT_SEARCH_DEFAULT_SIZE = { width: 720, height: 440 };
const DOCUMENT_SEARCH_MIN_SIZE = { width: 320, height: 220 };

function getInitialDocumentSearchSize() {
  if (typeof window === "undefined") {
    return DOCUMENT_SEARCH_DEFAULT_SIZE;
  }

  return {
    width: Math.min(
      DOCUMENT_SEARCH_DEFAULT_SIZE.width,
      Math.max(window.innerWidth - 32, 1),
    ),
    height: Math.min(
      DOCUMENT_SEARCH_DEFAULT_SIZE.height,
      Math.max(window.innerHeight - 32, 1),
    ),
  };
}

/**
 * Stealth search-dialog contents inside the existing non-modal, resizable panel.
 * The feature retains ownership of filtering, selection and document navigation.
 */
export default function DocumentSearch({
  buttonProps,
  buttonText = "Search",
  dialogDescription = "Search document passages.",
  dialogTitle = "Search",
  enableKeyboardShortcut = false,
  finalFocusEl,
  items = [],
  search,
  sourceLabel = "Local results",
  onOpenChange,
  open: controlledOpen,
  hideTrigger = false,
  showDescription = false,
  onSelect,
  onSelectionFocus,
  placeholder = "What are you looking for?",
  boundaryElement,
  trigger,
}: DocumentSearchConfig) {
  const [internalOpen, setIsPanelOpen] = useState(false);
  const isPanelOpen = controlledOpen ?? internalOpen;
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const selectionFocusPending = useRef(false);
  const [panelSize] = useState(getInitialDocumentSearchSize);
  const filteredItems = useMemo(
    () => filterDocumentSearchItems(items, query),
    [items, query],
  );
  const live = useLiveDocumentSearch(search, query, isPanelOpen);
  const visibleItems = search ? live.items : query.trim() ? filteredItems : [];
  const { hoverIndex, moveDown, moveUp, selectedIndex, selectionOrigin } =
    useDocumentSearchKeyboardNavigation(visibleItems.map((item) => item.id));
  const selectedItem =
    selectedIndex >= 0 ? visibleItems[selectedIndex] : undefined;

  function setOpen(open: boolean) {
    if (open) selectionFocusPending.current = false;
    setIsPanelOpen(open);
    onOpenChange?.(open);
  }

  function selectItem(item: DocumentSearchItem | undefined) {
    if (!item || item.disabledReason) {
      return;
    }
    onSelect(item);
    selectionFocusPending.current = Boolean(onSelectionFocus);
    setOpen(false);
  }

  useEffect(() => {
    if (!enableKeyboardShortcut) {
      return;
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen(true);
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [enableKeyboardShortcut]);

  return (
    <FloatingPanel
      allowOverflow={false}
      closeOnEscape
      defaultSize={panelSize}
      defaultPosition={getFloatingPanelInitialPosition(panelSize, boundaryElement)}
      getBoundaryEl={() => boundaryElement ?? document.documentElement}
      initialFocusEl={() => inputRef.current}
      finalFocusEl={() => {
        if (selectionFocusPending.current && onSelectionFocus) {
          selectionFocusPending.current = false;
          onSelectionFocus();
          return document.activeElement instanceof HTMLElement ? document.activeElement : null;
        }
        return finalFocusEl?.() ?? null;
      }}
      minSize={{
        width: Math.min(DOCUMENT_SEARCH_MIN_SIZE.width, panelSize.width),
        height: Math.min(DOCUMENT_SEARCH_MIN_SIZE.height, panelSize.height),
      }}
      onOpenChange={({ open }) => setOpen(open)}
      open={isPanelOpen}
    >
      {!hideTrigger ? <FloatingPanelTrigger asChild>
        {trigger ?? (
          <DocumentSearchButton
            showShortcut={enableKeyboardShortcut}
            {...buttonProps}
          >
            {buttonText}
          </DocumentSearchButton>
        )}
      </FloatingPanelTrigger> : null}
      <FloatingPanelContent
        aria-describedby="local-search-description"
        className="max-w-[calc(100vw-2rem)]"
      >
        <FloatingPanelHeader>
          <FloatingPanelTitle>{dialogTitle}</FloatingPanelTitle>
          <FloatingPanelControl>
            <FloatingPanelCloseTrigger asChild>
              <Button
                aria-label="Close document search"
                size="icon-xs"
                type="button"
                variant="ghost"
              >
                <XIcon aria-hidden="true" />
              </Button>
            </FloatingPanelCloseTrigger>
          </FloatingPanelControl>
        </FloatingPanelHeader>
        <p className={showDescription ? "px-4 py-2 text-xs text-muted" : "sr-only"} id="local-search-description">
          {dialogDescription}
        </p>
        <div className="flex min-h-0 flex-1 flex-col">
          <SearchDialogContent
            activeItemId={selectedItem ? `local-search-result-${selectedItem.id}` : undefined}
            loading={live.loading}
            error={live.error}
            onRetry={live.retry}
            sourceLabel={sourceLabel}
            inputRef={inputRef}
            items={visibleItems}
            onArrowDown={moveDown}
            onArrowUp={moveUp}
            onEnter={() => selectItem(selectedItem)}
            onHoverIndex={hoverIndex}
            onSelect={selectItem}
            onQueryChange={setQuery}
            onClose={() => setOpen(false)}
            placeholder={placeholder}
            query={query}
            selectedIndex={selectedIndex}
            shouldScrollSelection={selectionOrigin === "keyboard"}
          />
        </div>
      </FloatingPanelContent>
    </FloatingPanel>
  );
}
