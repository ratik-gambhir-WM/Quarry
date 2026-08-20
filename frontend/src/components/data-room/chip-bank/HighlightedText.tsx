import { getSearchQueryTerms } from "./repositorySearch";

type HighlightedTextProps = {
  highlightClassName?: string;
  query: string;
  text: string;
};

export function HighlightedText({
  highlightClassName = "bg-[#fff1a8] px-0.5 text-[#1c2433]",
  query,
  text,
}: HighlightedTextProps) {
  const terms = getSearchQueryTerms(query);

  if (terms.length === 0) {
    return text;
  }

  const expression = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "gi");
  const termSet = new Set(terms);

  return text.split(expression).map((part, index) =>
    termSet.has(part.toLowerCase()) ? (
      <mark className={highlightClassName} key={`${part}-${index}`}>
        {part}
      </mark>
    ) : (
      part
    ),
  );
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
