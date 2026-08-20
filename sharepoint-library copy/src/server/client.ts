import { InMemoryCache } from "./core/cache";
import { TokenManager } from "./core/auth";
import * as teams from "./services/teams";
import * as drives from "./services/drives";
import * as files from "./services/files";
import * as search from "./services/search";
import type {
  CacheAdapter,
  DiffOptions,
  DownloadResult,
  DriveItem,
  DriveItemFilterOptions,
  FileDiff,
  FileSyncResult,
  SharePointClientConfig,
  SharePointSearchOptions,
  Team,
  TeamChannels,
  TeamIdentifier,
  UserGraphProxyConfig,
} from "./types";

const DEFAULT_CACHE_TTL_MS = 300_000; // 5 minutes

/**
 * Entry point for all SharePoint / Microsoft Graph operations.
 *
 * Handles credential management, token auto-refresh, and response caching.
 * Every Graph-calling method accepts an optional `graphToken` parameter —
 * if provided it overrides the auto-managed token (useful for user-delegated tokens).
 */
export class SharePointClient {
  private readonly tokenManager: TokenManager;
  private readonly cache: CacheAdapter;
  private readonly cacheTtlMs: number;
  private readonly userGraphProxy?: UserGraphProxyConfig;

  constructor(config: SharePointClientConfig) {
    this.cache = config.cache ?? new InMemoryCache();
    this.cacheTtlMs = config.defaultCacheTtlMs ?? DEFAULT_CACHE_TTL_MS;
    this.userGraphProxy = config.userGraphProxy;
    this.tokenManager = new TokenManager(config, this.cache);
  }

  /** Resolve a token — uses the explicit override or auto-acquires one. */
  private async resolveToken(graphToken?: string): Promise<string> {
    return graphToken ?? this.tokenManager.getToken();
  }

  /** Resolve user-context auth, proxy mode does not need a Graph token. */
  private async resolveUserGraphToken(
    graphToken?: string,
  ): Promise<string | undefined> {
    return this.userGraphProxy ? graphToken : this.resolveToken(graphToken);
  }

  // ── Token ────────────────────────────────────────────────────────────

  /** Acquire a fresh Graph API token using the configured credentials. */
  async acquireToken(): Promise<string> {
    return this.tokenManager.getToken();
  }

  // ── Cache ────────────────────────────────────────────────────────────

  /** Clear all cached data (tokens, drive IDs, API responses, etc.). */
  async clearCache(): Promise<void> {
    await this.cache.clear();
  }

  // ── Teams ────────────────────────────────────────────────────────────

  /** List all teams the authenticated user has joined. */
  async getJoinedTeams(graphToken?: string): Promise<Team[]> {
    const token = await this.resolveUserGraphToken(graphToken);
    return teams.getJoinedTeams(
      token,
      this.cache,
      this.cacheTtlMs,
      this.userGraphProxy,
    );
  }

  /** Check whether the token holder is a member of a specific channel. */
  async isChannelMember(
    team: TeamIdentifier,
    graphToken?: string,
  ): Promise<boolean> {
    const token = await this.resolveUserGraphToken(graphToken);
    return teams.isChannelMember(token, team, this.userGraphProxy);
  }

  /** Check whether the token holder is a member of a team (and optionally channel). */
  async isTeamMember(
    team: TeamIdentifier,
    graphToken?: string,
  ): Promise<boolean> {
    const token = await this.resolveUserGraphToken(graphToken);
    return teams.isTeamMember(
      token,
      team,
      this.cache,
      this.cacheTtlMs,
      this.userGraphProxy,
    );
  }

  /** Fetch channels for multiple teams using Graph batch API. */
  async getTeamsWithChannels(
    teamIds: string[],
    options?: { batchSize?: number },
    graphToken?: string,
  ): Promise<TeamChannels[]> {
    const token = await this.resolveUserGraphToken(graphToken);
    return teams.getTeamsWithChannels(
      token,
      teamIds,
      this.cache,
      this.cacheTtlMs,
      options?.batchSize,
      this.userGraphProxy,
    );
  }

  /** Pure check — does a team+channel pair exist in a pre-fetched list? */
  isTeamAndChannelMember(
    teamsWithChannels: TeamChannels[],
    teamsId: string,
    channelId: string,
  ): boolean {
    return teams.isTeamAndChannelMember(teamsWithChannels, teamsId, channelId);
  }

  // ── Drives ───────────────────────────────────────────────────────────

  /** Resolve the drive ID for a team or channel file store. */
  async getDriveId(team: TeamIdentifier, graphToken?: string): Promise<string> {
    const token = await this.resolveToken(graphToken);
    return drives.getDriveId(token, team, this.cache, this.cacheTtlMs);
  }

  /** Async generator yielding drive items page-by-page from a folder. */
  async *getDriveItemChildren(
    driveId: string,
    folderPath: string,
    options?: { pageSize?: number },
    graphToken?: string,
  ): AsyncGenerator<DriveItem> {
    const token = await this.resolveToken(graphToken);
    yield* drives.getDriveItemChildren(
      token,
      driveId,
      folderPath,
      options?.pageSize,
    );
  }

  /** Check whether a folder exists at the given drive path. */
  async checkFolderExists(
    driveId: string,
    folderPath: string,
    graphToken?: string,
  ): Promise<boolean> {
    const token = await this.resolveToken(graphToken);
    return drives.checkFolderExists(
      token,
      driveId,
      folderPath,
      this.cache,
      this.cacheTtlMs,
    );
  }

  // ── Files ────────────────────────────────────────────────────────────

  /** Recursively list all files under a drive folder. Returns `DriveItem[]`. */
  async listFiles(
    driveId: string,
    folderPath: string,
    options?: DriveItemFilterOptions,
    graphToken?: string,
  ): Promise<DriveItem[]> {
    const token = await this.resolveToken(graphToken);
    return files.listFiles(token, driveId, folderPath, options);
  }

  /** Generic file diff — caller supplies identity extractors and update predicate. */
  diffFiles<TNew, TExisting>(
    newFiles: TNew[],
    existingFiles: TExisting[],
    options: DiffOptions<TNew, TExisting>,
  ): FileDiff<TNew, TExisting> {
    return files.diffFiles(newFiles, existingFiles, options);
  }

  /** Download a file by its drive item ID. Returns the raw ArrayBuffer + size. */
  async downloadFile(
    driveId: string,
    itemId: string,
    graphToken?: string,
  ): Promise<DownloadResult> {
    const token = await this.resolveToken(graphToken);
    return files.downloadFile(token, driveId, itemId);
  }

  /**
   * High-level sync orchestrator: resolves drive, lists files, diffs against existing set.
   * Generic over the existing file type.
   */
  async getFilesForSync<TExisting>(
    team: TeamIdentifier,
    sharepointFolderURL: string,
    existingFiles: TExisting[],
    options: DriveItemFilterOptions & DiffOptions<DriveItem, TExisting>,
    graphToken?: string,
  ): Promise<FileSyncResult<TExisting>> {
    const token = await this.resolveToken(graphToken);
    const { excludedFolders, excludedFiles, excludedExtensions, ...diffOpts } =
      options;
    const filterOptions: DriveItemFilterOptions = {
      excludedFolders,
      excludedFiles,
      excludedExtensions,
    };
    return files.getFilesForSync(
      token,
      team,
      sharepointFolderURL,
      existingFiles,
      filterOptions,
      diffOpts,
      this.cache,
      this.cacheTtlMs,
    );
  }

  /** Check whether a SharePoint folder URL resolves to an existing folder. */
  async checkSharePointFolderExists(
    team: TeamIdentifier,
    sharepointFolderURL: string,
    graphToken?: string,
  ): Promise<boolean> {
    const token = await this.resolveToken(graphToken);
    return files.checkSharePointFolderExists(
      token,
      team,
      sharepointFolderURL,
      this.cache,
      this.cacheTtlMs,
    );
  }

  // ── Search ───────────────────────────────────────────────────────────

  /** Execute a SharePoint search query with full control over entity types. */
  async searchSharePoint(
    options: SharePointSearchOptions,
    graphToken?: string,
  ): Promise<unknown> {
    const token = await this.resolveToken(graphToken);
    return search.searchSharePoint(token, options, this.cache, this.cacheTtlMs);
  }

  /** Search SharePoint files (listItem entity type). */
  async searchFiles(query: string, graphToken?: string): Promise<unknown> {
    const token = await this.resolveToken(graphToken);
    return search.searchFiles(token, query, this.cache, this.cacheTtlMs);
  }

  /** Search SharePoint sites. */
  async searchSites(query: string, graphToken?: string): Promise<unknown> {
    const token = await this.resolveToken(graphToken);
    return search.searchSites(token, query, this.cache, this.cacheTtlMs);
  }

  /** Search SharePoint folders (list entity type). */
  async searchFolders(query: string, graphToken?: string): Promise<unknown> {
    const token = await this.resolveToken(graphToken);
    return search.searchFolders(token, query, this.cache, this.cacheTtlMs);
  }
}
