import { SharePointClientError } from "../core/errors";
import type {
  CacheAdapter,
  DriveItem,
  GraphDriveChildrenResponse,
  GraphDriveFolderResponse,
  GraphErrorResponse,
  TeamIdentifier,
} from "../types";
import {
  buildDriveChildrenUrl,
  fetchWithRetry,
  normalizeFileExtension,
} from "../core/utils";

const GRAPH_BASE = "https://graph.microsoft.com/v1.0";

/** Resolve the drive ID for a Teams team or channel file store. */
export async function getDriveId(
  graphToken: string,
  team: TeamIdentifier,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<string> {
  const cacheKey = `drive:${team.teamsId}:${team.channelId ?? "root"}`;
  const cached = await cache.get<string>(cacheKey);
  if (cached) return cached;

  const url = team.channelId
    ? `${GRAPH_BASE}/teams/${team.teamsId}/channels/${team.channelId}/filesFolder`
    : `${GRAPH_BASE}/teams/${team.teamsId}/filesFolder`;

  const response = await fetchWithRetry(url, {
    headers: { Authorization: graphToken },
  });

  const data = (await response.json()) as GraphDriveFolderResponse;
  const driveId = data.parentReference.driveId;

  await cache.set(cacheKey, driveId, cacheTtlMs);
  return driveId;
}

/**
 * Async generator that yields drive items page-by-page from a folder.
 * Follows `@odata.nextLink` for pagination.
 */
export async function* getDriveItemChildren(
  graphToken: string,
  driveId: string,
  folderPath: string,
  pageSize = 500,
): AsyncGenerator<DriveItem> {
  let nextLink: string | null = buildDriveChildrenUrl(
    driveId,
    folderPath,
    pageSize,
  );

  while (nextLink) {
    const response = await fetchWithRetry(nextLink, {
      headers: { Authorization: graphToken },
    });
    const data = (await response.json()) as GraphDriveChildrenResponse;

    for (const item of data.value ?? []) {
      if (item.folder) continue;

      yield {
        id: item.id,
        name: normalizeFileExtension(item.name),
        webUrl: item.webUrl,
        size: item.size,
        mimeType: item.file?.mimeType ?? "",
        lastModifiedDateTime: item.lastModifiedDateTime,
        relativePath: "",
      };
    }

    nextLink = data["@odata.nextLink"] ?? null;
  }
}

/** Check whether a specific folder path exists in a drive. */
export async function checkFolderExists(
  graphToken: string,
  driveId: string,
  folderPath: string,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<boolean> {
  const cacheKey = `folder:${driveId}:${folderPath}`;
  const cached = await cache.get<boolean>(cacheKey);
  if (cached !== undefined) return cached;

  const url = `${GRAPH_BASE}/drives/${driveId}/root:${encodeURIComponent(folderPath)}`;
  const response = await fetch(url, {
    headers: { Authorization: graphToken },
  });

  if (response.ok) {
    await cache.set(cacheKey, true, cacheTtlMs);
    return true;
  }
  if (response.status === 404) {
    await cache.set(cacheKey, false, cacheTtlMs);
    return false;
  }

  const errorData = (await response.json()) as GraphErrorResponse;
  throw new SharePointClientError(
    `Error checking folder: ${errorData.error?.message ?? response.statusText}`,
    response.status,
    errorData,
  );
}
