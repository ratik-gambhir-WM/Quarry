import type { DocumentSearchItem } from "./documentSearchModel";
import { DocumentSearchHighlightedText } from "./DocumentSearchHighlightedText";

export function DocumentSearchResultCard({
  index,
  item,
  onHover,
  onSelect,
  query,
  selected,
}: {
  index: number;
  item: DocumentSearchItem;
  onHover: (index: number) => void;
  onSelect: (item: DocumentSearchItem) => void;
  query: string;
  selected: boolean;
}) {
  return (
    <button
      aria-disabled={item.disabledReason ? "true" : undefined}
      aria-selected={selected}
      className="flex cursor-pointer flex-row items-center gap-4 rounded-sm bg-background p-4 text-left text-foreground no-underline transition aria-selected:bg-surface-container dark:aria-selected:bg-slate-900 motion-reduce:transition-none"
      id={`local-search-result-${item.id}`}
      onClick={() => {
        if (!item.disabledReason) {
          onSelect(item);
        }
      }}
      onMouseMove={() => onHover(index)}
      role="option"
      tabIndex={-1}
      type="button"
    >
      {item.imageUrl ? (
        <span className="flex h-[100px] w-[100px] shrink-0 items-center justify-center self-start overflow-hidden rounded-sm bg-muted">
          <img
            alt={item.imageAlt ?? ""}
            className="h-full w-full rounded-sm object-contain"
            src={item.imageUrl}
          />
        </span>
      ) : null}
      <span className="min-w-0 flex-1">
        <span className="block font-medium [&_mark]:bg-transparent [&_mark]:text-secondary-foreground [&_mark]:underline [&_mark]:underline-offset-4">
          <DocumentSearchHighlightedText query={query} text={item.primaryText} />
        </span>
        {item.secondaryText ? (
          <span className="mt-2 block text-sm text-muted-foreground">
            <DocumentSearchHighlightedText query={query} text={item.secondaryText} />
          </span>
        ) : null}
        {item.tertiaryText ? (
          <span className="mt-2 block text-sm text-muted-foreground">
            {item.tertiaryText}
          </span>
        ) : null}
        {item.disabledReason ? (
          <span className="mt-2 block text-xs font-medium text-muted-foreground">
            {item.disabledReason}
          </span>
        ) : null}
      </span>
    </button>
  );
}
