import { SharePointClientError } from "../core/errors";
import type {
  CacheAdapter,
  DiffOptions,
  DownloadResult,
  DriveItem,
  DriveItemFilterOptions,
  FileDiff,
  FileSyncResult,
  GraphDownloadMetadataResponse,
  GraphDriveChildrenResponse,
  TeamIdentifier,
} from "../types";
import { getDriveId, checkFolderExists } from "./drives";
import {
  fetchWithRetry,
  buildDriveChildrenUrl,
  normalizePath,
  isPathExcluded,
  normalizeFileExtension,
  parseSharePointFolderPath,
} from "../core/utils";

const DEFAULT_EXCLUDED_EXTENSIONS = [".mp4", ".zip"];

/**
 * Recursively lists all files under a drive folder path.
 * Returns a flat `DriveItem[]` — no array mutation.
 */
export async function listFiles(
  graphToken: string,
  driveId: string,
  folderPath: string,
  options?: DriveItemFilterOptions,
  rootPath: string = folderPath,
): Promise<DriveItem[]> {
  const excludedExtensions =
    options?.excludedExtensions ?? DEFAULT_EXCLUDED_EXTENSIONS;
  const results: DriveItem[] = [];

  let nextLink: string | null = buildDriveChildrenUrl(driveId, folderPath);

  while (nextLink) {
    const response = await fetchWithRetry(nextLink, {
      headers: { Authorization: graphToken },
    });
    const data = (await response.json()) as GraphDriveChildrenResponse;

    for (const item of data.value ?? []) {
      const fullPath = `${folderPath}/${item.name}`;
      const relativePath = fullPath.replace(
        new RegExp(`^${rootPath}/?`, "i"),
        "",
      );
      const normalizedPath = normalizePath(relativePath);

      if (item.folder) {
        if (isPathExcluded(normalizedPath, options?.excludedFolders)) continue;
        const subFiles = await listFiles(
          graphToken,
          driveId,
          fullPath,
          options,
          rootPath,
        );
        results.push(...subFiles);
        continue;
      }

      if (isPathExcluded(normalizedPath, options?.excludedFiles)) continue;

      const fileName = normalizeFileExtension(item.name);
      if (
        excludedExtensions.some((ext) => fileName.toLowerCase().endsWith(ext))
      ) {
        continue;
      }

      results.push({
        id: item.id,
        name: fileName,
        webUrl: item.webUrl,
        size: item.size,
        mimeType: item.file?.mimeType ?? "",
        lastModifiedDateTime: item.lastModifiedDateTime,
        relativePath: relativePath.split("/").slice(0, -1).join("/"),
      });
    }

    nextLink = data["@odata.nextLink"] ?? null;
  }

  return results;
}

/**
 * Generic file diff — compares two sets using caller-supplied identity and update logic.
 * No Graph API calls; pure computation.
 */
export function diffFiles<TNew, TExisting>(
  newFiles: TNew[],
  existingFiles: TExisting[],
  options: DiffOptions<TNew, TExisting>,
): FileDiff<TNew, TExisting> {
  const { getNewId, getExistingId, shouldUpdate } = options;

  const existingMap = new Map(existingFiles.map((f) => [getExistingId(f), f]));
  const newMap = new Map(newFiles.map((f) => [getNewId(f), f]));

  const added = newFiles.filter((f) => !existingMap.has(getNewId(f)));
  const removed = existingFiles.filter((f) => !newMap.has(getExistingId(f)));
  const modified: [TNew, TExisting][] = [];

  if (shouldUpdate) {
    for (const newFile of newFiles) {
      const existing = existingMap.get(getNewId(newFile));
      if (existing && shouldUpdate(newFile, existing)) {
        modified.push([newFile, existing]);
      }
    }
  }

  return { added, modified, removed };
}

/** Download a file's binary content by its drive item ID. NOT cached. */
export async function downloadFile(
  graphToken: string,
  driveId: string,
  itemId: string,
): Promise<DownloadResult> {
  const metadataUrl = `https://graph.microsoft.com/v1.0/drives/${driveId}/items/${itemId}?select=id,@microsoft.graph.downloadUrl`;

  const metadataResponse = await fetchWithRetry(metadataUrl, {
    headers: { Authorization: graphToken },
  });

  const metadata =
    (await metadataResponse.json()) as GraphDownloadMetadataResponse;
  const downloadUrl = metadata["@microsoft.graph.downloadUrl"];
  if (!downloadUrl) {
    throw new SharePointClientError(
      "Missing download URL from Graph API response",
    );
  }

  const fileResponse = await fetchWithRetry(downloadUrl, {
    headers: { Authorization: graphToken },
  });

  const blob = await fileResponse.blob();
  return { buffer: await blob.arrayBuffer(), size: blob.size };
}

/**
 * High-level orchestrator: resolves the drive, lists files, diffs against existing set.
 * Generic over the existing file type.
 */
export async function getFilesForSync<TExisting>(
  graphToken: string,
  team: TeamIdentifier,
  sharepointFolderURL: string,
  existingFiles: TExisting[],
  filterOptions: DriveItemFilterOptions | undefined,
  diffOpts: DiffOptions<DriveItem, TExisting>,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<FileSyncResult<TExisting>> {
  const driveId = await getDriveId(graphToken, team, cache, cacheTtlMs);
  const relativePath = parseSharePointFolderPath(sharepointFolderURL);
  const files = await listFiles(
    graphToken,
    driveId,
    relativePath,
    filterOptions,
  );
  const diff = diffFiles(files, existingFiles, diffOpts);
  return { diff, driveId };
}

/**
 * High-level convenience: checks whether a SharePoint folder exists given a URL, team, and token.
 */
export async function checkSharePointFolderExists(
  graphToken: string,
  team: TeamIdentifier,
  sharepointFolderURL: string,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<boolean> {
  const driveId = await getDriveId(graphToken, team, cache, cacheTtlMs);
  const folderPath = parseSharePointFolderPath(sharepointFolderURL);
  return checkFolderExists(graphToken, driveId, folderPath, cache, cacheTtlMs);
}
