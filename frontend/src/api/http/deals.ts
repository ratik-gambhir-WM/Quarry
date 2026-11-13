import { acceptDealDeletion } from "../../data/dealDeletion/lifecycle";
import type { UpdateDealInput } from "../../data/dealModel";
import type { DealDeletion, PersistedDeal, QuarryApi } from "../../contracts/quarryApi";
import type {
  SaveDealInput,
  SaveDealMetadataInput,
  SaveDealMetadataResponse,
  SaveDealResponse,
  SavedDeal,
} from "../../data/dealExtraction";

import { get, post, postForm, appendFiles, requestJson } from "./transport";

function createDeal(input: SaveDealInput) {
  return post<SaveDealResponse, SaveDealInput>("/api/v1/deals", input);
}

function updateDeal(id: string, input: UpdateDealInput) {
  return post<SaveDealResponse, UpdateDealInput>(
    `/api/v1/deals/${encodeURIComponent(id)}/update`,
    input,
  );
}

function saveDealMetadata(dealId: string, input: SaveDealMetadataInput) {
  const form = new FormData();
  form.append("expectedRevision", String(input.expectedRevision));
  appendFiles(form, input.files);
  appendDealMetadataLinks(form, input);
  return postForm<SaveDealMetadataResponse>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/metadata`,
    form,
  );
}

function appendDealMetadataLinks(form: FormData, input: SaveDealMetadataInput) {
  form.append("sharepointLink", input.sharepointLink ?? "");
  form.append("sowLink", input.sowLink ?? "");
  form.append("factSheetLink", input.factSheetLink ?? "");
  form.append("rlLink", input.rlLink ?? "");
}

function listDeals() {
  return get<PersistedDeal[]>("/api/v1/deals");
}

function getDeal(dealId: string) {
  return get<PersistedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}`);
}

function archiveDeal(dealId: string) {
  return post<SavedDeal, Record<string, never>>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/archive`,
    {},
  );
}

export const httpDealsApi: Pick<
  QuarryApi,
  | "createDeal"
  | "updateDeal"
  | "saveDealMetadata"
  | "listDeals"
  | "getDeal"
  | "archiveDeal"
  | "deleteDeal"
  | "getDealDeletion"
> = {
  createDeal,
  updateDeal,
  saveDealMetadata,
  listDeals,
  getDeal,
  archiveDeal,
  deleteDeal: (id, expectedRevision) => acceptDealDeletion(id, async () => (await requestJson<DealDeletion | undefined>(`/api/v1/deals/${encodeURIComponent(id)}?expectedRevision=${expectedRevision}`, { method: "DELETE" }, undefined, false, [202, 204])) ?? null),
  getDealDeletion: (id) => requestJson<DealDeletion>(`/api/v1/deal-deletions/${encodeURIComponent(id)}`, { cache: "no-store" }, undefined, false, [200]),
};
