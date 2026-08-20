import { describe, expect, test, beforeEach, afterEach, mock } from "bun:test";
import { InMemoryCache } from "../../core/cache";
import {
  searchSharePoint,
  searchFiles,
  searchSites,
  searchFolders,
} from "../search";
import { SharePointSearchEntityType } from "../../types";

const TOKEN = "Bearer test-token";
const TTL = 60_000;

const SEARCH_RESULT = {
  value: [
    {
      searchTerms: ["test"],
      hitsContainers: [{ hits: [{ summary: "result 1" }] }],
    },
  ],
};

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

describe("searchSharePoint", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("sends correct request body", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchSharePoint(
      TOKEN,
      { query: "test query", entityTypes: ["listItem"], from: 0, size: 10 },
      cache,
      TTL,
    );

    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests).toHaveLength(1);
    expect(parsed.requests[0].entityTypes).toEqual(["listItem"]);
    expect(parsed.requests[0].query.queryString).toBe("test query");
    expect(parsed.requests[0].size).toBe(10);
  });

  test("uses default from=0 and size=5", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchSharePoint(
      TOKEN,
      { query: "test", entityTypes: ["site"] },
      cache,
      TTL,
    );

    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests[0].from).toBe(0);
    expect(parsed.requests[0].size).toBe(5);
  });

  test("returns and caches search results", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const opts = {
      query: "test",
      entityTypes: ["listItem"] as SharePointSearchEntityType[],
    };
    const r1 = await searchSharePoint(TOKEN, opts, cache, TTL);
    const r2 = await searchSharePoint(TOKEN, opts, cache, TTL);

    expect(r1).toEqual(SEARCH_RESULT);
    expect(r2).toEqual(SEARCH_RESULT);
    expect(callCount).toBe(1); // cached
  });

  test("different queries get different cache entries", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(JSON.stringify({ result: callCount }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const r1 = await searchSharePoint(
      TOKEN,
      { query: "alpha", entityTypes: ["listItem"] },
      cache,
      TTL,
    );
    const r2 = await searchSharePoint(
      TOKEN,
      { query: "beta", entityTypes: ["listItem"] },
      cache,
      TTL,
    );

    expect((r1 as { result: number }).result).toBe(1);
    expect((r2 as { result: number }).result).toBe(2);
    expect(callCount).toBe(2);
  });

  test("sends Authorization header", async () => {
    let capturedHeaders: HeadersInit | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedHeaders = init?.headers;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchSharePoint(
      TOKEN,
      { query: "q", entityTypes: ["site"] },
      cache,
      TTL,
    );

    expect((capturedHeaders as Record<string, string>).Authorization).toBe(
      TOKEN,
    );
  });

  test("supports multiple entity types", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchSharePoint(
      TOKEN,
      { query: "q", entityTypes: ["listItem", "site", "drive"] },
      cache,
      TTL,
    );

    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests[0].entityTypes).toEqual([
      "listItem",
      "site",
      "drive",
    ]);
  });
});

describe("searchFiles", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("uses listItem entity type", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchFiles(TOKEN, "report", cache, TTL);
    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests[0].entityTypes).toEqual(["listItem"]);
  });
});

describe("searchSites", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("uses site entity type", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchSites(TOKEN, "team site", cache, TTL);
    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests[0].entityTypes).toEqual(["site"]);
  });
});

describe("searchFolders", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("uses list entity type", async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = mock((_url: string, init?: RequestInit) => {
      capturedBody = init?.body as string;
      return Promise.resolve(
        new Response(JSON.stringify(SEARCH_RESULT), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await searchFolders(TOKEN, "docs", cache, TTL);
    const parsed = JSON.parse(capturedBody ?? "");
    expect(parsed.requests[0].entityTypes).toEqual(["list"]);
  });
});
