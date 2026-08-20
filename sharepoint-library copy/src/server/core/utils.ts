import { SharePointClientError } from "./errors";
import type { RetryOptions } from "../types";

const DEFAULT_MAX_RETRIES = 10;
const DEFAULT_RETRY_DELAY_MS = 1_000;
const NON_RETRIABLE_STATUS_CODES = new Set([400, 401, 403, 404]);

function shouldRetryStatus(status: number): boolean {
  return !NON_RETRIABLE_STATUS_CODES.has(status);
}

async function readErrorBody(response: Response): Promise<string | undefined> {
  const text = await response.text().catch(() => "");
  const trimmed = text.trim();
  return trimmed ? trimmed.slice(0, 1_000) : undefined;
}

async function createHttpError(response: Response, maxRetries: number, url: string): Promise<SharePointClientError> {
  const body = await readErrorBody(response);
  const retryText = shouldRetryStatus(response.status)
    ? `exceeded ${maxRetries} retries`
    : "not retryable";
  const details = body ? `: ${body}` : "";
  return new SharePointClientError(
    `HTTP ${response.status}: ${retryText} for ${url}${details}`,
    response.status,
    body,
  );
}

/**
 * Performs a fetch with automatic retry on failure.
 * Respects the `Retry-After` response header when present.
 */
export async function fetchWithRetry(
  url: string,
  options: RequestInit,
  retryOptions?: RetryOptions,
): Promise<Response> {
  const maxRetries = retryOptions?.maxRetries ?? DEFAULT_MAX_RETRIES;
  const retryDelayMs = retryOptions?.retryDelayMs ?? DEFAULT_RETRY_DELAY_MS;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    const response = await fetch(url, options);
    if (response.ok) return response;

    if (!shouldRetryStatus(response.status) || attempt === maxRetries) {
      throw await createHttpError(response, maxRetries, url);
    }

    const retryAfter = response.headers.get("Retry-After");
    const delay = retryAfter ? Number(retryAfter) * 1_000 : retryDelayMs;
    await new Promise((resolve) => setTimeout(resolve, delay));
  }

  throw new SharePointClientError(`Exceeded maximum retries for ${url}`);
}

/**
 * Parses a SharePoint folder URL into its relative path component.
 * Handles both "Shared Documents" and Teams channel folder structures.
 */
export function parseSharePointFolderPath(sharepointFolderURL: string): string {
  const cleaned = sharepointFolderURL.replace("/:f:/r/", "/").split("?")[0];
  const urlObj = new URL(cleaned);
  const fullPath = decodeURIComponent(urlObj.pathname);

  const markers = ["/Shared Documents/", "@thread.tacv2/"];
  for (const marker of markers) {
    const idx = fullPath.indexOf(marker);
    if (idx !== -1) {
      return "/" + fullPath.substring(idx + marker.length);
    }
  }
  return fullPath;
}

/** Trims leading/trailing slashes and lowercases the path. */
export function normalizePath(path: string): string {
  return path.replace(/^\/+|\/+$/g, "").toLowerCase();
}

/** Checks whether a normalized path matches any entry in an exclusion list. */
export function isPathExcluded(
  normalizedPath: string,
  exclusions?: string[],
): boolean {
  return (
    exclusions?.some(
      (excluded) => normalizePath(excluded) === normalizedPath,
    ) ?? false
  );
}

/** Lowercases the file extension while preserving the base name casing. */
export function normalizeFileExtension(filename: string): string {
  const lastDot = filename.lastIndexOf(".");
  if (lastDot === -1) return filename;
  return (
    filename.substring(0, lastDot) + filename.substring(lastDot).toLowerCase()
  );
}

/** Builds the Graph API URL for listing folder children. */
export function buildDriveChildrenUrl(
  driveId: string,
  folderPath: string,
  pageSize = 500,
): string {
  return `https://graph.microsoft.com/v1.0/drives/${driveId}/root:${encodeURIComponent(folderPath)}:/children?$top=${pageSize}`;
}

/**
 * Returns a stable, collision-resistant namespace derived from a Graph token,
 * suitable for scoping cache keys per caller. Uses SHA-256 so that cache
 * entries from one caller cannot be returned to another caller sharing the
 * same cache backend.
 */
export async function hashToken(token: string): Promise<string> {
  const data = new TextEncoder().encode(token);
  const digest = await crypto.subtle.digest("SHA-256", data);
  const bytes = new Uint8Array(digest);
  let hex = "";
  for (let i = 0; i < bytes.length; i++) {
    hex += bytes[i].toString(16).padStart(2, "0");
  }
  return hex;
}
