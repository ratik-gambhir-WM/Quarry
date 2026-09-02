export function DocumentSearchHighlightedText({
  query,
  text,
}: {
  query: string;
  text: string;
}) {
  const terms = getDocumentSearchQueryTerms(query);
  if (terms.length === 0) {
    return text;
  }
  const expression = new RegExp(`(${terms.map(escapeRegExp).join("|")})`, "gi");
  const termSet = new Set(terms);
  return text.split(expression).map((part, index) =>
    termSet.has(part.toLocaleLowerCase()) ? <mark key={`${part}-${index}`}>{part}</mark> : part,
  );
}

export function getDocumentSearchQueryTerms(query: string): string[] {
  return query
    .trim()
    .toLocaleLowerCase()
    .split(/\s+/)
    .filter(Boolean);
}

function escapeRegExp(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
