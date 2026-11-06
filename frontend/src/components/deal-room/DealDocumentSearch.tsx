import {
  useCallback,
  useId,
  useReducer,
  useRef,
  useSyncExternalStore,
} from "react";
import { browserFocusStore } from "../../data/requests/browserFocusStore";
import { Link } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import type { KeywordFileChunkHit } from "../../contracts/quarryApi";
import { getDataRoomPath } from "../../data/workspace";
import { keywordSearchExcerpt } from "../../data/documents/keywordSearch";
import { useWorkspace } from "../../app/WorkspaceProvider";
import { Button } from "../ui/stealth/button";
import { SearchField } from "../ui/search-field";
import { Field, FieldLabel } from "../ui/stealth/field";
import { SelectField } from "../ui/stealth/select-field";
import { EmptyState } from "../ui/stealth/empty-state";
type Result =
  | { status: "idle" | "loading" }
  | { status: "ready"; hits: KeywordFileChunkHit[] }
  | { status: "error"; message: string };
/** Query scope is explicit; each server route derives the authenticated user's permissions. */
export function DealDocumentSearch({ dealId }: { dealId: string }) {
  const searchId = useId();
  const { deals } = useWorkspace();
  const focusRevision = useSyncExternalStore(
    browserFocusStore.subscribe,
    browserFocusStore.getSnapshot,
    browserFocusStore.getSnapshot,
  );
  const [state, dispatch] = useReducer(searchReducer, {
    scope: "deal",
    query: "",
    result: { status: "idle" },
    focusRevision,
  });
  const generation = useRef(0);
  const pending = useRef<number | null>(null);
  const { scope, query } = state;
  const result =
    state.focusRevision === focusRevision
      ? state.result
      : { status: "idle" as const };
  const search = useCallback(async () => {
    if (
      !query.trim() ||
      (pending.current !== null &&
        state.focusRevision === browserFocusStore.getSnapshot())
    )
      return;
    const request = ++generation.current;
    pending.current = request;
    const focus = browserFocusStore.getSnapshot();
    dispatch({ type: "result", result: { status: "loading" }, focus });
    try {
      const body = { queryText: query.trim(), limit: 30 };
      const hits =
        scope === "deal"
          ? await runtime.api.searchDealDocumentChunksByKeyword(dealId, body)
          : await runtime.api.searchDocumentChunksByKeyword(body);
      if (
        request === generation.current &&
        focus === browserFocusStore.getSnapshot()
      )
        dispatch({ type: "result", result: { status: "ready", hits }, focus });
    } catch (error) {
      if (
        request === generation.current &&
        focus === browserFocusStore.getSnapshot()
      )
        dispatch({
          type: "result",
          result: {
            status: "error",
            message:
              error instanceof Error
                ? error.message
                : "Search failed. Try again.",
          },
          focus,
        });
    } finally {
      if (pending.current === request) pending.current = null;
    }
  }, [dealId, query, scope, state.focusRevision]);
  return (
    <section aria-labelledby="document-search-heading" className="space-y-5">
      <h1 className="type-h3" id="document-search-heading">
        Search documents
      </h1>
      <form
        className="flex flex-col gap-3 sm:flex-row sm:items-end"
        onSubmit={(e) => {
          e.preventDefault();
          void search();
        }}
      >
        <Field className="min-w-48 flex-1">
          <FieldLabel htmlFor={searchId}>Search terms</FieldLabel>
          <SearchField
            id={searchId}
            maxLength={2000}
            required
            value={query}
            clearLabel="Clear document search"
            onValueChange={(value) => {
              generation.current++;
              pending.current = null;
              dispatch({ type: "query", value, focus: focusRevision });
            }}
          />
        </Field>
        <SelectField<"deal" | "all">
          label="Search scope"
          className="w-full sm:w-48"
          value={scope}
          onValueChange={(value) => {
            generation.current++;
            pending.current = null;
            dispatch({ type: "scope", value, focus: focusRevision });
          }}
          options={[
            { value: "deal", label: "This deal" },
            { value: "all", label: "All accessible deals" },
          ]}
        />
        <Button
          type="submit"
          variant="primary"
          loading={result.status === "loading"}
          loadingText="Searching…"
          disabled={!query.trim()}
        >
          Search documents
        </Button>
      </form>
      <div aria-live="polite" role="status" className="text-sm text-muted-foreground">
        {result.status === "loading"
          ? "Searching documents…"
          : result.status === "ready"
            ? `${result.hits.length} matching ${result.hits.length === 1 ? "passage" : "passages"}${scope === "deal" ? " in this deal" : " across accessible deals"}.`
            : "Search current uploaded documents."}
      </div>
      {result.status === "error" ? (
        <div role="alert">
          <p>{result.message}</p>
          <Button onClick={() => void search()}>Retry search</Button>
        </div>
      ) : null}
      {result.status === "ready" ? (
        result.hits.length === 0 ? (
          <EmptyState
            variant="no-results"
            size="sm"
            heading="No matching documents."
            description="Try different search terms."
          />
        ) : (
          <ul aria-label="Search results" className="divide-y divide-border">
            {result.hits.map((hit) => {
              const dealName = deals.find((d) => d.room.id === hit.deal_id)?.room.name ?? "Deal";
              const page = hit.page_start == null
                ? `Passage ${hit.chunk_index}`
                : hit.page_end != null && hit.page_end !== hit.page_start
                  ? `Pages ${hit.page_start}–${hit.page_end}`
                  : `Page ${hit.page_start}`;
              return (
                <li key={hit.chunk_id} className="py-5">
                  <div className="max-w-4xl space-y-3">
                    <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-2">
                      <div className="min-w-0 flex-1 basis-56 space-y-1">
                        <p className="text-xs text-muted-foreground">{dealName} · {page}</p>
                        <h2 className="line-clamp-2 text-base font-medium text-foreground">
                          {hit.section_path.trim() || `Passage ${hit.chunk_index}`}
                        </h2>
                      </div>
                      <Link
                        aria-label={`Open data room for ${dealName}`}
                        className="shrink-0 rounded-sm text-sm text-primary underline underline-offset-4 focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-primary"
                        to={getDataRoomPath(hit.deal_id)}
                      >
                        Open data room
                      </Link>
                    </div>
                    <p className="wrap-anywhere text-sm leading-relaxed text-foreground [&_mark]:rounded-xs [&_mark]:bg-primary/10 [&_mark]:text-foreground [&_mark]:font-medium">
                      {keywordSearchExcerpt(hit.text, query).map((part, index) =>
                        part.matched ? <mark key={index}>{part.text}</mark> : part.text,
                      )}
                    </p>
                  </div>
                </li>
              );
            })}
          </ul>
        )
      ) : null}
    </section>
  );
}

type SearchState = {
  scope: "deal" | "all";
  query: string;
  result: Result;
  focusRevision: number;
};
type SearchAction =
  | { type: "query"; value: string; focus: number }
  | { type: "scope"; value: "deal" | "all"; focus: number }
  | { type: "result"; result: Result; focus: number };
function searchReducer(state: SearchState, action: SearchAction): SearchState {
  if (action.type === "result")
    return { ...state, result: action.result, focusRevision: action.focus };
  if (action.type === "query")
    return {
      ...state,
      query: action.value,
      result: { status: "idle" },
      focusRevision: action.focus,
    };
  return {
    ...state,
    scope: action.value,
    result: { status: "idle" },
    focusRevision: action.focus,
  };
}
