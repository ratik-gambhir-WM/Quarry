import type { DealDeletion, QuarryApi } from "../contracts/quarryApi";
import { sessionStore } from "../auth/sessionStore";

/** Cancelling polling never sends a cancellation request for accepted deletion. */
export async function waitForDealDeletion(api: Pick<QuarryApi, "getDealDeletion">, operation: DealDeletion, signal: AbortSignal): Promise<DealDeletion> {
  const generation = sessionStore.getSnapshot().generation;
  let current = operation;
  for (;;) {
    signal.throwIfAborted();
    if (sessionStore.getSnapshot().generation !== generation) throw new Error("Your session changed. Sign in to check deletion status.");
    if (current.status === "completed" || current.status === "blocked") return current;
    await new Promise<void>((resolve, reject) => {
      const stop = () => { clearTimeout(timer); signal.removeEventListener("abort", stop); reject(signal.reason); };
      const timer = setTimeout(() => { signal.removeEventListener("abort", stop); resolve(); }, 1000);
      signal.addEventListener("abort", stop, { once: true });
    });
    current = await api.getDealDeletion(current.deletionId);
  }
}
