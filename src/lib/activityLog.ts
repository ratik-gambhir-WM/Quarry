import { useSyncExternalStore } from "react";

export type ActivityLogSource = "event" | "ipc";
export type ActivityLogStatus = "error" | "info" | "pending" | "success";

export type ActivityLogEntry = {
  details?: string;
  durationMs?: number;
  eventName?: string;
  id: string;
  occurredAt: string;
  operation: string;
  source: ActivityLogSource;
  status: ActivityLogStatus;
  title: string;
};

type IpcRequestFinish = {
  details?: unknown;
  durationMs: number;
  message?: string;
  status: "error" | "success";
};

const STORAGE_KEY = "quarry.activity-log:v1";
const MAX_ENTRIES = 400;
const MAX_DEPTH = 5;
const MAX_ARRAY_ITEMS = 30;
const MAX_OBJECT_KEYS = 50;
const MAX_STRING_LENGTH = 2_000;
const REDACTED = "[REDACTED]";
const sensitiveKeyPattern = /api[-_]?key|authorization|base64|content|cookie|email|file(name)?|folder|password|path|payload|question|secret|summary|text|token/i;
const emailPattern = /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi;
const absolutePathPattern = /^(?:\/|[A-Za-z]:[\\/]|\\\\)/;

let entries = readStoredEntries();
const listeners = new Set<() => void>();

function readStoredEntries(): ActivityLogEntry[] {
  if (typeof window === "undefined") {
    return [];
  }

  try {
    const stored = window.sessionStorage.getItem(STORAGE_KEY);
    if (!stored) {
      return [];
    }
    const parsed = JSON.parse(stored) as unknown;
    return Array.isArray(parsed)
      ? (parsed.slice(0, MAX_ENTRIES) as ActivityLogEntry[])
      : [];
  } catch {
    return [];
  }
}

function persistEntries() {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.sessionStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    // Activity capture must never break the operation being observed.
  }
}

function publish(nextEntries: ActivityLogEntry[]) {
  entries = nextEntries.slice(0, MAX_ENTRIES);
  persistEntries();
  listeners.forEach((listener) => listener());
}

function createId() {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function getServerSnapshot(): ActivityLogEntry[] {
  return [];
}

export function getActivityLogEntries() {
  return entries;
}

export function useActivityLogEntries() {
  return useSyncExternalStore(subscribe, getActivityLogEntries, getServerSnapshot);
}

export function beginIpcRequest(operation: string, request?: unknown) {
  const id = createId();
  publish([
    {
      details:
        request === undefined ? undefined : serializeForLog({ request }),
      id,
      occurredAt: new Date().toISOString(),
      operation,
      source: "ipc",
      status: "pending",
      title: operation,
    },
    ...entries,
  ]);
  return id;
}

export function finishIpcRequest(id: string, result: IpcRequestFinish) {
  publish(
    entries.map((entry) => {
      if (entry.id !== id) {
        return entry;
      }

      const response =
        result.details === undefined
          ? undefined
          : serializeForLog({ response: result.details });
      return {
        ...entry,
        details: [entry.details, response].filter(Boolean).join("\n\n") || undefined,
        durationMs: Math.round(result.durationMs),
        status: result.status,
        title: result.message
          ? `${entry.operation} — ${sanitizeString(result.message)}`
          : entry.title,
      };
    }),
  );
}

export function logTauriEvent(
  eventName: string,
  status: Exclude<ActivityLogStatus, "pending">,
  details?: unknown,
) {
  publish([
    {
      details: details === undefined ? undefined : serializeForLog(details),
      eventName,
      id: createId(),
      occurredAt: new Date().toISOString(),
      operation: eventName,
      source: "event",
      status,
      title: `${eventName} event`,
    },
    ...entries,
  ]);
}

export function clearActivityLog() {
  publish([]);
}

export function buildActivityLogExport() {
  return JSON.stringify(
    {
      entries,
      exportedAt: new Date().toISOString(),
      schemaVersion: 1,
      session: "desktop-webview",
    },
    null,
    2,
  );
}

function serializeForLog(value: unknown) {
  try {
    return JSON.stringify(sanitizeValue(value), null, 2);
  } catch {
    return "[Unable to serialize details]";
  }
}

function sanitizeValue(value: unknown, depth = 0): unknown {
  if (value === null || typeof value === "number" || typeof value === "boolean") {
    return value;
  }

  if (typeof value === "string") {
    return sanitizeString(value);
  }

  if (value instanceof Error) {
    return { message: sanitizeString(value.message), name: value.name };
  }

  if (Array.isArray(value)) {
    if (depth >= MAX_DEPTH) {
      return `[Array with ${value.length} items]`;
    }
    const sanitized = value
      .slice(0, MAX_ARRAY_ITEMS)
      .map((item) => sanitizeValue(item, depth + 1));
    if (value.length > MAX_ARRAY_ITEMS) {
      sanitized.push(`[${value.length - MAX_ARRAY_ITEMS} more items]`);
    }
    return sanitized;
  }

  if (typeof value === "object") {
    if (depth >= MAX_DEPTH) {
      return "[Nested object]";
    }
    const objectEntries = Object.entries(value as Record<string, unknown>);
    const result: Record<string, unknown> = {};
    for (const [key, nestedValue] of objectEntries.slice(0, MAX_OBJECT_KEYS)) {
      result[key] = sensitiveKeyPattern.test(key)
        ? REDACTED
        : sanitizeValue(nestedValue, depth + 1);
    }
    if (objectEntries.length > MAX_OBJECT_KEYS) {
      result.__truncated = `${objectEntries.length - MAX_OBJECT_KEYS} more keys`;
    }
    return result;
  }

  return String(value);
}

function sanitizeString(value: string) {
  if (absolutePathPattern.test(value.trim())) {
    return REDACTED;
  }
  const redacted = value.replace(emailPattern, REDACTED);
  return redacted.length > MAX_STRING_LENGTH
    ? `${redacted.slice(0, MAX_STRING_LENGTH)}… [truncated ${redacted.length - MAX_STRING_LENGTH} characters]`
    : redacted;
}
