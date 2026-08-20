import { useSyncExternalStore } from "react";

export type ActivityLogSource = "api" | "event" | "ipc" | "sse";
export type ActivityLogStatus = "error" | "info" | "pending" | "success";

export type ActivityLogEntry = {
  details?: string;
  durationMs?: number;
  eventName?: string;
  httpStatus?: number;
  id: string;
  method?: string;
  occurredAt: string;
  operation?: string;
  source: ActivityLogSource;
  status: ActivityLogStatus;
  title: string;
  url?: string;
};

type ApiRequestStart = {
  method: string;
  request?: unknown;
  url: string;
};

type ApiRequestFinish = {
  details?: unknown;
  durationMs: number;
  httpStatus?: number;
  message?: string;
  status: "error" | "success";
};

type SseLogInput = {
  data?: unknown;
  eventName: string;
  status: Exclude<ActivityLogStatus, "pending">;
  title: string;
  url: string;
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
const absolutePathPattern = /(?:\/Users\/|\/home\/|\/var\/|[A-Za-z]:[\\/]|\\\\)[^\s]*/g;

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
    if (!Array.isArray(parsed)) {
      return [];
    }

    return parsed.slice(0, MAX_ENTRIES) as ActivityLogEntry[];
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
    // Logging must never interrupt the operation it is observing.
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

function getSnapshot() {
  return entries;
}

function getServerSnapshot(): ActivityLogEntry[] {
  return [];
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useActivityLogEntries() {
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

export function getActivityLogEntries() {
  return entries;
}

export function beginApiRequest({ method, request, url }: ApiRequestStart) {
  const id = createId();
  const entry: ActivityLogEntry = {
    details: request === undefined ? undefined : serializeForLog({ request }),
    id,
    method,
    occurredAt: new Date().toISOString(),
    source: "api",
    status: "pending",
    title: `${method} ${displayUrl(url)}`,
    url: sanitizeUrl(url),
  };

  publish([entry, ...entries]);
  return id;
}

export function finishApiRequest(id: string, result: ApiRequestFinish) {
  const nextEntries = entries.map((entry) => {
    if (entry.id !== id) {
      return entry;
    }

    return {
      ...entry,
      details: result.details === undefined
        ? entry.details
        : [entry.details, serializeForLog({ response: result.details })].filter(Boolean).join("\n\n"),
      durationMs: Math.round(result.durationMs),
      httpStatus: result.httpStatus,
      status: result.status,
      title: result.message
        ? `${entry.method} ${displayUrl(entry.url ?? "")} — ${sanitizeString(result.message)}`
        : entry.title,
    };
  });

  publish(nextEntries);
}

export function beginIpcRequest(operation: string, request?: unknown) {
  const id = createId();
  publish([
    {
      details: request === undefined ? undefined : serializeForLog({ request }),
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
        result.details === undefined ? undefined : serializeForLog({ response: result.details });
      return {
        ...entry,
        details: [entry.details, response].filter(Boolean).join("\n\n") || undefined,
        durationMs: Math.round(result.durationMs),
        status: result.status,
        title: result.message
          ? `${entry.operation ?? entry.title} — ${sanitizeString(result.message)}`
          : entry.title,
      };
    }),
  );
}

export function logSseEvent({ data, eventName, status, title, url }: SseLogInput) {
  const entry: ActivityLogEntry = {
    details: data === undefined ? undefined : serializeForLog(data),
    eventName,
    id: createId(),
    occurredAt: new Date().toISOString(),
    source: "sse",
    status,
    title,
    url: sanitizeUrl(url),
  };

  publish([entry, ...entries]);
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

export function exportActivityLog() {
  const payload = buildActivityLogExport("browser-tab");
  const blob = new Blob([payload], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.download = `quarry-session-log-${new Date().toISOString().replace(/[:.]/g, "-")}.json`;
  anchor.href = url;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function buildActivityLogExport(session = "client-session") {
  return JSON.stringify(
    {
      entries,
      exportedAt: new Date().toISOString(),
      schemaVersion: 1,
      session,
    },
    null,
    2,
  );
}

export function summarizeFormData(formData: FormData) {
  const summary: Record<string, unknown> = {};

  for (const [key, value] of formData.entries()) {
    const summarizedValue = value instanceof File
      ? {
          lastModified: value.lastModified,
          name: value.name,
          sizeBytes: value.size,
          type: value.type || "unknown",
        }
      : value;
    const existing = summary[key];
    summary[key] = existing === undefined
      ? summarizedValue
      : Array.isArray(existing)
        ? [...existing, summarizedValue]
        : [existing, summarizedValue];
  }

  return summary;
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

    const sanitized = value.slice(0, MAX_ARRAY_ITEMS).map((item) => sanitizeValue(item, depth + 1));
    if (value.length > MAX_ARRAY_ITEMS) {
      sanitized.push(`[${value.length - MAX_ARRAY_ITEMS} more items]`);
    }
    return sanitized;
  }

  if (typeof value === "object") {
    if (depth >= MAX_DEPTH) {
      return "[Nested object]";
    }

    const result: Record<string, unknown> = {};
    const objectEntries = Object.entries(value as Record<string, unknown>);
    for (const [key, nestedValue] of objectEntries.slice(0, MAX_OBJECT_KEYS)) {
      result[key] = sensitiveKeyPattern.test(key) ? REDACTED : sanitizeValue(nestedValue, depth + 1);
    }
    if (objectEntries.length > MAX_OBJECT_KEYS) {
      result.__truncated = `${objectEntries.length - MAX_OBJECT_KEYS} more keys`;
    }
    return result;
  }

  return String(value);
}

function sanitizeUrl(url: string) {
  try {
    const parsed = new URL(url, typeof window === "undefined" ? "http://localhost" : window.location.origin);
    parsed.searchParams.forEach((_, key) => {
      if (sensitiveKeyPattern.test(key)) {
        parsed.searchParams.set(key, REDACTED);
      }
    });
    return `${parsed.pathname}${parsed.search}`;
  } catch {
    return url;
  }
}

function displayUrl(url: string) {
  return sanitizeUrl(url).split("?")[0];
}

function sanitizeString(value: string) {
  const redacted = value
    .replace(emailPattern, REDACTED)
    .replace(absolutePathPattern, REDACTED);
  return redacted.length > MAX_STRING_LENGTH
    ? `${redacted.slice(0, MAX_STRING_LENGTH)}… [truncated ${redacted.length - MAX_STRING_LENGTH} characters]`
    : redacted;
}
