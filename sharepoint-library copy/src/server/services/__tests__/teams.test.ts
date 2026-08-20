import { describe, expect, test, beforeEach, afterEach, mock } from "bun:test";
import { InMemoryCache } from "../../core/cache";
import {
  getJoinedTeams,
  isChannelMember,
  isTeamMember,
  getTeamsWithChannels,
  isTeamAndChannelMember,
} from "../teams";
import type { TeamChannels } from "../../types";

const TOKEN = "Bearer test-token-12345678";
const TTL = 60_000;
const PROXY = { baseUrl: "https://westmonroe-cloud.com" };

function mockFetchResponse(body: unknown, status = 200): typeof fetch {
  return mock(() =>
    Promise.resolve(
      new Response(JSON.stringify(body), {
        status,
        headers: { "Content-Type": "application/json" },
      }),
    ),
  ) as unknown as typeof fetch;
}

describe("getJoinedTeams", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns teams from Graph API", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [
        { id: "t1", displayName: "Team One" },
        { id: "t2", displayName: "Team Two" },
      ],
    });

    const teams = await getJoinedTeams(TOKEN, cache, TTL);
    expect(teams).toHaveLength(2);
    expect(teams[0].id).toBe("t1");
    expect(teams[1].displayName).toBe("Team Two");
  });

  test("uses user graph proxy without Authorization when configured", async () => {
    let requestedUrl = "";
    let capturedHeaders: HeadersInit | undefined;
    globalThis.fetch = mock((url: string, init?: RequestInit) => {
      requestedUrl = url;
      capturedHeaders = init?.headers;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "t1", displayName: "Team One" }] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const teams = await getJoinedTeams(undefined, cache, TTL, PROXY);

    expect(teams[0].id).toBe("t1");
    expect(requestedUrl).toBe(
      "https://westmonroe-cloud.com/users/me/graph/joinedTeams",
    );
    expect(
      (capturedHeaders as Record<string, string>).Authorization,
    ).toBeUndefined();
  });

  test("does not cache proxy /me responses without a stable cache key", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            value: [{ id: `t${callCount}`, displayName: "T" }],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getJoinedTeams(undefined, cache, TTL, PROXY);
    await getJoinedTeams(undefined, cache, TTL, PROXY);

    expect(callCount).toBe(2);
  });

  test("caches proxy responses when a stable cache key is configured", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "t1", displayName: "T" }] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getJoinedTeams(undefined, cache, TTL, {
      ...PROXY,
      cacheKey: "session-1",
    });
    await getJoinedTeams(undefined, cache, TTL, {
      ...PROXY,
      cacheKey: "session-1",
    });

    expect(callCount).toBe(1);
  });

  test("returns cached result on second call", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "t1", displayName: "T" }] }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );
    }) as unknown as typeof fetch;

    await getJoinedTeams(TOKEN, cache, TTL);
    await getJoinedTeams(TOKEN, cache, TTL);
    expect(callCount).toBe(1);
  });

  test("scopes cache per full token (different callers do not share cache)", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [{ id: "t1", displayName: "T" }],
    });

    const otherToken = "Bearer different-token-XXXXXXXX";
    await getJoinedTeams(TOKEN, cache, TTL);

    // Different token should trigger a new fetch
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "t2", displayName: "T2" }] }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      );
    }) as unknown as typeof fetch;

    const teams = await getJoinedTeams(otherToken, cache, TTL);
    expect(callCount).toBe(1);
    expect(teams[0].id).toBe("t2");
  });

  test("does not leak cache between tokens that share the same suffix", async () => {
    // Two tokens that only differ in their prefix — the old last-8-chars
    // scheme would have incorrectly returned caller A's teams to caller B.
    const tokenA = "Bearer aaaaaaaa-shared-suffix";
    const tokenB = "Bearer bbbbbbbb-shared-suffix";

    globalThis.fetch = mockFetchResponse({
      value: [{ id: "team-of-A", displayName: "A" }],
    });
    const teamsA = await getJoinedTeams(tokenA, cache, TTL);
    expect(teamsA[0].id).toBe("team-of-A");

    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ value: [{ id: "team-of-B", displayName: "B" }] }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const teamsB = await getJoinedTeams(tokenB, cache, TTL);
    expect(callCount).toBe(1);
    expect(teamsB[0].id).toBe("team-of-B");
  });

  test("returns empty array when user has no teams", async () => {
    globalThis.fetch = mockFetchResponse({ value: [] });
    const teams = await getJoinedTeams(TOKEN, cache, TTL);
    expect(teams).toEqual([]);
  });
});

describe("isChannelMember", () => {
  let originalFetch: typeof fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns true when no channelId provided", async () => {
    const result = await isChannelMember(TOKEN, { teamsId: "t1" });
    expect(result).toBe(true);
  });

  test("returns true when channel is found", async () => {
    globalThis.fetch = mockFetchResponse({ value: [{ id: "ch1" }] });
    const result = await isChannelMember(TOKEN, {
      teamsId: "t1",
      channelId: "ch1",
    });
    expect(result).toBe(true);
  });

  test("uses user graph proxy for channel membership checks", async () => {
    let requestedUrl = "";
    let capturedHeaders: HeadersInit | undefined;
    globalThis.fetch = mock((url: string, init?: RequestInit) => {
      requestedUrl = url;
      capturedHeaders = init?.headers;
      return Promise.resolve(
        new Response(JSON.stringify({ value: [{ id: "ch1" }] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const result = await isChannelMember(
      undefined,
      { teamsId: "t1", channelId: "ch1" },
      PROXY,
    );

    expect(result).toBe(true);
    expect(requestedUrl).toBe(
      "https://westmonroe-cloud.com/users/me/graph/teams/t1/channels?$filter=id eq 'ch1'",
    );
    expect(
      (capturedHeaders as Record<string, string>).Authorization,
    ).toBeUndefined();
  });

  test("returns false when channel is not found", async () => {
    globalThis.fetch = mockFetchResponse({ value: [] });
    const result = await isChannelMember(TOKEN, {
      teamsId: "t1",
      channelId: "ch-nonexistent",
    });
    expect(result).toBe(false);
  });

  test("throws when fetch throws (network error)", async () => {
    globalThis.fetch = mock(() =>
      Promise.reject(new Error("network error")),
    ) as unknown as typeof fetch;
    await expect(
      isChannelMember(TOKEN, { teamsId: "t1", channelId: "ch1" }),
    ).rejects.toThrow("network error");
  });
});

describe("isTeamMember", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns true when user is member of team (no channel)", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [{ id: "t1", displayName: "T" }],
    });
    const result = await isTeamMember(TOKEN, { teamsId: "t1" }, cache, TTL);
    expect(result).toBe(true);
  });

  test("returns false when user is not member of team", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [{ id: "t1", displayName: "T" }],
    });
    const result = await isTeamMember(
      TOKEN,
      { teamsId: "t-other" },
      cache,
      TTL,
    );
    expect(result).toBe(false);
  });

  test("checks channel membership when channelId is provided and team matches", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      if (fetchCount === 1) {
        // getJoinedTeams
        return Promise.resolve(
          new Response(
            JSON.stringify({ value: [{ id: "t1", displayName: "T" }] }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      // isChannelMember
      return Promise.resolve(
        new Response(JSON.stringify({ value: [{ id: "ch1" }] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const result = await isTeamMember(
      TOKEN,
      { teamsId: "t1", channelId: "ch1" },
      cache,
      TTL,
    );
    expect(result).toBe(true);
    expect(fetchCount).toBe(2);
  });

  test("skips channel check when user is not in team", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      return Promise.resolve(
        new Response(JSON.stringify({ value: [] }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const result = await isTeamMember(
      TOKEN,
      { teamsId: "t1", channelId: "ch1" },
      cache,
      TTL,
    );
    expect(result).toBe(false);
    // Only 1 call — getJoinedTeams, no isChannelMember
    expect(fetchCount).toBe(1);
  });
});

describe("getTeamsWithChannels", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns channels for teams via batch API", async () => {
    globalThis.fetch = mockFetchResponse({
      responses: [
        {
          status: 200,
          id: "0",
          body: {
            value: [
              { id: "ch1", webUrl: "https://example.com/ch1" },
              { id: "ch2", webUrl: "https://example.com/ch2" },
            ],
          },
        },
      ],
    });

    const result = await getTeamsWithChannels(TOKEN, ["t1"], cache, TTL);
    expect(result).toHaveLength(1);
    expect(result[0].teamId).toBe("t1");
    expect(result[0].channels).toHaveLength(2);
  });

  test("uses user graph proxy for user-scoped batch channel requests", async () => {
    let requestedUrl = "";
    let capturedHeaders: HeadersInit | undefined;
    let capturedBody = "";
    globalThis.fetch = mock((url: string, init?: RequestInit) => {
      requestedUrl = url;
      capturedHeaders = init?.headers;
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            responses: [
              {
                status: 200,
                id: "0",
                body: { value: [{ id: "ch1", webUrl: "https://example.com" }] },
              },
            ],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const result = await getTeamsWithChannels(
      undefined,
      ["t1"],
      cache,
      TTL,
      undefined,
      PROXY,
    );

    expect(result[0].channels).toEqual(["ch1"]);
    expect(requestedUrl).toBe(
      "https://westmonroe-cloud.com/users/me/graph/$batch",
    );
    expect(
      (capturedHeaders as Record<string, string>).Authorization,
    ).toBeUndefined();
    expect(JSON.parse(capturedBody).requests[0].url).toBe(
      "/teams/t1/channels?$select=id,webUrl",
    );
  });

  test("filters out channels without webUrl", async () => {
    globalThis.fetch = mockFetchResponse({
      responses: [
        {
          status: 200,
          id: "0",
          body: {
            value: [
              { id: "ch1", webUrl: "https://example.com/ch1" },
              { id: "ch2" }, // no webUrl
            ],
          },
        },
      ],
    });

    const result = await getTeamsWithChannels(TOKEN, ["t1"], cache, TTL);
    expect(result[0].channels).toHaveLength(1);
  });

  test("skips failed batch responses", async () => {
    globalThis.fetch = mockFetchResponse({
      responses: [
        { status: 200, id: "0", body: { value: [{ id: "ch1", webUrl: "u" }] } },
        { status: 403, id: "1" }, // failed — no body
      ],
    });

    const result = await getTeamsWithChannels(TOKEN, ["t1", "t2"], cache, TTL);
    expect(result).toHaveLength(1);
    expect(result[0].teamId).toBe("t1");
  });

  test("returns cached result on second call", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            responses: [{ status: 200, id: "0", body: { value: [] } }],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getTeamsWithChannels(TOKEN, ["t1"], cache, TTL);
    await getTeamsWithChannels(TOKEN, ["t1"], cache, TTL);
    expect(callCount).toBe(1);
  });

  test("scopes channel cache per token (different callers do not share cache)", async () => {
    // Caller A warms the cache with their channels.
    globalThis.fetch = mockFetchResponse({
      responses: [
        {
          status: 200,
          id: "0",
          body: {
            value: [{ id: "ch-of-A", webUrl: "https://example.com/a" }],
          },
        },
      ],
    });
    const resultA = await getTeamsWithChannels(TOKEN, ["t1"], cache, TTL);
    expect(resultA[0].channels[0]).toBe("ch-of-A");

    // A second caller with a different token must not see A's cached data.
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            responses: [
              {
                status: 200,
                id: "0",
                body: {
                  value: [{ id: "ch-of-B", webUrl: "https://example.com/b" }],
                },
              },
            ],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const otherToken = "Bearer different-token";
    const resultB = await getTeamsWithChannels(otherToken, ["t1"], cache, TTL);
    expect(callCount).toBe(1);
    expect(resultB[0].channels[0]).toBe("ch-of-B");
  });

  test("cache key is stable regardless of teamIds order", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            responses: [
              { status: 200, id: "0", body: { value: [] } },
              { status: 200, id: "1", body: { value: [] } },
            ],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getTeamsWithChannels(TOKEN, ["t1", "t2"], cache, TTL);
    await getTeamsWithChannels(TOKEN, ["t2", "t1"], cache, TTL);
    expect(callCount).toBe(1);
  });

  test("batches requests when teamIds exceed batchSize", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({
            responses: [{ status: 200, id: "0", body: { value: [] } }],
          }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getTeamsWithChannels(TOKEN, ["t1", "t2", "t3"], cache, TTL, 2);
    expect(callCount).toBe(2); // 2 batches: [t1,t2] and [t3]
  });

  test("handles empty teamIds array", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(new Response("", { status: 200 }));
    }) as unknown as typeof fetch;

    const result = await getTeamsWithChannels(TOKEN, [], cache, TTL);
    expect(result).toEqual([]);
    expect(callCount).toBe(0); // no fetch needed
  });

  test("throws when batch fetch throws (network error)", async () => {
    globalThis.fetch = mock(() =>
      Promise.reject(new Error("batch network error")),
    ) as unknown as typeof fetch;
    await expect(
      getTeamsWithChannels(TOKEN, ["t1"], cache, TTL),
    ).rejects.toThrow("batch network error");
  });
});

describe("isTeamAndChannelMember", () => {
  const data: TeamChannels[] = [
    { teamId: "t1", channels: ["ch1", "ch2"] },
    { teamId: "t2", channels: ["ch3"] },
  ];

  test("returns true when team and channel match", () => {
    expect(isTeamAndChannelMember(data, "t1", "ch2")).toBe(true);
  });

  test("returns false when team matches but channel does not", () => {
    expect(isTeamAndChannelMember(data, "t1", "ch99")).toBe(false);
  });

  test("returns false when team does not match", () => {
    expect(isTeamAndChannelMember(data, "t99", "ch1")).toBe(false);
  });

  test("returns false for empty list", () => {
    expect(isTeamAndChannelMember([], "t1", "ch1")).toBe(false);
  });

  test("channel ID matching is exact (no partial match)", () => {
    expect(isTeamAndChannelMember(data, "t1", "ch")).toBe(false);
  });
});
