import { useEffect, useRef, type RefObject } from "react";
import { ArrowDown, ArrowUp, CornerDownLeft, FileText, Search, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { HighlightMatch } from "./highlight-match";
import type { MatchRange } from "./highlight-match-model";

type SearchDialogItem = {
  id: string;
  primaryText: string;
  primaryMatchRanges?: readonly MatchRange[];
  secondaryText?: string;
  secondaryMatchRanges?: readonly MatchRange[];
  tertiaryText?: string;
  disabledReason?: string;
  matchSources?: ("keyword" | "semantic")[];
};

type MatchSourceFilter = "all" | "keyword" | "semantic";
const MATCH_SOURCE_FILTERS = [
  { value: "all", label: "All results" },
  { value: "keyword", label: "Keyword" },
  { value: "semantic", label: "Semantic" },
] as const;

/** Adapted from https://www.stealth.pm/r/search-dialog.json: compact results,
 * preview and keyboard footer, hosted by Quarry's non-modal floating panel.
 * The feature owns filtering and activation; no registry-wide tokens or backdrop. */
export function SearchDialogContent<T extends SearchDialogItem>({
  activeItemId, inputRef, items, onArrowDown, onArrowUp, onEnter, onHoverIndex,
  onSelect, onQueryChange, onClose, placeholder, query, selectedIndex, shouldScrollSelection,
  loading = false, error, onRetry, sourceLabel = "Local results",
  onSubmitSearch, awaitingSubmit = false, onKeywordFallback, resultFilter,
}: {
  resultFilter?: {
    value: MatchSourceFilter;
    counts: Record<MatchSourceFilter, number>;
    onChange: (value: MatchSourceFilter) => void;
  };
  onSubmitSearch?: () => void;
  awaitingSubmit?: boolean;
  onKeywordFallback?: () => void;
  loading?: boolean;
  error?: string;
  onRetry?: () => void;
  sourceLabel?: string;
  activeItemId?: string;
  inputRef: RefObject<HTMLInputElement | null>;
  items: T[];
  onArrowDown: () => void;
  onArrowUp: () => void;
  onEnter: () => void;
  onHoverIndex: (index: number) => void;
  onSelect: (item: T) => void;
  onQueryChange: (query: string) => void;
  onClose: () => void;
  placeholder: string;
  query: string;
  selectedIndex: number;
  shouldScrollSelection: boolean;
}) {
  const listRef = useRef<HTMLDivElement>(null);
  const selected = items[selectedIndex];
  const hasQuery = Boolean(query.trim());
  const showCounts = hasQuery && !awaitingSubmit && !loading && !error;
  const emptySource = resultFilter && resultFilter.value !== "all" && resultFilter.counts.all > 0
    ? resultFilter.value : undefined;
  const resultType = resultFilter && resultFilter.value !== "all" ? `${resultFilter.value} ` : "";
  useEffect(() => {
    if (shouldScrollSelection) listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [selectedIndex, shouldScrollSelection]);
  function clear() { onQueryChange(""); inputRef.current?.focus(); }
  return (
    <div className="@container flex min-h-0 flex-1 flex-col overflow-hidden rounded-b-2xl">
      <form role="search" data-slot="search-input-row" className="flex h-12 shrink-0 items-center gap-2.5 border-b border-outline-variant px-4" onSubmit={(event) => { event.preventDefault(); onSubmitSearch?.(); }}>
        <Search aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
        <input autoFocus ref={inputRef} aria-label="Search document" aria-controls="local-search-results" aria-activedescendant={activeItemId}
          autoComplete="off" autoCapitalize="off" spellCheck={false} enterKeyHint="search" type="search" maxLength={512}
          placeholder={placeholder} value={query} onChange={(event) => onQueryChange(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") { event.preventDefault(); onArrowDown(); }
            else if (event.key === "ArrowUp") { event.preventDefault(); onArrowUp(); }
            else if (event.key === "Enter" && !event.nativeEvent.isComposing) { event.preventDefault(); onEnter(); }
          }}
          className="h-9 min-w-0 flex-1 rounded-sm bg-transparent text-[13px] text-foreground outline-none placeholder:text-muted-foreground [&::-webkit-search-cancel-button]:appearance-none" />
        {query && <Button aria-label="Clear search" size="icon-xs" variant="ghost" onClick={clear} type="button"><X aria-hidden="true" className="size-3.5" /></Button>}
        {onSubmitSearch && <Button type="submit" size="sm" disabled={!hasQuery || loading}>Search</Button>}
        <Button aria-label="Close document search" size="sm" variant="ghost" onClick={onClose} type="button"><Kbd className="border border-outline-variant bg-surface-container-low text-on-surface-variant">esc</Kbd></Button>
      </form>
      <div className="flex min-h-10 shrink-0 flex-wrap items-center gap-2 border-b border-outline-variant px-4 py-1.5 text-xs">
        {resultFilter ? <div role="group" aria-label="Filter results by match type" className="flex flex-wrap gap-1">
          {MATCH_SOURCE_FILTERS.map(({ value, label }) => <Button
            key={value} type="button" size="sm" variant="ghost"
            aria-pressed={resultFilter.value === value} disabled={!showCounts}
            className="px-2 text-xs aria-pressed:bg-foreground/7"
            onClick={() => resultFilter.onChange(value)}
          >
            {label}
            {showCounts && <span className="ml-1 font-mono tabular-nums text-muted-foreground">{resultFilter.counts[value]}</span>}
          </Button>)}
        </div> : <>
          <span className="rounded-md bg-foreground/7 px-2.5 py-1 font-medium">All results</span>
          {showCounts && <span className="font-mono tabular-nums text-muted-foreground">{items.length}</span>}
        </>}
        <span className="ml-auto text-muted-foreground">{sourceLabel}</span>
      </div>
      {resultFilter && <p className="shrink-0 border-b border-outline-variant px-4 py-2 text-[11px] text-muted-foreground">
        Keyword matches all search words. Semantic matches related meaning. A passage can match both.
      </p>}
      <div className="flex min-h-0 flex-1">
        <div ref={listRef} aria-busy={loading} className="min-w-0 flex-1 overflow-y-auto overscroll-contain @[600px]:max-w-[55%]">
          <div id="local-search-results" role="listbox" aria-label="Search results" className="p-1.5">
            {items.map((item, index) => <button key={item.id} id={`local-search-result-${item.id}`} role="option" aria-selected={selectedIndex === index}
              aria-disabled={item.disabledReason ? true : undefined} tabIndex={-1} type="button"
              onMouseMove={() => onHoverIndex(index)} onClick={() => { if (!item.disabledReason) onSelect(item); }}
              className="flex min-h-14 w-full cursor-pointer items-start gap-2.5 rounded-md px-2.5 py-2 text-left text-[13px] text-foreground outline-none aria-selected:bg-foreground/7 [@media(hover:hover)]:hover:bg-foreground/5 active:bg-foreground/10 focus-visible:ring-2 focus-visible:ring-ring">
              <FileText aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-icon-knowledge" />
              <span className="min-w-0 flex-1">
                <HighlightMatch className="block truncate" text={item.primaryText} query={query} ranges={item.primaryMatchRanges} />
                {item.secondaryText && <HighlightMatch className="mt-0.5 line-clamp-2 text-xs leading-relaxed text-muted-foreground" text={item.secondaryText} query={query} ranges={item.secondaryMatchRanges} excerpt={140} />}
                <span className="mt-1 block text-[11px] text-muted-foreground">{item.tertiaryText}</span>
                {item.matchSources && <span className="mt-1 flex flex-wrap gap-1">{item.matchSources.map((source) => <span key={source} className="rounded bg-foreground/7 px-1.5 py-0.5 text-[10px] text-muted-foreground">{source === "keyword" ? "Keyword" : "Semantic"}</span>)}</span>}
                {item.disabledReason && <span className="mt-1 block text-[11px] text-muted-foreground">{item.disabledReason}</span>}
              </span>
            </button>)}
          </div>
          {loading && <p className="px-6 py-12 text-center text-[13px] text-muted-foreground">Searching documents…</p>}
          {error && <div role="alert" className="px-6 py-12 text-center text-[13px] text-muted-foreground"><p>{error}</p><Button className="mt-3" size="sm" variant="outline" onClick={onRetry}>Retry search</Button>{onKeywordFallback && <Button className="mt-3 ml-2" size="sm" variant="outline" onClick={onKeywordFallback}>Use keyword search</Button>}</div>}
          {!loading && !error && !items.length && <div className="px-6 py-6 text-center text-[13px] text-muted-foreground @[600px]:py-12">
            {awaitingSubmit ? <p>Press Enter or Search to search uploaded passages.</p>
              : hasQuery && emptySource ? <>
                <p className="break-words">No {emptySource} matches for “{query}”</p>
                <p className="mt-1 text-xs">Other match types are available.</p>
                <Button className="mt-3" size="sm" variant="outline" onClick={() => { resultFilter?.onChange("all"); inputRef.current?.focus(); }}>Show all results</Button>
              </> : hasQuery ? <>
                <p className="break-words">No results for “{query}”</p>
                <p className="mt-1 text-xs">Try a different query.</p>
                <Button className="mt-3" size="sm" variant="outline" onClick={clear}>Clear query</Button>
              </> : <p>Search files and excerpts to find a passage.</p>}
          </div>}
        </div>
        <aside aria-label="Preview" className="hidden min-w-0 flex-1 flex-col border-l border-outline-variant bg-foreground/2 @[600px]:flex">
          {selected ? <>
            <div className="min-h-0 flex-1 space-y-3 overflow-y-auto overscroll-contain p-5">
              <p className="text-xs text-muted-foreground">{selected.tertiaryText}</p>
              <h3 className="break-words text-[15px] font-medium leading-snug tracking-tight"><HighlightMatch text={selected.primaryText} query={query} ranges={selected.primaryMatchRanges} /></h3>
              {selected.matchSources && selected.matchSources.length > 0 && <p className="text-xs font-medium text-foreground">
                {selected.matchSources.includes("keyword") && selected.matchSources.includes("semantic")
                  ? "Keyword + Semantic match" : selected.matchSources.includes("keyword") ? "Keyword match" : "Semantic match"}
              </p>}
              {selected.secondaryText && <p className="text-[13px] leading-relaxed text-muted-foreground"><HighlightMatch text={selected.secondaryText} query={query} ranges={selected.secondaryMatchRanges} /></p>}
              {selected.disabledReason && <p className="text-xs text-muted-foreground">{selected.disabledReason}</p>}
            </div>
            <div className="shrink-0 border-t border-outline-variant px-5 py-3"><Button size="sm" disabled={Boolean(selected.disabledReason)} onClick={() => onSelect(selected)}>Open <CornerDownLeft aria-hidden="true" className="size-3.5" /></Button></div>
          </> : <p className="m-auto px-6 text-center text-xs text-muted-foreground">{loading ? "Searching…" : error ? "Search unavailable" : hasQuery ? "Nothing to preview" : "Results show a preview here"}</p>}
        </aside>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-x-4 gap-y-2 border-t border-outline-variant px-4 py-2 text-[11px] text-muted-foreground">
        <span className="flex items-center gap-1"><Kbd className="border border-outline-variant bg-surface-container-low text-on-surface-variant"><ArrowUp aria-hidden="true" /></Kbd><Kbd className="border border-outline-variant bg-surface-container-low text-on-surface-variant"><ArrowDown aria-hidden="true" /></Kbd>Navigate</span>
        <span className="flex items-center gap-1"><Kbd className="border border-outline-variant bg-surface-container-low text-on-surface-variant"><CornerDownLeft aria-hidden="true" /></Kbd>Open</span>
        <span role="status" className="ml-auto">{loading ? "Searching…" : error ? "Search unavailable" : awaitingSubmit ? "Press Enter to search" : hasQuery ? `${items.length} ${resultType}${items.length === 1 ? "result" : "results"}` : "Type to search"}</span>
      </div>
    </div>
  );
}
