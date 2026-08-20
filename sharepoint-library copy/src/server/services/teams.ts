import { SharePointClientError } from "../core/errors";
import type {
  CacheAdapter,
  GraphBatchResponse,
  GraphChannelFilterResponse,
  GraphTeamsResponse,
  Team,
  TeamChannels,
  TeamIdentifier,
  UserGraphProxyConfig,
} from "../types";
import { fetchWithRetry, hashToken } from "../core/utils";
import { buildGraphRequest } from "../core/graph";

const DEFAULT_BATCH_SIZE = 20;

/** Fetch all teams the authenticated user has joined. */
export async function getJoinedTeams(
  graphToken: string | undefined,
  cache: CacheAdapter,
  cacheTtlMs: number,
  userGraphProxy?: UserGraphProxyConfig,
): Promise<Team[]> {
  const cacheKey = await getUserGraphCacheKey(
    "teams:joined",
    graphToken,
    userGraphProxy,
  );
  const cached = cacheKey ? await cache.get<Team[]>(cacheKey) : undefined;
  if (cached) return cached;

  const request = buildGraphRequest(
    "/me/joinedTeams",
    { token: graphToken, userGraphProxy },
    {},
    { requiresUserContext: true },
  );
  const response = await fetchWithRetry(request.url, request.init);

  const json = (await response.json()) as GraphTeamsResponse;
  if (!response.ok) {
    throw new SharePointClientError(
      "Failed to get joined teams",
      response.status,
      json,
    );
  }

  if (cacheKey) await cache.set(cacheKey, json.value, cacheTtlMs);
  return json.value;
}

/** Check whether the token holder is a member of a specific channel. */
export async function isChannelMember(
  graphToken: string | undefined,
  team: TeamIdentifier,
  userGraphProxy?: UserGraphProxyConfig,
): Promise<boolean> {
  const { teamsId, channelId } = team;
  if (!channelId) return true;

  const request = buildGraphRequest(
    `/teams/${teamsId}/channels?$filter=id eq '${channelId}'`,
    { token: graphToken, userGraphProxy },
    {},
    { requiresUserContext: true },
  );
  const response = await fetchWithRetry(request.url, request.init);

  if (!response.ok) {
    throw new SharePointClientError(
      "Failed to check channel membership",
      response.status,
    );
  }

  const data = (await response.json()) as GraphChannelFilterResponse;
  return data.value.length > 0;
}

/** Check whether the token holder is a member of a team (and optionally a channel). */
export async function isTeamMember(
  graphToken: string | undefined,
  team: TeamIdentifier,
  cache: CacheAdapter,
  cacheTtlMs: number,
  userGraphProxy?: UserGraphProxyConfig,
): Promise<boolean> {
  const teams = await getJoinedTeams(
    graphToken,
    cache,
    cacheTtlMs,
    userGraphProxy,
  );
  const isMember = teams.some((t) => t.id === team.teamsId);
  if (!isMember) return false;

  if (team.channelId) {
    return isChannelMember(graphToken, team, userGraphProxy);
  }
  return true;
}

/** Fetch channels for multiple teams using Graph batch API. */
export async function getTeamsWithChannels(
  graphToken: string | undefined,
  teamIds: string[],
  cache: CacheAdapter,
  cacheTtlMs: number,
  batchSize = DEFAULT_BATCH_SIZE,
  userGraphProxy?: UserGraphProxyConfig,
): Promise<TeamChannels[]> {
  if (teamIds.length === 0) return [];

  const sortedTeamIds = [...teamIds].sort();
  const cacheScope = await getUserGraphCacheKey(
    "teams:channels",
    graphToken,
    userGraphProxy,
  );
  const cacheKey = cacheScope
    ? `${cacheScope}:${sortedTeamIds.join(",")}`
    : null;
  const cached = cacheKey
    ? await cache.get<TeamChannels[]>(cacheKey)
    : undefined;
  if (cached) return cached;

  const results: TeamChannels[] = [];

  for (let i = 0; i < teamIds.length; i += batchSize) {
    const batch = teamIds.slice(i, i + batchSize);
    const batchBody = {
      requests: batch.map((id, idx) => ({
        id: String(idx),
        method: "GET",
        url: `/teams/${id}/channels?$select=id,webUrl`,
      })),
    };

    const request = buildGraphRequest(
      "/$batch",
      { token: graphToken, userGraphProxy },
      {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(batchBody),
      },
      { requiresUserContext: true },
    );
    const response = await fetchWithRetry(request.url, request.init);

    if (!response.ok) {
      throw new SharePointClientError(
        "Batch channel request failed",
        response.status,
      );
    }

    const json = (await response.json()) as GraphBatchResponse;

    for (const res of json.responses) {
      if (res.status === 200 && res.body) {
        const channels = res.body.value
          .filter((ch) => ch.webUrl)
          .map((ch) => encodeURIComponent(ch.id));
        results.push({ teamId: batch[parseInt(res.id)], channels });
      }
    }
  }

  if (cacheKey) await cache.set(cacheKey, results, cacheTtlMs);
  return results;
}

/** Pure function — checks if a team+channel pair exists in a pre-fetched list. */
export function isTeamAndChannelMember(
  teamsWithChannels: TeamChannels[],
  teamsId: string,
  channelId: string,
): boolean {
  const team = teamsWithChannels.find((t) => t.teamId === teamsId);
  return team ? team.channels.includes(channelId) : false;
}

async function getUserGraphCacheKey(
  keyPrefix: string,
  graphToken?: string,
  userGraphProxy?: UserGraphProxyConfig,
): Promise<string | null> {
  if (!userGraphProxy) {
    return graphToken ? `${keyPrefix}:${await hashToken(graphToken)}` : null;
  }

  const proxyCacheKey =
    userGraphProxy.cacheKey ??
    (userGraphProxy.userId && userGraphProxy.userId !== "me"
      ? `user:${userGraphProxy.userId}`
      : undefined);

  return proxyCacheKey ? `${keyPrefix}:proxy:${proxyCacheKey}` : null;
}
