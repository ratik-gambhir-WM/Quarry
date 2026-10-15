import type { Company } from "../data/dealModel";
import { useEffect, useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import { buildWorkspaceDealFromExtractionResult, type SaveDealResponse, type SaveDealMetadataResponse, buildWorkspaceDealFromPersisted } from "../data/dealExtraction";
import type { WorkspaceDeal } from "../data/workspace";

export type WorkspaceDataSource = "api" | "demo";

export type WorkspaceDealsResource =
  | { status: "loading" }
  | { deals: WorkspaceDeal[]; source: "server"; status: "success" }
  | { deals: WorkspaceDeal[]; source: "demo"; status: "success" }
  | { message: string; status: "error" };

export function getWorkspaceDataSource(
  value: string | undefined = import.meta.env.VITE_WORKSPACE_DATA_SOURCE,
): WorkspaceDataSource {
  if (value === undefined || value === "" || value === "api") {
    return "api";
  }
  if (value === "demo") {
    return "demo";
  }
  throw new Error(`Invalid VITE_WORKSPACE_DATA_SOURCE value: ${value}. Expected "api" or "demo".`);
}

// Each provider owns its resource store. Subscription callbacks publish async results;
// scope/retry changes never set React state from an effect body.
function createDealsStore(dataSourceValue: string | undefined) {
  let resource: WorkspaceDealsResource = { status: "loading" };
  let generation = 0;
  const writesDuringLoad = new Map<string, WorkspaceDeal>();
  let pendingLoad = false;
  const listeners = new Set<() => void>();
  const publish = (next: WorkspaceDealsResource) => { resource = next; for (const listener of listeners) listener(); };
  const load = () => {
    const request = ++generation;
    pendingLoad = true;
    writesDuringLoad.clear();
    publish({ status: "loading" });
    let source: WorkspaceDataSource;
    try { source = getWorkspaceDataSource(dataSourceValue); }
    catch (error) { publish({ status: "error", message: error instanceof Error ? error.message : String(error) }); return; }
    const pending = source === "demo"
      ? import("../fixtures/workspace/portfolio").then(({ workspaceDeals }) => ({ deals: workspaceDeals, source: "demo" as const, status: "success" as const }))
      : runtime.api.listDeals().then((deals) => ({ deals: deals.map((deal) => buildWorkspaceDealFromPersisted(deal, deal.metadata)), source: "server" as const, status: "success" as const }));
    void pending.then((next) => {
      if (request !== generation) return;
      pendingLoad = false;
      const merged = [...writesDuringLoad.values(), ...next.deals.filter((deal) => !writesDuringLoad.has(deal.room.id))];
      publish({ ...next, deals: merged });
      writesDuringLoad.clear();
    }).catch((error: unknown) => {
      if (request === generation) publish({ status: "error", message: error instanceof Error ? error.message : String(error) });
    });
  };
  return {
    load,
    cancel: () => { generation++; },
    getSnapshot: () => resource,
    subscribe: (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    saveCompany: (company: Company) => {
      if (resource.status !== "success") return;
      const updated = resource.deals.map((deal) => {
        if (!deal.persisted || !deal.persisted.parties.some((p) => p.companyId === company.companyId)) return deal;
        const persisted = { ...deal.persisted, parties: deal.persisted.parties.map((p) => p.companyId === company.companyId ? { ...p, company } : p) };
        const rebuilt = buildWorkspaceDealFromPersisted(persisted, deal.persistedMetadata ?? null);
        return { ...rebuilt, room: { ...rebuilt.room, keyQuestions: deal.room.keyQuestions, resources: deal.room.resources } };
      });
      publish({ ...resource, deals: updated });
    },
    save: (result: SaveDealResponse | SaveDealMetadataResponse) => {
      const deal = "extraction" in result ? buildWorkspaceDealFromExtractionResult(result) : buildWorkspaceDealFromPersisted(result.deal, result.metadata);
      if (pendingLoad) writesDuringLoad.set(deal.room.id, deal);
      const existing = resource.status === "success" ? resource.deals : [];
      publish({ status: "success", source: "server", deals: [deal, ...existing.filter((d) => d.room.id !== deal.room.id)] });
    },
  };
}
export function useWorkspaceDeals(dataSourceValue: string | undefined = import.meta.env.VITE_WORKSPACE_DATA_SOURCE) {
  const store = useMemo(() => createDealsStore(dataSourceValue), [dataSourceValue]);
  const resource = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
  useEffect(() => { store.load(); return store.cancel; }, [store]);
  return { resource, retry: store.load, saveDealResult: store.save, saveCompanyResult: store.saveCompany };
}
