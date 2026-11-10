import { sessionStore } from "../../auth/sessionStore";
import type { DealDeletion } from "../../contracts/quarryApi";

const deleted = new Set<string>();
const listeners = new Set<(dealId: string) => void>();
let generation = sessionStore.getSnapshot().generation;
sessionStore.subscribe(() => {
  const next = sessionStore.getSnapshot().generation;
  if (generation !== next) { deleted.clear(); generation = next; }
});
export function isDealDeleted(id: string) { return deleted.has(id); }
export function subscribeDealDeletion(listener: (dealId: string) => void) {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}
/** Publish only responses from the same authenticated session that issued the request. */
export async function acceptDealDeletion(id: string, request: () => Promise<DealDeletion | null>) {
  const started = sessionStore.getSnapshot().generation;
  const result = await request();
  if (sessionStore.getSnapshot().generation !== started) throw new Error("Your session changed. Sign in to check deletion status.");
  if (result !== null && (result.dealId !== id || !result.deletionId || !["pending", "running", "retrying", "blocked", "completed"].includes(result.status))) throw new Error("The server returned an invalid deletion receipt.");
  deleted.add(id);
  for (const listener of listeners) listener(id);
  return result;
}
