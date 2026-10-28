import type { SessionResponse } from "../contracts/quarryApi";
import { clearActivityLog } from "../lib/activityLog";

export type SessionState = {
  generation: number;
  session: SessionResponse | null;
  notice?: string;
};

let state: SessionState = { generation: 0, session: null };
let expiry: ReturnType<typeof setTimeout> | undefined;
const listeners = new Set<() => void>();
const requests = new Set<AbortController>();

export const sessionStore = {
  getSnapshot: () => state,
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  },
  set(session: SessionResponse | null, notice?: string) {
    if (expiry) clearTimeout(expiry);
    for (const request of requests) request.abort();
    requests.clear();
    state = { generation: state.generation + 1, session, notice };
    clearActivityLog();
    try {
      window.sessionStorage.removeItem("quarry.workspace.email");
    } catch {
      // Session state remains usable when browser storage is unavailable.
    }
    const generation = state.generation;
    if (session) {
      expiry = setTimeout(
        () => sessionStore.clearIfCurrent(generation, "Your session expired. Sign in again."),
        Math.max(0, Date.parse(session.expiresAt) - Date.now()),
      );
    }
    listeners.forEach((listener) => listener());
  },
  clearIfCurrent(generation: number, notice?: string) {
    if (state.generation === generation) sessionStore.set(null, notice);
  },
  track(controller: AbortController) {
    requests.add(controller);
    return () => { requests.delete(controller); };
  },
};

export function credentials() {
  const { session, generation } = state;
  return {
    auth: session ? { token: session.accessToken, userId: session.user.id } : null,
    generation,
  };
}
