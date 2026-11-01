// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testSession } from "@/test/fixtures/auth";
import { persistSession, readPersistedSession, SESSION_STORAGE_KEY } from "@/auth/persistedSession";

const stores: Array<typeof import("@/auth/sessionStore").sessionStore> = [];
beforeEach(() => {
  vi.useFakeTimers();
  window.localStorage.clear();
});
afterEach(() => {
  stores.forEach((store) => store.set(null));
  stores.length = 0;
  vi.restoreAllMocks();
  vi.useRealTimers();
  window.localStorage.clear();
});
async function reloadStore() {
  vi.resetModules();
  const { sessionStore } = await import("@/auth/sessionStore");
  stores.push(sessionStore);
  return sessionStore;
}

describe("session persistence", () => {
  it("restores a signed-in session after reload without extending its expiry", async () => {
    const first = await reloadStore();
    first.set(testSession);
    vi.advanceTimersByTime(1000);
    const restored = await reloadStore();
    expect(restored.getSnapshot().session).toEqual(testSession);
    vi.advanceTimersByTime(Date.parse(testSession.expiresAt) - Date.now());
    expect(restored.getSnapshot().session).toBeNull();
    expect(window.localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });

  it("removes the persisted session on sign-out", async () => {
    const store = await reloadStore();
    store.set(testSession);
    store.set(null);
    expect((await reloadStore()).getSnapshot().session).toBeNull();
  });

  it.each([
    "not json",
    JSON.stringify({ ...testSession, expiresAt: "invalid" }),
    JSON.stringify({ ...testSession, expiresAt: "2000-01-01T00:00:00Z" }),
    JSON.stringify({ ...testSession, accessToken: "invalid" }),
    JSON.stringify({ ...testSession, user: { ...testSession.user, id: -1 } }),
  ])("discards malformed or expired stored sessions (%#)", (raw) => {
    window.localStorage.setItem(SESSION_STORAGE_KEY, raw);
    expect(readPersistedSession()).toBeNull();
    expect(window.localStorage.getItem(SESSION_STORAGE_KEY)).toBeNull();
  });

  it("does not persist extra fields such as a password", () => {
    const withExtraField = { ...testSession, user: { ...testSession.user, password: "must not persist" } };
    persistSession(withExtraField);
    expect(window.localStorage.getItem(SESSION_STORAGE_KEY)).not.toContain("password");
  });

  it("keeps sign-in usable when persistent storage is blocked", async () => {
    const store = await reloadStore();
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("blocked"); });
    expect(() => store.set(testSession)).not.toThrow();
    expect(store.getSnapshot().session).toEqual(testSession);
  });

  it("clears another tab and its requests when the stored session is removed", async () => {
    const store = await reloadStore();
    store.set(testSession);
    const request = new AbortController();
    store.track(request);
    const stop = store.listenForStorageChanges();
    window.localStorage.removeItem(SESSION_STORAGE_KEY);
    window.dispatchEvent(new StorageEvent("storage", { key: SESSION_STORAGE_KEY }));
    expect(store.getSnapshot().session).toBeNull();
    expect(request.signal.aborted).toBe(true);
    stop();
  });
});
