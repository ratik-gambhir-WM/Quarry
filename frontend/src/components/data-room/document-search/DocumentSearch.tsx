import { ArrowDown, ArrowUp, CornerDownLeft, SearchIcon, XIcon } from "lucide-react";
import {
  memo,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactElement,
} from "react";
import { Button } from "@/components/ui/button";
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
import { getDocumentSearchQueryTerms } from "./DocumentSearchHighlightedText";
import { DocumentSearchResultCard } from "./DocumentSearchResultCard";
import type { DocumentSearchItem } from "./documentSearchModel";
import { useDocumentSearchKeyboardNavigation } from "./useDocumentSearchKeyboardNavigation";

export type DocumentSearchConfig = {
  buttonProps?: Omit<DocumentSearchButtonProps, "showShortcut">;
  buttonText?: string;
  dialogDescription?: string;
  dialogTitle?: string;
  enableKeyboardShortcut?: boolean;
  finalFocusEl?: () => HTMLElement | null;
  items: DocumentSearchItem[];
  onOpenChange?: (open: boolean) => void;
  onSelect: (item: DocumentSearchItem) => void;
  onSelectionFocus?: () => void;
  placeholder?: string;
  boundaryElement?: HTMLElement | null;
  trigger?: ReactElement;
};

type SearchInputProps = {
  activeItemId?: string;
  inputRef: React.RefObject<HTMLInputElement | null>;
  onArrowDown: () => void;
  onArrowUp: () => void;
  onEnter: () => void;
  onQueryChange: (query: string) => void;
  onClose: () => void;
  placeholder: string;
  query: string;
};

const SearchInput = memo(function SearchInput({
  activeItemId,
  inputRef,
  onArrowDown,
  onArrowUp,
  onEnter,
  onQueryChange,
  onClose,
  placeholder,
  query,
}: SearchInputProps) {
  return (
    <form
      className="flex flex-row items-center rounded-t-sm border-b border-muted bg-background p-2 placeholder:text-muted-foreground"
      onReset={(event) => {
        event.preventDefault();
        onQueryChange("");
        inputRef.current?.focus();
      }}
      onSubmit={(event) => event.preventDefault()}
      role="search"
    >
      <span
        aria-hidden="true"
        className="flex items-center justify-center rounded-full p-2 text-muted-foreground transition-colors peer-focus:text-primary"
      >
        <SearchIcon strokeWidth={1.5} />
      </span>
      <input
        aria-activedescendant={activeItemId}
        aria-controls="local-search-results"
        aria-label="Search document"
        autoCapitalize="off"
        autoComplete="off"
        autoCorrect="off"
        className="peer min-w-0 flex-1 bg-transparent text-xl font-light text-foreground outline-none"
        inputMode="search"
        maxLength={512}
        onChange={(event) => onQueryChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            event.preventDefault();
            onArrowDown();
          } else if (event.key === "ArrowUp") {
            event.preventDefault();
            onArrowUp();
          } else if (event.key === "Enter") {
            event.preventDefault();
            onEnter();
          }
        }}
        placeholder={placeholder}
        ref={inputRef}
        spellCheck={false}
        type="search"
        value={query}
      />
      <span className="ml-auto flex items-center gap-2">
        {query ? (
          <Button className="px-2 text-muted-foreground" type="reset" variant="ghost">
            Clear
          </Button>
        ) : null}
        <Button
          aria-label="Close document search"
          className="px-2 text-muted-foreground"
          onClick={onClose}
          type="button"
          variant="outline"
        >
          esc
        </Button>
      </span>
    </form>
  );
});

type HitsListProps = {
  items: DocumentSearchItem[];
  onHoverIndex: (index: number) => void;
  onSelect: (item: DocumentSearchItem) => void;
  query: string;
  selectedIndex: number;
  shouldScrollSelection: boolean;
};

const HitsList = memo(function HitsList({
  items,
  onHoverIndex,
  onSelect,
  query,
  selectedIndex,
  shouldScrollSelection,
}: HitsListProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!shouldScrollSelection) {
      return;
    }
    const container = containerRef.current;
    const selected = container?.querySelector<HTMLElement>("[aria-selected='true']");
    selected?.scrollIntoView?.({ block: "nearest" });
  }, [selectedIndex, shouldScrollSelection]);

  return (
    <div
      className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto bg-muted p-2"
      id="local-search-results"
      ref={containerRef}
      role="listbox"
    >
      {items.map((item, index) => (
        <DocumentSearchResultCard
          index={index}
          item={item}
          key={item.id}
          onHover={onHoverIndex}
          onSelect={onSelect}
          query={query}
          selected={selectedIndex === index}
        />
      ))}
    </div>
  );
});

const NoResults = memo(function NoResults({
  onClear,
  query,
}: {
  onClear: () => void;
  query: string;
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 bg-muted p-4 text-foreground">
      <span className="flex h-10 w-10 items-center justify-center rounded-full border border-muted-foreground p-2">
        <SearchIcon aria-hidden="true" />
      </span>
      <p className="text-lg font-medium">No results for &quot;{query}&quot;</p>
      <p className="text-sm text-muted-foreground">Try a different query.</p>
      <Button onClick={onClear} variant="outline">
        Clear query
      </Button>
    </div>
  );
});

const Footer = memo(function Footer() {
  return (
    <div className="flex items-center justify-between rounded-b-sm bg-background p-4">
      <div className="inline-flex items-center gap-4 text-sm">
        <span className="flex items-center gap-2">
          <kbd className="flex h-6 items-center justify-center rounded-sm bg-muted p-1 text-muted-foreground">
            <CornerDownLeft aria-hidden="true" size={20} />
          </kbd>
          <span className="text-muted-foreground">Open</span>
        </span>
        <span className="flex items-center gap-2">
          <kbd className="flex h-6 items-center justify-center rounded-sm bg-muted p-1 text-muted-foreground">
            <ArrowUp aria-hidden="true" size={20} />
          </kbd>
          <kbd className="flex h-6 items-center justify-center rounded-sm bg-muted p-1 text-muted-foreground">
            <ArrowDown aria-hidden="true" size={20} />
          </kbd>
          <span className="text-muted-foreground">Navigate</span>
        </span>
      </div>
    </div>
  );
});

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
 * Installed from the @algolia/search shadcn registry item, then adapted to accept
 * local items. It intentionally has no Algolia client, credentials, analytics,
 * branding, or network behavior.
 */
export default function DocumentSearch({
  buttonProps,
  buttonText = "Search",
  dialogDescription = "Search local mock results.",
  dialogTitle = "Search",
  enableKeyboardShortcut = false,
  finalFocusEl,
  items,
  onOpenChange,
  onSelect,
  onSelectionFocus,
  placeholder = "What are you looking for?",
  boundaryElement,
  trigger,
}: DocumentSearchConfig) {
  const [isPanelOpen, setIsPanelOpen] = useState(false);
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const [panelSize] = useState(getInitialDocumentSearchSize);
  const filteredItems = useMemo(
    () => filterDocumentSearchItems(items, query),
    [items, query],
  );
  const visibleItems = query.trim() ? filteredItems : [];
  const {
    hoverIndex,
    moveDown,
    moveUp,
    selectedIndex,
    selectionOrigin,
  } = useDocumentSearchKeyboardNavigation(visibleItems.map((item) => item.id));
  const selectedItem = selectedIndex >= 0 ? visibleItems[selectedIndex] : undefined;

  function setOpen(open: boolean) {
    setIsPanelOpen(open);
    onOpenChange?.(open);
  }

  function selectItem(item: DocumentSearchItem | undefined) {
    if (!item || item.disabledReason) {
      return;
    }
    onSelect(item);
    setOpen(false);
    if (onSelectionFocus) {
      queueMicrotask(onSelectionFocus);
    }
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
      getAnchorPosition={({ boundaryRect }) => {
        const boundaryX = boundaryRect?.x ?? 0;
        const boundaryY = boundaryRect?.y ?? 0;
        const boundaryWidth = boundaryRect?.width ?? window.innerWidth;
        const boundaryHeight = boundaryRect?.height ?? window.innerHeight;

        return {
          x: Math.max(boundaryX, boundaryX + (boundaryWidth - panelSize.width) / 2),
          y: Math.max(boundaryY, boundaryY + (boundaryHeight - panelSize.height) / 2),
        };
      }}
      getBoundaryEl={() => boundaryElement ?? document.documentElement}
      initialFocusEl={() => inputRef.current}
      finalFocusEl={finalFocusEl}
      minSize={{
        width: Math.min(DOCUMENT_SEARCH_MIN_SIZE.width, panelSize.width),
        height: Math.min(DOCUMENT_SEARCH_MIN_SIZE.height, panelSize.height),
      }}
      onOpenChange={({ open }) => setOpen(open)}
      open={isPanelOpen}
    >
      <FloatingPanelTrigger asChild>
        {trigger ?? (
          <DocumentSearchButton showShortcut={enableKeyboardShortcut} {...buttonProps}>
            {buttonText}
          </DocumentSearchButton>
        )}
      </FloatingPanelTrigger>
      <FloatingPanelContent
        aria-describedby="local-search-description"
        className="max-w-[calc(100vw-2rem)] overflow-hidden"
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
        <p className="sr-only" id="local-search-description">
          {dialogDescription}
        </p>
        <div className="flex min-h-0 flex-1 flex-col">
          <SearchInput
            activeItemId={
              selectedItem ? `local-search-result-${selectedItem.id}` : undefined
            }
            inputRef={inputRef}
            onArrowDown={moveDown}
            onArrowUp={moveUp}
            onEnter={() => selectItem(selectedItem)}
            onQueryChange={setQuery}
            onClose={() => setOpen(false)}
            placeholder={placeholder}
            query={query}
          />
          {query.trim() && visibleItems.length > 0 ? (
            <HitsList
              items={visibleItems}
              onHoverIndex={hoverIndex}
              onSelect={selectItem}
              query={query}
              selectedIndex={selectedIndex}
              shouldScrollSelection={selectionOrigin === "keyboard"}
            />
          ) : null}
          {query.trim() && visibleItems.length === 0 ? (
            <NoResults
              onClear={() => {
                setQuery("");
                inputRef.current?.focus();
              }}
              query={query}
            />
          ) : null}
          <Footer />
        </div>
      </FloatingPanelContent>
    </FloatingPanel>
  );
}

export function filterDocumentSearchItems(
  items: DocumentSearchItem[],
  query: string,
): DocumentSearchItem[] {
  const terms = getDocumentSearchQueryTerms(query);
  if (terms.length === 0) {
    return items;
  }
  return items.filter((item) => {
    const searchable = [
      item.primaryText,
      item.secondaryText,
      item.tertiaryText,
      item.searchText,
    ]
      .filter((value): value is string => Boolean(value))
      .join(" ")
      .toLocaleLowerCase();
    return terms.every((term) => searchable.includes(term));
  });
}
