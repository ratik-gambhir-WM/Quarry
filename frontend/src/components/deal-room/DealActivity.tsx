import { useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import { createRequestResource } from "../../data/requests/requestResource";
import { FileUp, History, Pencil, Plus, UserMinus, Users } from "lucide-react";
import { ActivityFeed, type ActivityItem } from "../ui/stealth/activity-feed";
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
          <Button onClick={page ? loadMore : resource.refresh}>
            Retry activity
          </Button>
        </div>
      ) : null}
      <ActivityFeed
        items={
          page?.items.map((event): ActivityItem => ({
            id: event.eventId,
            actor: { name: event.actorName ?? "System" },
            action: event.kind.split("_").join(" "),
            at: event.createdAt,
            icon: activityIcon(event.kind),
          })) ?? []
        }
        loading={state.status === "loading"}
        loadingMore={state.status === "ready" && state.pending}
        hasMore={!!page?.nextCursor}
        onLoadMore={loadMore}
        hidden={state.status === "error"}
      />
    </section>
  );
}

function activityIcon(kind: string) {
  if (kind === "document_uploaded") return <FileUp />;
  if (kind === "member_removed" || kind === "team_deleted") return <UserMinus />;
  if (
    kind.startsWith("member_") ||
    kind.startsWith("team_") ||
    kind === "ownership_transferred"
  ) return <Users />;
  if (kind.endsWith("created")) return <Plus />;
  if (kind.endsWith("updated") || kind.endsWith("renamed")) return <Pencil />;
  return <History />;
}
