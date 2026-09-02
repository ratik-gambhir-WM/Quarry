import { ArrowDown, ArrowUp, CornerDownLeft, SearchIcon } from "lucide-react";
import {
  memo,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Dialog as DialogPrimitive } from "radix-ui";
import { Button } from "@/components/ui/button";
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
  items: DocumentSearchItem[];
  onOpenChange?: (open: boolean) => void;
  onSelect: (item: DocumentSearchItem) => void;
  onSelectionFocus?: () => void;
  placeholder?: string;
  portalContainer?: HTMLElement | null;
};

type SearchInputProps = {
  activeItemId?: string;
  inputRef: React.RefObject<HTMLInputElement | null>;
  onArrowDown: () => void;
  onArrowUp: () => void;
  onEnter: () => void;
  onQueryChange: (query: string) => void;
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
        <DialogPrimitive.Close asChild>
          <Button
            aria-label="Close document search"
            className="px-2 text-muted-foreground"
            type="button"
            variant="outline"
          >
            esc
          </Button>
        </DialogPrimitive.Close>
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
      className="flex h-[91vh] flex-col gap-4 overflow-y-auto bg-muted p-2 md:h-[50vh]"
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
    <div className="flex h-[91vh] flex-col items-center justify-center gap-2 bg-muted p-4 text-foreground md:h-[50vh]">
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
  items,
  onOpenChange,
  onSelect,
  onSelectionFocus,
  placeholder = "What are you looking for?",
  portalContainer,
}: DocumentSearchConfig) {
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const selectedItemRef = useRef(false);
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
    if (open) {
      selectedItemRef.current = false;
    }
    setIsModalOpen(open);
    onOpenChange?.(open);
  }

  function selectItem(item: DocumentSearchItem | undefined) {
    if (!item || item.disabledReason) {
      return;
    }
    selectedItemRef.current = true;
    onSelect(item);
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
    <DialogPrimitive.Root modal onOpenChange={setOpen} open={isModalOpen}>
      <DialogPrimitive.Trigger asChild>
        <DocumentSearchButton showShortcut={enableKeyboardShortcut} {...buttonProps}>
          {buttonText}
        </DocumentSearchButton>
      </DialogPrimitive.Trigger>
      <DialogPrimitive.Portal container={portalContainer}>
        <DialogPrimitive.Overlay className="pointer-events-auto absolute inset-0 z-50 flex items-start justify-center bg-black/50 backdrop-blur-sm dark:bg-black/60" />
        <DialogPrimitive.Content
          aria-describedby="local-search-description"
          className="pointer-events-auto absolute inset-x-0 top-0 z-50 h-full w-full max-w-full overflow-hidden bg-background shadow-2xl outline-none data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95 motion-reduce:animate-none md:inset-x-4 md:top-[10%] md:mx-auto md:h-auto md:max-h-[80%] md:max-w-[720px] md:rounded-xl"
          onCloseAutoFocus={(event) => {
            if (selectedItemRef.current && onSelectionFocus) {
              event.preventDefault();
              onSelectionFocus();
            }
          }}
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            inputRef.current?.focus();
          }}
        >
          <DialogPrimitive.Title className="sr-only">
            {dialogTitle}
          </DialogPrimitive.Title>
          <DialogPrimitive.Description className="sr-only" id="local-search-description">
            {dialogDescription}
          </DialogPrimitive.Description>
          <SearchInput
            activeItemId={
              selectedItem ? `local-search-result-${selectedItem.id}` : undefined
            }
            inputRef={inputRef}
            onArrowDown={moveDown}
            onArrowUp={moveUp}
            onEnter={() => selectItem(selectedItem)}
            onQueryChange={setQuery}
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
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
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
