export type DocumentSearchTarget = {
  kind: "pdf-page";
  page: number;
};

export type DocumentSearchItem = {
  matchSources?: ("keyword" | "semantic")[];
  fileName?: string;
  target?: DocumentSearchTarget;
  disabledReason?: string;
  id: string;
  imageAlt?: string;
  imageUrl?: string;
  primaryText: string;
  primaryMatchRanges?: readonly (readonly [number, number])[];
  searchText?: string;
  secondaryText?: string;
  secondaryMatchRanges?: readonly (readonly [number, number])[];
  tertiaryText?: string;
};

export type DocumentSearchResult = {
  excerpt: string;
  fileName: string;
  id: string;
  location: string;
  target?: DocumentSearchTarget;
};

export function filterDocumentSearchItems(
  items: DocumentSearchItem[],
  query: string,
): DocumentSearchItem[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/).filter(Boolean);
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
