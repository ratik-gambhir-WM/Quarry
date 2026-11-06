// Adapted from https://www.stealth.pm/r/highlight-match.json to Quarry theme tokens.
import { Fragment, useMemo } from "react";
import { cn } from "@/lib/utils";

import { rangesFor, snippet, type MatchMode, type MatchRange } from "./highlight-match-model";

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */

export type HighlightMatchProps = Omit<React.ComponentProps<"span">, "children"> & {
  text: string;
  /** What to mark. An array marks several queries at once, e.g. one per active search term. */
  query: string | string[];
  mode?: MatchMode;
  /** Precomputed ranges (from findMatches) instead of a query, so a list doesn't match twice. */
  ranges?: readonly MatchRange[];
  /** "fill" washes the match like find-in-page; "text" lifts it to full contrast and medium weight. */
  variant?: "fill" | "text";
  /** Show a snippet of about this many characters around the first match, with ellipses. */
  excerpt?: number;
  markClassName?: string;
};

export function HighlightMatch({ text, query, mode = "words", ranges: given, variant = "fill", excerpt, markClassName, className, ...rest }: HighlightMatchProps) {
  const ranges = useMemo(() => (given ? [...given] : rangesFor(text, query, mode)), [given, text, query, mode]);
  const { from, to } = excerpt ? snippet(text, ranges, excerpt) : { from: 0, to: text.length };

  const parts: React.ReactNode[] = [];
  let at = from;
  for (const [a, b] of ranges) {
    const s = Math.max(a, from, at);
    const e = Math.min(b, to);
    if (e <= s) continue;
    if (s > at) parts.push(<Fragment key={`t${at}`}>{text.slice(at, s)}</Fragment>);
    parts.push(
      // Keyed by where the match starts: a match that grows as you type stays the same element
      // and simply widens, while a match that is new washes in from nothing.
      <mark
        key={`m${a}`}
        data-match=""
        data-variant={variant}
        className={cn(
                    variant === "fill"
            ? "-mx-px box-decoration-clone rounded-[3px] bg-search-match px-px text-on-search-match"
            : "bg-transparent font-medium text-foreground transition-[color] duration-150 ease-out starting:text-inherit",
          "motion-reduce:transition-none",
          markClassName,
        )}
      >
        {text.slice(s, e)}
      </mark>,
    );
    at = e;
  }
  if (at < to) parts.push(<Fragment key={`t${at}`}>{text.slice(at, to)}</Fragment>);

  return (
    <span className={className} {...rest}>
      {from > 0 && <span aria-hidden>…</span>}
      {parts}
      {to < text.length && <span aria-hidden>…</span>}
    </span>
  );
}
