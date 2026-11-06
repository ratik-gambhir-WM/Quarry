export type KeywordExcerptPart = { text: string; matched: boolean };

// Keep word boundaries aligned with the backend's keyword_words, including C++/C#.
function keywordWords(text: string) {
  return [...text.matchAll(/[\p{L}\p{N}+#]+/gu)].filter(([word]) =>
    /[\p{L}\p{N}]/u.test(word),
  );
}

/** A compact passage around a distinctive match, with safe text-only highlights. */
export function keywordSearchExcerpt(
  text: string,
  query: string,
): KeywordExcerptPart[] {
  const compact = text.replace(/\s+/gu, " ").trim();
  const terms = new Set(
    keywordWords(query).map(([word]) => word.toLowerCase()),
  );
  const matches = keywordWords(compact).filter(([word]) =>
    terms.has(word.toLowerCase()),
  );
  const frequencies = new Map<string, number>();
  for (const [word] of matches) {
    const key = word.toLowerCase();
    frequencies.set(key, (frequencies.get(key) ?? 0) + 1);
  }
  const lowestFrequency = Math.min(...frequencies.values());
  const match = matches.find(
    ([word]) => frequencies.get(word.toLowerCase()) === lowestFrequency,
  );
  const length = 320;
  let start = Math.max(
    0,
    Math.min((match?.index ?? 0) - 80, compact.length - length),
  );
  if (start > 0) {
    const boundary = compact.indexOf(" ", start);
    if (boundary >= 0 && boundary < (match?.index ?? 0)) start = boundary + 1;
  }
  let end = Math.min(compact.length, start + length);
  if (end < compact.length) {
    const boundary = compact.lastIndexOf(" ", end);
    if (boundary > (match?.index ?? start)) end = boundary;
  }
  const excerpt = compact.slice(start, end);
  const parts: KeywordExcerptPart[] = [];
  let position = 0;
  if (start > 0) parts.push({ text: "…", matched: false });
  for (const word of keywordWords(excerpt)) {
    if (!terms.has(word[0].toLowerCase())) continue;
    if (word.index > position)
      parts.push({ text: excerpt.slice(position, word.index), matched: false });
    parts.push({ text: word[0], matched: true });
    position = word.index + word[0].length;
  }
  if (position < excerpt.length)
    parts.push({ text: excerpt.slice(position), matched: false });
  if (end < compact.length) parts.push({ text: "…", matched: false });
  return parts;
}
