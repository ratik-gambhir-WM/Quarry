import { useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import { createRequestResource } from "../../data/requests/requestResource";
import { Button } from "../ui/button";

export function DealActivity({ dealId }: { dealId: string }) {
  const resource = useMemo(
    () => createRequestResource(() => runtime.api.listDealActivity(dealId)),
    [dealId],
  );
  const state = useSyncExternalStore(
    resource.subscribe,
    resource.getSnapshot,
    resource.getSnapshot,
  );
  const page = state.status === "ready" ? state.value : null;
  const error =
    state.status === "error"
      ? state.message
      : state.status === "ready"
        ? state.message
        : undefined;
  const loadMore = () =>
    resource.update(async (current) => {
      if (!current.nextCursor) return current;
      const next = await runtime.api.listDealActivity(dealId, {
        cursor: current.nextCursor,
      });
      const seen = new Set(current.items.map((item) => item.eventId));
      return {
        items: [
          ...current.items,
          ...next.items.filter((item) => !seen.has(item.eventId)),
        ],
        nextCursor: next.nextCursor,
      };
    });
  return (
    <section aria-labelledby="deal-activity-heading" className="space-y-4">
      <h1 className="type-h3" id="deal-activity-heading">
        Deal activity
      </h1>
      <p className="text-sm text-muted">
        Activity records accepted changes. Legacy uploads have no recorded
        uploader.
      </p>
      {error ? (
        <div role="alert">
          <p>{error}</p>
          <Button onClick={resource.refresh}>Retry activity</Button>
        </div>
      ) : null}
      {state.status === "loading" ? (
        <p role="status">Loading activity…</p>
      ) : null}
      {page?.items.length === 0 ? (
        <p>No activity yet.</p>
      ) : (
        <ol className="divide-y divide-border">
          {page?.items.map((event) => (
            <li className="py-3" key={event.eventId}>
              <p>
                {event.actorName ?? "System"} ·{" "}
                {event.kind.split("_").join(" ")}
              </p>
              <time className="text-sm text-muted" dateTime={event.createdAt}>
                {new Date(event.createdAt).toLocaleString()}
              </time>
            </li>
          ))}
        </ol>
      )}
      {page?.nextCursor ? (
        <Button
          disabled={state.status === "ready" && state.pending}
          onClick={loadMore}
        >
          {state.status === "ready" && state.pending
            ? "Loading…"
            : "Load more activity"}
        </Button>
      ) : null}
    </section>
  );
}
