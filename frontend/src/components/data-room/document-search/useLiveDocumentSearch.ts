import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { browserFocusStore } from "@/data/requests/browserFocusStore";
import type { DocumentSearchItem } from "./documentSearchModel";

export type DocumentSearchRequest = (query: string) => Promise<DocumentSearchItem[]>;
type SearchResult =
  | { status: "ready"; items: DocumentSearchItem[] }
  | { status: "error"; message: string };

/** Debounces requests; late responses never replace a newer query or a closed panel. */
export function useLiveDocumentSearch(
  search: DocumentSearchRequest | undefined,
  query: string,
  open: boolean,
) {
  const focusRevision = useSyncExternalStore(
    browserFocusStore.subscribe, browserFocusStore.getSnapshot, browserFocusStore.getSnapshot,
  );
  const [attempt, setAttempt] = useState(0);
  const q = query.trim();
  const key = useMemo(() => ({}), [search, q, open, attempt, focusRevision]);
  const [completed, setCompleted] = useState<{ key: object; result: SearchResult } | null>(null);
  const enabled = Boolean(search && q && open);

  useEffect(() => {
    if (!search || !enabled) return;
    let active = true;
    const timer = window.setTimeout(() => {
      void search(q).then(
        (items) => { if (active) setCompleted({ key, result: { status: "ready", items } }); },
        (error: unknown) => {
          if (active) setCompleted({ key, result: {
            status: "error",
            message: error instanceof Error ? error.message : "Couldn’t search documents. Try again.",
          } });
        },
      );
    }, 250);
    return () => { active = false; window.clearTimeout(timer); };
  }, [enabled, key, q, search]);

  const result = enabled && completed?.key === key ? completed.result : null;
  return {
    items: result?.status === "ready" ? result.items : [],
    loading: enabled && result === null,
    error: result?.status === "error" ? result.message : undefined,
    retry: () => setAttempt((value) => value + 1),
  };
}
