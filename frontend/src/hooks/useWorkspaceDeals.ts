import { useEffect, useState } from "react";
import { runtime } from "@quarry/runtime";
import { buildWorkspaceDealFromPersisted } from "../data/dealExtraction";
import type { WorkspaceDeal } from "../data/workspace";
import { workspaceDeals } from "../fixtures/workspace/portfolio";

export function useWorkspaceDeals() {
  const [deals, setDeals] = useState<WorkspaceDeal[]>(workspaceDeals);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let active = true;
    void runtime.api.listDeals()
      .then((persisted) => {
        if (!active) return;
        const serverDeals = persisted.map((deal) => buildWorkspaceDealFromPersisted(deal, deal.metadata));
        const ids = new Set(serverDeals.map((deal) => deal.room.id));
        setDeals([...serverDeals, ...workspaceDeals.filter((deal) => !ids.has(deal.room.id))]);
      })
      .catch(() => {
        if (active) setDeals(workspaceDeals);
      })
      .finally(() => {
        if (active) setLoaded(true);
      });
    return () => {
      active = false;
    };
  }, []);

  return { deals, loaded };
}
