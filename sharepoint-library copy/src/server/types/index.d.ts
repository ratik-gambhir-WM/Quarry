// ── Configuration ───────────────────────────────────────────────────

/** Configuration for the SharePointClient constructor. */
export type SharePointClientConfig = SharePointClientBaseConfig &
  (SharePointClientCredentialsConfig | SharePointClientProxyConfig);

interface SharePointClientBaseConfig {
  /** Optional custom cache adapter. Defaults to InMemoryCache. */
  cache?: CacheAdapter;
  /** Default TTL for cached Graph API responses in milliseconds. Defaults to 300_000 (5 min). */
  defaultCacheTtlMs?: number;
}

interface SharePointClientCredentialsConfig {
  /** Azure AD tenant ID. Required when the client must acquire its own Graph token. */
  tenantId: string;
  /** Azure AD application (client) ID. Required when the client must acquire its own Graph token. */
  clientId: string;
  /** Azure AD client secret. Required when the client must acquire its own Graph token. */
  clientSecret: string;
  /**
   * Optional proxy for Graph requests that require user context. When set,
   * user-scoped calls use `${baseUrl}/users/{userId}/graph/...` without an
   * Authorization header. Non-user Graph calls still use Microsoft Graph
   * directly with a token.
   */
  userGraphProxy?: UserGraphProxyConfig;
}

type SharePointClientProxyConfig = {
  userGraphProxy: UserGraphProxyConfig;
} & Partial<SharePointClientCredentialsConfig>;

/** Configuration for routing user-context Graph calls through a proxy. */
export interface UserGraphProxyConfig {
  /** Root proxy URL, e.g. `https://westmonroe-cloud.com`. */
  baseUrl: string;
  /** User segment used when mapping Graph `/me/...` paths. Defaults to `"me"`. */
  userId?: string;
  /**
   * Optional stable cache namespace for proxy responses. Omit this for
   * request/session-scoped `/users/me` proxy calls to avoid cross-user cache
   * sharing.
   */
  cacheKey?: string;
}

// ── Cache ───────────────────────────────────────────────────────────

/** Pluggable cache backend interface. */
export interface CacheAdapter {
  get<T>(key: string): Promise<T | undefined>;
  set<T>(key: string, value: T, ttlMs?: number): Promise<void>;
  delete(key: string): Promise<void>;
  clear(): Promise<void>;
}

/** Internal cache entry used by InMemoryCache. */
export interface CacheEntry {
  value: unknown;
  expiresAt: number;
}

// ── Teams ───────────────────────────────────────────────────────────

/** Identifies a Microsoft Teams team and optional channel. */
export interface TeamIdentifier {
  teamsId: string;
  channelId?: string;
}

/** A Microsoft Teams team. */
export interface Team {
  id: string;
  displayName: string;
}

/** A team with its resolved channel IDs. */
export interface TeamChannels {
  teamId: string;
  channels: string[];
}

// ── Drive Items ─────────────────────────────────────────────────────

/** A generic SharePoint drive item returned by file-listing operations. */
export interface DriveItem {
  /** SharePoint item ID. */
  id: string;
  /** File or folder name. */
  name: string;
  /** Web URL for the item. */
  webUrl: string;
  /** File size in bytes. */
  size: number;
  /** MIME type (empty string for folders). */
  mimeType: string;
  /** ISO 8601 date string of last modification. */
  lastModifiedDateTime: string;
  /** Path relative to the listed root folder (folder portion only). */
  relativePath: string;
}

/** Options for filtering drive items during listing. */
export interface DriveItemFilterOptions {
  /** Folder paths (relative) to skip during recursive listing. */
  excludedFolders?: string[];
  /** File paths (relative) to skip. */
  excludedFiles?: string[];
  /** File extensions to exclude. Defaults to `[".mp4", ".zip"]`. Pass `[]` to disable. */
  excludedExtensions?: string[];
}

/** Result of downloading a file — buffer and byte size. */
export interface DownloadResult {
  buffer: ArrayBuffer;
  size: number;
}

// ── File Diffing ────────────────────────────────────────────────────

/** Result of diffing two file sets. Fully generic over the item types. */
export interface FileDiff<TNew, TExisting> {
  added: TNew[];
  modified: [TNew, TExisting][];
  removed: TExisting[];
}

/** Options for diffing files — caller supplies identity & update logic. */
export interface DiffOptions<TNew, TExisting> {
  /** Extract a unique identifier from a new item. */
  getNewId: (item: TNew) => string;
  /** Extract a unique identifier from an existing item. */
  getExistingId: (item: TExisting) => string;
  /** Return true if the new item should replace the existing one. */
  shouldUpdate?: (newItem: TNew, existingItem: TExisting) => boolean;
}

/** Result of a file sync operation — diff plus the resolved drive ID. */
export interface FileSyncResult<TExisting> {
  diff: FileDiff<DriveItem, TExisting>;
  driveId: string;
}

// ── Search ──────────────────────────────────────────────────────────

/** Entity types supported by the Microsoft Graph Search API. */
export type SharePointSearchEntityType =
  | "listItem"
  | "site"
  | "list"
  | "drive"
  | "driveItem"
  | "externalItem";

/** Options for a SharePoint search query. */
export interface SharePointSearchOptions {
  query: string;
  entityTypes: SharePointSearchEntityType[];
  from?: number;
  size?: number;
}

// ── HTTP ────────────────────────────────────────────────────────────

/** Retry configuration for HTTP requests. */
export interface RetryOptions {
  /** Maximum number of retries. Defaults to 10. */
  maxRetries?: number;
  /** Default delay between retries in ms (used when no Retry-After header). Defaults to 1000. */
  retryDelayMs?: number;
}

// ── Graph API Response Shapes ───────────────────────────────────────

/** Shape of the Graph API teams response. */
export interface GraphTeamsResponse {
  value: Team[];
}

/** Shape of a single Graph API channel filter response. */
export interface GraphChannelFilterResponse {
  value: unknown[];
}

/** Shape of a Graph API drive folder response. */
export interface GraphDriveFolderResponse {
  parentReference: { driveId: string };
}

/** Shape of a single item in a Graph API drive children listing. */
export interface GraphDriveChildItem {
  id: string;
  name: string;
  webUrl: string;
  size: number;
  lastModifiedDateTime: string;
  file?: { mimeType: string };
  folder?: unknown;
}

/** Paginated response from Graph API drive children endpoint. */
export interface GraphDriveChildrenResponse {
  value: GraphDriveChildItem[];
  "@odata.nextLink"?: string;
}

/** Shape of a Graph API file download metadata response. */
export interface GraphDownloadMetadataResponse {
  "@microsoft.graph.downloadUrl"?: string;
}

/** Shape of a Graph API batch response. */
export interface GraphBatchResponse {
  responses: GraphBatchResponseItem[];
}

/** A single item in a Graph API batch response. */
export interface GraphBatchResponseItem {
  status: number;
  id: string;
  body?: {
    value: Array<{ id: string; webUrl?: string }>;
  };
}

/** Shape of an OAuth2 token response. */
export interface TokenResponse {
  access_token: string;
  expires_in: number;
}

/** Shape of a Graph API error response body. */
export interface GraphErrorResponse {
  error?: { message?: string };
}
