import { isDealDeleted, subscribeDealDeletion } from "../data/dealDeletion/lifecycle";
import type { Company } from "../data/dealModel";
import { useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import {
  buildWorkspaceDealFromExtractionResult,
  type SaveDealResponse,
  type SaveDealMetadataResponse,
  buildWorkspaceDealFromPersisted,
} from "../data/dealExtraction";
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
  throw new Error(
    `Invalid VITE_WORKSPACE_DATA_SOURCE value: ${value}. Expected "api" or "demo".`,
  );
}

// Each provider owns its resource store. Subscription callbacks publish async results;
// scope/retry changes never set React state from an effect body.
function createDealsStore(dataSourceValue: string | undefined) {
  let resource: WorkspaceDealsResource = { status: "loading" };
  let generation = 0;
  const revoked = new Map<string, number>();
  const forgotten = new Set<string>();
  let revocationRevision = 0;
  const writesDuringLoad = new Map<string, WorkspaceDeal>();
  const companiesDuringLoad = new Map<string, Company>();
  let pendingLoad = false;
  const listeners = new Set<() => void>();
  const publish = (next: WorkspaceDealsResource) => {
    resource = next;
    for (const listener of listeners) listener();
  };
  const cache = (deal: WorkspaceDeal) => {
    if (forgotten.has(deal.room.id) || isDealDeleted(deal.room.id)) return;
    if (pendingLoad) writesDuringLoad.set(deal.room.id, deal);
    const existing = resource.status === "success" ? resource.deals : [];
    const previous = existing.find((d) => d.room.id === deal.room.id);
    if (
      previous &&
      previous.persisted?.accessRevision === deal.persisted?.accessRevision &&
      previous.persistedMetadata?.sowLink === deal.persistedMetadata?.sowLink
    ) {
      deal.room.resources = deal.room.resources.map((item) => {
        const sourceName = previous.room.resources.find(
          (old) => old.id === item.id,
        )?.sourceName;
        return !item.sourceName && sourceName
          ? { ...item, sourceName, availability: "available" }
          : item;
      });
    }
    publish({
      status: "success",
      source: "server",
      deals: [deal, ...existing.filter((d) => d.room.id !== deal.room.id)],
    });
  };
  const load = () => {
    const request = ++generation;
    const revokedAtStart = new Map(revoked);
    pendingLoad = true;
    writesDuringLoad.clear();
    companiesDuringLoad.clear();
    publish({ status: "loading" });
    let source: WorkspaceDataSource;
    try {
      source = getWorkspaceDataSource(dataSourceValue);
    } catch (error) {
      publish({
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      });
      return;
    }
    const pending =
      source === "demo"
        ? import("../fixtures/workspace/portfolio").then(
            ({ workspaceDeals }) => ({
              deals: workspaceDeals,
              source: "demo" as const,
              status: "success" as const,
            }),
          )
        : runtime.api.listDeals().then((deals) => ({
            deals: deals.map((deal) =>
              buildWorkspaceDealFromPersisted(deal, deal.metadata),
            ),
            source: "server" as const,
            status: "success" as const,
          }));
    void pending
      .then((next) => {
        if (request !== generation) return;
        pendingLoad = false;
        const authorized = next.deals.filter((deal) => {
          if (next.source === "server" && isDealDeleted(deal.room.id)) return false;
          const revision = revoked.get(deal.room.id);
          if (
            revision !== undefined &&
            revokedAtStart.get(deal.room.id) !== revision
          )
            return false;
          forgotten.delete(deal.room.id);
          return true;
        });
        const merged = [
          ...writesDuringLoad.values(),
          ...authorized.filter((deal) => !writesDuringLoad.has(deal.room.id)),
        ];
        const deals = merged.map((deal) => {
          for (const company of companiesDuringLoad.values())
            deal = withCompany(deal, company);
          return deal;
        });
        publish({ ...next, deals });
        writesDuringLoad.clear();
        companiesDuringLoad.clear();
      })
      .catch((error: unknown) => {
        if (request === generation)
          publish({
            status: "error",
            message: error instanceof Error ? error.message : String(error),
          });
      });
  };
  let stopDeletion: (() => void) | undefined;
  const forget = (id: string) => {
      revoked.set(id, ++revocationRevision);
      forgotten.add(id);
      writesDuringLoad.delete(id);
      if (resource.status === "success")
        publish({
          ...resource,
          deals: resource.deals.filter((d) => d.room.id !== id),
        });
  };
  return {
    forget,
    load,
    getSnapshot: () => resource,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (listeners.size === 1) { stopDeletion = subscribeDealDeletion(forget); load(); }
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) { generation++; stopDeletion?.(); }
      };
    },
    saveCompany: (company: Company) => {
      if (pendingLoad) companiesDuringLoad.set(company.companyId, company);
      if (resource.status !== "success") return;
      const updated = resource.deals.map((deal) => withCompany(deal, company));
      publish({ ...resource, deals: updated });
    },
    loadDeal: async (id: string) => {
      if (isDealDeleted(id)) throw new Error("This deal is unavailable while permanent deletion is processed.");
      const revision = revoked.get(id);
      const aggregate = await runtime.api.getDeal(id);
      if (revoked.get(id) !== revision || isDealDeleted(id)) {
        throw new Error("Deal access changed. Try again.");
      }
      // Keep revocation revisions even after access is restored so older requests
      // cannot repopulate the cache after this fresh authorized response.
      forgotten.delete(id);
      cache(buildWorkspaceDealFromPersisted(aggregate, aggregate.metadata));
      return aggregate;
    },
    save: (
      result: SaveDealResponse | SaveDealMetadataResponse,
      sowSourceName?: string,
    ) => {
      const deal =
        "extraction" in result
          ? buildWorkspaceDealFromExtractionResult(result, sowSourceName)
          : buildWorkspaceDealFromPersisted(result.deal, result.metadata);
      // Core deal edits preserve metadata, including source details held only in this session.
      const previous =
        resource.status === "success"
          ? resource.deals.find((d) => d.room.id === deal.room.id)
          : undefined;
      if (!("extraction" in result) && previous)
        deal.room.resources = previous.room.resources;
      if ("extraction" in result) {
        if (result.files.length === 0) {
          deal.room.keyQuestions = buildWorkspaceDealFromPersisted(
            result.deal,
            result.metadata,
          ).room.keyQuestions;
        }
        const previousSow = previous?.room.resources.find(
          (resource) => resource.id === "sow",
        );
        if (!sowSourceName && previousSow?.sourceName) {
          deal.room.resources = deal.room.resources.map((resource) =>
            resource.id === "sow"
              ? {
                  ...resource,
                  availability: "available",
                  sourceName: previousSow.sourceName,
                }
              : resource,
          );
        }
      }
      cache(deal);
    },
  };
}

function withCompany(deal: WorkspaceDeal, company: Company): WorkspaceDeal {
  if (
    !deal.persisted ||
    !deal.persisted.parties.some((p) => p.companyId === company.companyId)
  )
    return deal;
  const persisted = {
    ...deal.persisted,
    parties: deal.persisted.parties.map((p) =>
      p.companyId === company.companyId ? { ...p, company } : p,
    ),
  };
  const rebuilt = buildWorkspaceDealFromPersisted(
    persisted,
    deal.persistedMetadata ?? null,
  );
  return {
    ...rebuilt,
    room: {
      ...rebuilt.room,
      keyQuestions: deal.room.keyQuestions,
      resources: deal.room.resources,
    },
  };
}
export function useWorkspaceDeals(
  dataSourceValue: string | undefined = import.meta.env
    .VITE_WORKSPACE_DATA_SOURCE,
) {
  const store = useMemo(
    () => createDealsStore(dataSourceValue),
    [dataSourceValue],
  );
  const resource = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
  return {
    resource,
    retry: store.load,
    forgetDeal: store.forget,
    loadDeal: store.loadDeal,
    saveDealResult: store.save,
    saveCompanyResult: store.saveCompany,
  };
}
