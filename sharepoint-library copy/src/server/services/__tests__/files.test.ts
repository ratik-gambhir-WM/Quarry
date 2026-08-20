import { describe, expect, test, beforeEach, afterEach, mock } from "bun:test";
import { InMemoryCache } from "../../core/cache";
import {
  listFiles,
  diffFiles,
  downloadFile,
  checkSharePointFolderExists,
} from "../files";
import type { DriveItem, GraphDriveChildrenResponse } from "../../types";

const TOKEN = "Bearer test-token";
const TTL = 60_000;

function makeItem(
  id: string,
  name: string,
  opts?: { folder?: boolean; mimeType?: string; size?: number },
) {
  return {
    id,
    name,
    webUrl: `https://sp.com/${id}`,
    size: opts?.size ?? 100,
    lastModifiedDateTime: "2024-01-01T00:00:00Z",
    ...(opts?.folder
      ? { folder: {} }
      : { file: { mimeType: opts?.mimeType ?? "application/octet-stream" } }),
  };
}

function mockGraphChildren(
  pages: Array<{ items: ReturnType<typeof makeItem>[]; nextLink?: string }>,
): typeof fetch {
  let page = 0;
  return mock(() => {
    const current = pages[page] ?? { items: [] };
    page++;
    const body: GraphDriveChildrenResponse = {
      value: current.items,
      ...(current.nextLink ? { "@odata.nextLink": current.nextLink } : {}),
    };
    return Promise.resolve(
      new Response(JSON.stringify(body), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
  }) as unknown as typeof fetch;
}

describe("listFiles", () => {
  let originalFetch: typeof fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns flat list of files from a single page", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "report.pdf"), makeItem("f2", "data.csv")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toHaveLength(2);
    expect(files[0].id).toBe("f1");
    expect(files[1].id).toBe("f2");
  });

  test("follows pagination", async () => {
    globalThis.fetch = mockGraphChildren([
      {
        items: [makeItem("f1", "a.txt")],
        nextLink: "https://graph.microsoft.com/page2",
      },
      { items: [makeItem("f2", "b.txt")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toHaveLength(2);
  });

  test("recurses into subfolders", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      const body =
        fetchCount === 1
          ? {
              value: [
                makeItem("d1", "sub", { folder: true }),
                makeItem("f1", "top.txt"),
              ],
            }
          : { value: [makeItem("f2", "nested.txt")] };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toHaveLength(2);
    expect(files.map((f) => f.name).sort()).toEqual(["nested.txt", "top.txt"]);
  });

  test("excludes folders in excludedFolders list", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      const body =
        fetchCount === 1
          ? {
              value: [
                makeItem("d1", "archive", { folder: true }),
                makeItem("f1", "keep.txt"),
              ],
            }
          : { value: [makeItem("f2", "hidden.txt")] };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const files = await listFiles(TOKEN, "drv", "/root", {
      excludedFolders: ["archive"],
    });
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe("keep.txt");
    expect(fetchCount).toBe(1); // subfolder was NOT fetched
  });

  test("excludes files in excludedFiles list", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "secret.txt"), makeItem("f2", "ok.txt")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root", {
      excludedFiles: ["secret.txt"],
    });
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe("ok.txt");
  });

  test("excludes default extensions (.mp4, .zip)", async () => {
    globalThis.fetch = mockGraphChildren([
      {
        items: [
          makeItem("f1", "video.mp4"),
          makeItem("f2", "archive.zip"),
          makeItem("f3", "doc.pdf"),
        ],
      },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe("doc.pdf");
  });

  test("allows overriding excluded extensions", async () => {
    globalThis.fetch = mockGraphChildren([
      {
        items: [
          makeItem("f1", "video.mp4"),
          makeItem("f2", "archive.zip"),
          makeItem("f3", "doc.pdf"),
        ],
      },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root", {
      excludedExtensions: [".pdf"], // only exclude PDF now
    });
    expect(files).toHaveLength(2);
    expect(files.map((f) => f.name).sort()).toEqual([
      "archive.zip",
      "video.mp4",
    ]);
  });

  test("disabling extension filter with empty array keeps all files", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "video.mp4"), makeItem("f2", "doc.pdf")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root", {
      excludedExtensions: [],
    });
    expect(files).toHaveLength(2);
  });

  test("extension filtering is case-insensitive", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "VIDEO.MP4"), makeItem("f2", "doc.pdf")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toHaveLength(1);
    expect(files[0].name).toBe("doc.pdf");
  });

  test("returns empty array for empty folder", async () => {
    globalThis.fetch = mockGraphChildren([{ items: [] }]);
    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files).toEqual([]);
  });

  test("computes relativePath correctly (folder portion only)", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "file.txt")] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    // File is at /root/file.txt, relative to /root → file portion is "file.txt"
    // relativePath is the folder portion only = ""
    expect(files[0].relativePath).toBe("");
  });

  test("relativePath for nested file includes parent folders", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      const body =
        fetchCount === 1
          ? { value: [makeItem("d1", "sub", { folder: true })] }
          : { value: [makeItem("f1", "deep.txt")] };
      return Promise.resolve(
        new Response(JSON.stringify(body), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }) as unknown as typeof fetch;

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files[0].relativePath).toBe("sub");
  });

  test("sets mimeType from Graph API response", async () => {
    globalThis.fetch = mockGraphChildren([
      { items: [makeItem("f1", "doc.pdf", { mimeType: "application/pdf" })] },
    ]);

    const files = await listFiles(TOKEN, "drv", "/root");
    expect(files[0].mimeType).toBe("application/pdf");
  });
});

describe("diffFiles", () => {
  const opts = {
    getNewId: (f: { id: string }) => f.id,
    getExistingId: (f: { exId: string }) => f.exId,
    shouldUpdate: (
      n: { id: string; mod: string },
      e: { exId: string; sync: string },
    ) => n.mod !== e.sync,
  };

  test("all new → all added", () => {
    const result = diffFiles([{ id: "1", mod: "a" }], [], opts);
    expect(result.added).toHaveLength(1);
    expect(result.removed).toHaveLength(0);
    expect(result.modified).toHaveLength(0);
  });

  test("all existing removed when new is empty", () => {
    const result = diffFiles([], [{ exId: "1", sync: "a" }], opts);
    expect(result.removed).toHaveLength(1);
  });

  test("modified when shouldUpdate returns true", () => {
    const result = diffFiles(
      [{ id: "1", mod: "b" }],
      [{ exId: "1", sync: "a" }],
      opts,
    );
    expect(result.modified).toHaveLength(1);
    expect(result.added).toHaveLength(0);
    expect(result.removed).toHaveLength(0);
  });

  test("not modified when values match", () => {
    const result = diffFiles(
      [{ id: "1", mod: "a" }],
      [{ exId: "1", sync: "a" }],
      opts,
    );
    expect(result.modified).toHaveLength(0);
  });

  test("no modification detection without shouldUpdate", () => {
    const result = diffFiles(
      [{ id: "1", mod: "b" }],
      [{ exId: "1", sync: "a" }],
      { getNewId: (f) => f.id, getExistingId: (f) => f.exId },
    );
    expect(result.modified).toHaveLength(0);
  });
});

describe("downloadFile", () => {
  let originalFetch: typeof fetch;

  beforeEach(() => {
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("downloads file and returns buffer + size", async () => {
    const content = new TextEncoder().encode("hello world");
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      if (fetchCount === 1) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              "@microsoft.graph.downloadUrl":
                "https://download.example.com/file",
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      return Promise.resolve(new Response(content, { status: 200 }));
    }) as unknown as typeof fetch;

    const result = await downloadFile(TOKEN, "drv", "item1");
    expect(result.size).toBe(11);
    expect(new TextDecoder().decode(result.buffer)).toBe("hello world");
  });

  test("throws when download URL is missing", async () => {
    globalThis.fetch = mock(() =>
      Promise.resolve(
        new Response(JSON.stringify({}), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      ),
    ) as unknown as typeof fetch;

    await expect(downloadFile(TOKEN, "drv", "item1")).rejects.toThrow(
      "Missing download URL",
    );
  });

  test("makes two fetch calls: metadata then download", async () => {
    let urls: string[] = [];
    globalThis.fetch = mock((url: string) => {
      urls.push(url);
      if (urls.length === 1) {
        return Promise.resolve(
          new Response(
            JSON.stringify({
              "@microsoft.graph.downloadUrl": "https://dl.example.com/f",
            }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      return Promise.resolve(new Response("data", { status: 200 }));
    }) as unknown as typeof fetch;

    await downloadFile(TOKEN, "drv", "item1");
    expect(urls).toHaveLength(2);
    expect(urls[0]).toContain("drives/drv/items/item1");
    expect(urls[1]).toBe("https://dl.example.com/f");
  });
});

describe("checkSharePointFolderExists", () => {
  let cache: InMemoryCache;
  let originalFetch: typeof fetch;

  beforeEach(() => {
    cache = new InMemoryCache();
    originalFetch = globalThis.fetch;
  });
  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  test("returns true when folder exists", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      if (fetchCount === 1) {
        // getDriveId
        return Promise.resolve(
          new Response(
            JSON.stringify({ parentReference: { driveId: "drv" } }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      // checkFolderExists
      return Promise.resolve(new Response("{}", { status: 200 }));
    }) as unknown as typeof fetch;

    const result = await checkSharePointFolderExists(
      TOKEN,
      { teamsId: "t1" },
      "https://company.sharepoint.com/sites/team/Shared%20Documents/Reports",
      cache,
      TTL,
    );
    expect(result).toBe(true);
  });

  test("returns false when folder does not exist", async () => {
    let fetchCount = 0;
    globalThis.fetch = mock(() => {
      fetchCount++;
      if (fetchCount === 1) {
        return Promise.resolve(
          new Response(
            JSON.stringify({ parentReference: { driveId: "drv" } }),
            { status: 200, headers: { "Content-Type": "application/json" } },
          ),
        );
      }
      return Promise.resolve(new Response("{}", { status: 404 }));
    }) as unknown as typeof fetch;

    const result = await checkSharePointFolderExists(
      TOKEN,
      { teamsId: "t1" },
      "https://company.sharepoint.com/sites/team/Shared%20Documents/Missing",
      cache,
      TTL,
    );
    expect(result).toBe(false);
  });
});
