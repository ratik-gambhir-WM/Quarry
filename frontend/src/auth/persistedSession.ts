import type { SessionResponse } from "../contracts/quarryApi";

export const SESSION_STORAGE_KEY = "quarry.auth.session.v1";

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function parseSession(value: unknown): SessionResponse | null {
  if (!record(value) || value.tokenType !== "Bearer" ||
    typeof value.accessToken !== "string" || !/^[A-Za-z0-9_-]{43}$/.test(value.accessToken) ||
    typeof value.expiresAt !== "string" || !Number.isFinite(Date.parse(value.expiresAt)) ||
    Date.parse(value.expiresAt) <= Date.now() || !record(value.user)) return null;
  const user = value.user;
  if (typeof user.id !== "number" || !Number.isSafeInteger(user.id) || user.id <= 0 ||
    typeof user.username !== "string" || typeof user.email !== "string" ||
    typeof user.firstName !== "string" || typeof user.lastName !== "string" ||
    typeof user.createdAt !== "string" || typeof user.updatedAt !== "string" ||
    !(user.jobTitle === null || typeof user.jobTitle === "string")) return null;
  return {
    accessToken: value.accessToken,
    tokenType: "Bearer",
    expiresAt: value.expiresAt,
    user: {
      id: user.id, username: user.username, email: user.email,
      firstName: user.firstName, lastName: user.lastName, jobTitle: user.jobTitle,
      createdAt: user.createdAt, updatedAt: user.updatedAt,
    },
  };
}

export function readPersistedSession(): SessionResponse | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(SESSION_STORAGE_KEY);
    if (!raw) return null;
    const session = parseSession(JSON.parse(raw));
    if (!session) window.localStorage.removeItem(SESSION_STORAGE_KEY);
    return session;
  } catch {
    try { window.localStorage.removeItem(SESSION_STORAGE_KEY); } catch { /* Storage can be unavailable. */ }
    return null;
  }
}

export function persistSession(session: SessionResponse | null) {
  if (typeof window === "undefined") return;
  try {
    const valid = session ? parseSession(session) : null;
    if (valid) window.localStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(valid));
    else window.localStorage.removeItem(SESSION_STORAGE_KEY);
  } catch {
    // Sign-in still works in memory when the browser blocks persistent storage.
  }
}
