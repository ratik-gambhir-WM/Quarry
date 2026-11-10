import { afterEach, describe, expect, it, vi } from "vitest";
import { httpQuarryApi } from "@/api/httpQuarryApi";
import { createTauriQuarryApi } from "@/api/tauriQuarryApi";
import { isDealDeleted, subscribeDealDeletion } from "@/data/dealDeletion/lifecycle";
import { waitForDealDeletion } from "@/api/dealDeletion";
import { sessionStore } from "@/auth/sessionStore";
import type { DealDeletion } from "@/contracts/quarryApi";
function receipt(id = "deal/one"): DealDeletion {
  return { deletionId: "deletion-one", dealId: id, status: "pending", phase: "draining", retryable: false, statusUrl: "/api/v1/deal-deletions/deletion-one", errorCode: null, createdAt: "now", updatedAt: "now", completedAt: null };
}
afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); sessionStore.set(null); });
describe("permanent deletion transport and lifecycle", () => {
  it("encodes DELETE, preserves pending status, and invalidates after acceptance", async () => {
    const listener = vi.fn(); const stop = subscribeDealDeletion(listener);
    const fetchMock = vi.fn().mockResolvedValue(Response.json(receipt(), { status: 202 })); vi.stubGlobal("fetch", fetchMock);
    await expect(httpQuarryApi.deleteDeal("deal/one")).resolves.toEqual(receipt());
    expect(fetchMock).toHaveBeenCalledWith("/api/v1/deals/deal%2Fone", expect.objectContaining({ method: "DELETE" }));
    expect(isDealDeleted("deal/one")).toBe(true); expect(listener).toHaveBeenCalledWith("deal/one"); stop();
  });
  it("treats 204 as completed without parsing JSON", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 204 })));
    await expect(httpQuarryApi.deleteDeal("finished")).resolves.toBeNull();
  });
  it.each([200, 409, 503])("rejects status %s without invalidating local data", async (status) => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json({ error: "cleanup unavailable" }, { status })));
    await expect(httpQuarryApi.deleteDeal("unchanged")).rejects.toThrow();
    expect(isDealDeleted("unchanged")).toBe(false);
  });
  it("rejects empty 202 instead of claiming completion", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(null, { status: 202 })));
    await expect(httpQuarryApi.deleteDeal("unchanged")).rejects.toThrow();
    expect(isDealDeleted("unchanged")).toBe(false);
  });
  it("uses a separate typed desktop DELETE without changing void deletes", async () => {
    const deleteJson = vi.fn().mockResolvedValue(receipt()); const remove = vi.fn().mockResolvedValue(undefined);
    const api = createTauriQuarryApi({ delete: remove, deleteJson, get: vi.fn(), getPdf: vi.fn(), post: vi.fn(), postMultipart: vi.fn(), postPowerPoint: vi.fn(), subscribeJob: vi.fn() });
    await expect(api.deleteDeal("deal/one")).resolves.toEqual(receipt());
    expect(deleteJson).toHaveBeenCalledWith("/api/v1/deals/deal%2Fone"); expect(remove).not.toHaveBeenCalled();
    deleteJson.mockResolvedValue(null); await expect(api.deleteDeal("deal/one")).resolves.toBeNull();
  });
  it("ignores acceptance from an obsolete session", async () => {
    let resolve!: (response: Response) => void;
    vi.stubGlobal("fetch", vi.fn(() => new Promise<Response>((done) => { resolve = done; })));
    const pending = httpQuarryApi.deleteDeal("stale"); sessionStore.set(null); resolve(Response.json(receipt("stale"), { status: 202 }));
    await expect(pending).rejects.toThrow(); expect(isDealDeleted("stale")).toBe(false);
  });
  it("cancels local polling without another DELETE or status request", async () => {
    vi.useFakeTimers(); const getDealDeletion = vi.fn(); const controller = new AbortController();
    const pending = waitForDealDeletion({ getDealDeletion }, receipt(), controller.signal);
    const rejected = expect(pending).rejects.toBeDefined(); controller.abort(); await rejected;
    await vi.runOnlyPendingTimersAsync(); expect(getDealDeletion).not.toHaveBeenCalled();
  });
  it("stops polling at blocked and completed receipts", async () => {
    for (const status of ["blocked", "completed"] as const) {
      const getDealDeletion = vi.fn(); await expect(waitForDealDeletion({ getDealDeletion }, { ...receipt(), status }, new AbortController().signal)).resolves.toMatchObject({ status }); expect(getDealDeletion).not.toHaveBeenCalled();
    }
  });
});
