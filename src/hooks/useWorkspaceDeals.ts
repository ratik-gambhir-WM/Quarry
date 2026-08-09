import { useEffect, useState } from "react";
import { buildWorkspaceDealFromPersisted } from "../data/dealExtraction";
import { workspaceDeals, type WorkspaceDeal } from "../data/workspace";
import { productApi } from "../lib/product";

export function useWorkspaceDeals() {
  const [deals, setDeals] = useState<WorkspaceDeal[]>(workspaceDeals);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let active = true;

    void productApi
      .listDeals()
      .then((persisted) => {
        if (!active) {
          return;
        }
        const nativeDeals = persisted.map((deal) =>
          buildWorkspaceDealFromPersisted(deal, deal.metadata),
        );
        const persistedIds = new Set(nativeDeals.map((deal) => deal.room.id));
        setDeals([
          ...nativeDeals,
          ...workspaceDeals.filter((deal) => !persistedIds.has(deal.room.id)),
        ]);
      })
      .catch(() => {
        if (active) {
          setDeals(workspaceDeals);
        }
      })
      .finally(() => {
        if (active) {
          setLoaded(true);
        }
      });

    return () => {
      active = false;
    };
  }, []);

  return { deals, loaded };
}
