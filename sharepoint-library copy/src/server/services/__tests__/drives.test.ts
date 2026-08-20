import { describe, expect, test, beforeEach, afterEach, mock } from "bun:test";
import { InMemoryCache } from "../../core/cache";
import { SharePointClientError } from "../../core/errors";
import { getDriveId, getDriveItemChildren, checkFolderExists } from "../drives";

const TOKEN = "Bearer test-token";
const TTL = 60_000;

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

describe("getDriveId", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("resolves drive ID for a team without channel", async () => {
    globalThis.fetch = mockFetchResponse({
      parentReference: { driveId: "drv-123" },
    });

    const id = await getDriveId(TOKEN, { teamsId: "t1" }, cache, TTL);
    expect(id).toBe("drv-123");
  });

  test("resolves drive ID for a team with channel", async () => {
    globalThis.fetch = mockFetchResponse({
      parentReference: { driveId: "drv-456" },
    });

    const id = await getDriveId(
      TOKEN,
      { teamsId: "t1", channelId: "ch1" },
      cache,
      TTL,
    );
    expect(id).toBe("drv-456");
  });

  test("uses correct URL for team vs channel", async () => {
    let requestedUrl = "";
    globalThis.fetch = mock((url: string) => {
      requestedUrl = url;
      return Promise.resolve(
        new Response(JSON.stringify({ parentReference: { driveId: "d" } }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    await getDriveId(TOKEN, { teamsId: "t1" }, cache, TTL);
    expect(requestedUrl).toContain("/teams/t1/filesFolder");
    expect(requestedUrl).not.toContain("/channels/");

    await cache.clear();
    await getDriveId(TOKEN, { teamsId: "t1", channelId: "ch1" }, cache, TTL);
    expect(requestedUrl).toContain("/teams/t1/channels/ch1/filesFolder");
  });

  test("caches drive ID and returns from cache on second call", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ parentReference: { driveId: "drv-1" } }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    await getDriveId(TOKEN, { teamsId: "t1" }, cache, TTL);
    await getDriveId(TOKEN, { teamsId: "t1" }, cache, TTL);
    expect(callCount).toBe(1);
  });

  test("different teams get different cache entries", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(
        new Response(
          JSON.stringify({ parentReference: { driveId: `drv-${callCount}` } }),
          { status: 200, headers: { "Content-Type": "application/json" } },
        ),
      );
    }) as unknown as typeof fetch;

    const d1 = await getDriveId(TOKEN, { teamsId: "t1" }, cache, TTL);
    const d2 = await getDriveId(TOKEN, { teamsId: "t2" }, cache, TTL);
    expect(d1).toBe("drv-1");
    expect(d2).toBe("drv-2");
    expect(callCount).toBe(2);
  });
});

describe("getDriveItemChildren", () => {
  let originalFetch: typeof fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("yields file items from a single page", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [
        {
          id: "f1",
          name: "report.PDF",
          webUrl: "https://sp.com/f1",
          size: 100,
          lastModifiedDateTime: "2024-01-01T00:00:00Z",
          file: { mimeType: "application/pdf" },
        },
      ],
    });

    const items: unknown[] = [];
    for await (const item of getDriveItemChildren(TOKEN, "drv", "/folder")) {
      items.push(item);
    }
    expect(items).toHaveLength(1);
    expect((items[0] as { name: string }).name).toBe("report.pdf"); // extension lowercased
  });

  test("skips folder items", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [
        {
          id: "d1",
          name: "subfolder",
          folder: {},
          size: 0,
          lastModifiedDateTime: "2024-01-01",
          webUrl: "u",
        },
        {
          id: "f1",
          name: "file.txt",
          size: 50,
          lastModifiedDateTime: "2024-01-01",
          webUrl: "u",
          file: { mimeType: "text/plain" },
        },
      ],
    });

    const items: unknown[] = [];
    for await (const item of getDriveItemChildren(TOKEN, "drv", "/folder")) {
      items.push(item);
    }
    expect(items).toHaveLength(1);
    expect((items[0] as { id: string }).id).toBe("f1");
  });

  test("follows pagination via @odata.nextLink", async () => {
    let page = 0;
    globalThis.fetch = mock(() => {
      page++;
      const body =
        page === 1
          ? {
              value: [
                {
                  id: "f1",
                  name: "a.txt",
                  size: 10,
                  lastModifiedDateTime: "2024-01-01",
                  webUrl: "u",
                  file: { mimeType: "text/plain" },
                },
              ],
              "@odata.nextLink": "https://graph.microsoft.com/page2",
            }
          : {
              value: [
                {
                  id: "f2",
                  name: "b.txt",
                  size: 20,
                  lastModifiedDateTime: "2024-01-01",
                  webUrl: "u",
                  file: { mimeType: "text/plain" },
                },
              ],
            };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const items: unknown[] = [];
    for await (const item of getDriveItemChildren(TOKEN, "drv", "/folder")) {
      items.push(item);
    }
    expect(items).toHaveLength(2);
    expect(page).toBe(2);
  });

  test("yields nothing for empty folder", async () => {
    globalThis.fetch = mockFetchResponse({ value: [] });

    const items: unknown[] = [];
    for await (const item of getDriveItemChildren(TOKEN, "drv", "/empty")) {
      items.push(item);
    }
    expect(items).toHaveLength(0);
  });

  test("sets empty relativePath (caller responsibility)", async () => {
    globalThis.fetch = mockFetchResponse({
      value: [
        {
          id: "f1",
          name: "file.txt",
          size: 1,
          lastModifiedDateTime: "2024-01-01",
          webUrl: "u",
          file: { mimeType: "text/plain" },
        },
      ],
    });

    for await (const item of getDriveItemChildren(TOKEN, "drv", "/folder")) {
      expect(item.relativePath).toBe("");
    }
  });
});

describe("checkFolderExists", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns true when folder exists (200)", async () => {
    globalThis.fetch = mock(() =>
      Promise.resolve(new Response("{}", { status: 200 })),
    ) as unknown as typeof fetch;

    const result = await checkFolderExists(TOKEN, "drv", "/exists", cache, TTL);
    expect(result).toBe(true);
  });

  test("returns false when folder not found (404)", async () => {
    globalThis.fetch = mock(() =>
      Promise.resolve(new Response("{}", { status: 404 })),
    ) as unknown as typeof fetch;

    const result = await checkFolderExists(
      TOKEN,
      "drv",
      "/missing",
      cache,
      TTL,
    );
    expect(result).toBe(false);
  });

  test("throws on other HTTP errors", async () => {
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: { message: "forbidden" } }), {
          status: 403,
        }),
      ),
    ) as unknown as typeof fetch;

    await expect(
      checkFolderExists(TOKEN, "drv", "/forbidden", cache, TTL),
    ).rejects.toThrow(SharePointClientError);
  });

  test("caches true result", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;

    await checkFolderExists(TOKEN, "drv", "/exists", cache, TTL);
    await checkFolderExists(TOKEN, "drv", "/exists", cache, TTL);
    expect(callCount).toBe(1);
  });

  test("caches false result (404)", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(new Response("{}", { status: 404 }));
    }) as unknown as typeof fetch;

    await checkFolderExists(TOKEN, "drv", "/gone", cache, TTL);
    const result = await checkFolderExists(TOKEN, "drv", "/gone", cache, TTL);
    expect(result).toBe(false);
    expect(callCount).toBe(1);
  });

  test("uses different cache keys for different paths", async () => {
    let callCount = 0;
    globalThis.fetch = mock(() => {
      callCount++;
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;

    await checkFolderExists(TOKEN, "drv", "/a", cache, TTL);
    await checkFolderExists(TOKEN, "drv", "/b", cache, TTL);
    expect(callCount).toBe(2);
  });
});
