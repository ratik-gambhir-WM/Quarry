import type { CacheAdapter, SharePointSearchOptions } from "../types";
import { fetchWithRetry } from "../core/utils";

const SEARCH_URL = "https://graph.microsoft.com/v1.0/search/query";

/** Execute a SharePoint search query via the Microsoft Graph Search API. */
export async function searchSharePoint(
  graphToken: string,
  options: SharePointSearchOptions,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<unknown> {
  const cacheKey = `search:${JSON.stringify(options)}`;
  const cached = await cache.get<unknown>(cacheKey);
  if (cached) return cached;

  const response = await fetchWithRetry(SEARCH_URL, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Authorization: graphToken,
    },
    body: JSON.stringify({
      requests: [
        {
          entityTypes: options.entityTypes,
          query: { queryString: options.query },
          from: options.from ?? 0,
          size: options.size ?? 5,
        },
      ],
    }),
  });

  const result = await response.json();
  await cache.set(cacheKey, result, cacheTtlMs);
  return result;
}

/** Search SharePoint files (listItem entity type). */
export async function searchFiles(
  graphToken: string,
  query: string,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<unknown> {
  return searchSharePoint(
    graphToken,
    { query, entityTypes: ["listItem"] },
    cache,
    cacheTtlMs,
  );
}

/** Search SharePoint sites. */
export async function searchSites(
  graphToken: string,
  query: string,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<unknown> {
  return searchSharePoint(
    graphToken,
    { query, entityTypes: ["site"] },
    cache,
    cacheTtlMs,
  );
}

/** Search SharePoint folders (list entity type). */
export async function searchFolders(
  graphToken: string,
  query: string,
  cache: CacheAdapter,
  cacheTtlMs: number,
): Promise<unknown> {
  return searchSharePoint(
    graphToken,
    { query, entityTypes: ["list"] },
    cache,
    cacheTtlMs,
  );
}
