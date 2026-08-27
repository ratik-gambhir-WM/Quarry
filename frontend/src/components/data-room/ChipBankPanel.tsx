import { useMemo, useState } from "react";
import type { DataRoomChip } from "../../data/dataRoom";
import { ChipBankHeader } from "./chip-bank/ChipBankHeader";
import { RepositorySearchControls } from "./chip-bank/RepositorySearchControls";
import { RepositorySearchEmptyState } from "./chip-bank/RepositorySearchEmptyState";
import { RepositorySearchResultCard } from "./chip-bank/RepositorySearchResultCard";
import {
  ALL_CATEGORIES,
  ANY_TIME,
  buildRepositoryResults,
  filterRepositoryResults,
  getRepositoryCategories,
  type SearchMode,
} from "./chip-bank/repositorySearch";

type ChipBankPanelProps = {
  chips: DataRoomChip[];
  onCollapse: () => void;
};

export function ChipBankPanel({ chips, onCollapse }: ChipBankPanelProps) {
  const [category, setCategory] = useState(ALL_CATEGORIES);
  const [date, setDate] = useState(ANY_TIME);
  const [mode, setMode] = useState<SearchMode>("semantic");
  const [query, setQuery] = useState("liability coverage environmental hazards");

  const repositoryResults = useMemo(() => buildRepositoryResults(chips), [chips]);
  const categories = useMemo(() => getRepositoryCategories(repositoryResults), [repositoryResults]);
  const filteredResults = useMemo(
    () => filterRepositoryResults({ category, date, mode, query, results: repositoryResults }),
    [category, date, mode, query, repositoryResults],
  );

  return (
    <section className="-ml-px flex w-[clamp(360px,30vw,440px)] min-w-[360px] max-w-[440px] flex-none flex-col border-l border-outline-variant/70 bg-background text-sidebar-text [font-family:var(--font-sidebar)]">
      <div className="flex h-16 shrink-0 items-center border-b border-outline-variant/70 px-4">
        <ChipBankHeader onCollapse={onCollapse} />
      </div>
      <div className="border-b border-outline-variant/70 px-4 py-4">
        <RepositorySearchControls
          categories={categories}
          category={category}
          date={date}
          mode={mode}
          onCategoryChange={setCategory}
          onDateChange={setDate}
          onModeChange={setMode}
          onQueryChange={setQuery}
          query={query}
        />
      </div>

      <div className="workspace-scrollbar-hidden min-h-0 flex-1 overflow-y-auto px-4 py-4">
        <div className="mb-3 flex items-center justify-between px-1">
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-sidebar-muted">
            {filteredResults.length} {filteredResults.length === 1 ? "match" : "matches"}
          </p>
          <p className="text-[12px] font-medium capitalize text-sidebar-muted">{mode} search</p>
        </div>

        {filteredResults.length > 0 ? (
          <div className="space-y-3">
            {filteredResults.map((result) => (
              <RepositorySearchResultCard key={result.id} mode={mode} query={query} result={result} />
            ))}
          </div>
        ) : (
          <RepositorySearchEmptyState />
        )}
      </div>
    </section>
  );
}
