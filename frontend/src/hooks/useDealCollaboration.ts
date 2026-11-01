import { useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import type { DealPage } from "../contracts/dealMemberships";
import { createRequestResource } from "../data/requests/requestResource";

export async function collectDealPages<T>(
  load: (cursor?: string) => Promise<DealPage<T>>,
  max: number,
): Promise<T[]> {
  const items: T[] = [];
  const seen = new Set<string>();
  let cursor: string | undefined;
  do {
    const page = await load(cursor);
    items.push(...page.items);
    if (items.length > max)
      throw new Error(
        "The roster exceeds its supported size. Refresh and try again.",
      );
    if (!page.nextCursor) return items;
    if (seen.has(page.nextCursor))
      throw new Error(
        "The roster changed while loading. Refresh and try again.",
      );
    seen.add(page.nextCursor);
    cursor = page.nextCursor;
  } while (true);
}

export function useDealCollaboration(dealId: string) {
  const resource = useMemo(
    () =>
      createRequestResource(async () => {
        const [members, teams] = await Promise.all([
          collectDealPages(
            (cursor) =>
              runtime.api.listDealMembers(dealId, { cursor, limit: 100 }),
            500,
          ),
          collectDealPages(
            (cursor) =>
              runtime.api.listDealTeams(dealId, { cursor, limit: 100 }),
            100,
          ),
        ]);
        return { members, teams };
      }),
    [dealId],
  );
  const state = useSyncExternalStore(
    resource.subscribe,
    resource.getSnapshot,
    resource.getSnapshot,
  );
  const roster =
    state.status === "ready"
      ? { status: "ready" as const, ...state.value }
      : state;
  return { roster, reload: resource.refresh };
}
