import { describe, expect, test } from "bun:test";
import {
  fetchWithRetry,
  parseSharePointFolderPath,
  normalizePath,
  isPathExcluded,
  normalizeFileExtension,
  buildDriveChildrenUrl,
} from "../utils";
import { SharePointClientError } from "../errors";

describe("parseSharePointFolderPath", () => {
  test("extracts path after Shared Documents", () => {
    const url =
      "https://company.sharepoint.com/sites/team/Shared%20Documents/Project/Subfolder";
    expect(parseSharePointFolderPath(url)).toBe("/Project/Subfolder");
  });

  test("handles /:f:/r/ prefix and query params", () => {
    const url =
      "https://company.sharepoint.com/:f:/r/sites/team/Shared%20Documents/Reports?csf=1&web=1";
    expect(parseSharePointFolderPath(url)).toBe("/Reports");
  });

  test("handles channel folder structure with @thread.tacv2 (no Shared Documents)", () => {
    const url =
      "https://company.sharepoint.com/sites/team/General/@thread.tacv2/Files";
    expect(parseSharePointFolderPath(url)).toBe("/Files");
  });

  test("prefers Shared Documents marker when both are present", () => {
    const url =
      "https://company.sharepoint.com/sites/team/Shared%20Documents/General/@thread.tacv2/Files";
    expect(parseSharePointFolderPath(url)).toBe("/General/@thread.tacv2/Files");
  });

  test("returns full path when no known marker is found", () => {
    const url = "https://company.sharepoint.com/sites/team/CustomLib/Docs";
    expect(parseSharePointFolderPath(url)).toBe("/sites/team/CustomLib/Docs");
  });

  test("handles encoded spaces in URL", () => {
    const url =
      "https://company.sharepoint.com/sites/team/Shared%20Documents/My%20Folder/Sub%20Folder";
    expect(parseSharePointFolderPath(url)).toBe("/My Folder/Sub Folder");
  });
});

describe("normalizePath", () => {
  test("trims leading and trailing slashes", () => {
    expect(normalizePath("/foo/bar/")).toBe("foo/bar");
  });

  test("lowercases the path", () => {
    expect(normalizePath("FOO/BAR")).toBe("foo/bar");
  });

  test("handles empty string", () => {
    expect(normalizePath("")).toBe("");
  });

  test("handles multiple leading slashes", () => {
    expect(normalizePath("///foo///")).toBe("foo");
  });
});

describe("isPathExcluded", () => {
  test("returns false when no exclusions provided", () => {
    expect(isPathExcluded("some/path")).toBe(false);
  });

  test("returns false for undefined exclusions", () => {
    expect(isPathExcluded("some/path", undefined)).toBe(false);
  });

  test("returns false when path not in exclusions", () => {
    expect(isPathExcluded("some/path", ["other/path"])).toBe(false);
  });

  test("returns true when path matches an exclusion", () => {
    expect(isPathExcluded("some/path", ["some/path"])).toBe(true);
  });

  test("matching is case-insensitive", () => {
    expect(isPathExcluded("some/path", ["SOME/PATH"])).toBe(true);
  });

  test("normalizes exclusion entries (trims slashes)", () => {
    expect(isPathExcluded("folder", ["/folder/"])).toBe(true);
  });

  test("returns true if any exclusion matches", () => {
    expect(isPathExcluded("target", ["alpha", "target", "beta"])).toBe(true);
  });

  test("returns false for empty exclusion list", () => {
    expect(isPathExcluded("some/path", [])).toBe(false);
  });
});

describe("normalizeFileExtension", () => {
  test("lowercases the extension", () => {
    expect(normalizeFileExtension("Report.PDF")).toBe("Report.pdf");
  });

  test("handles already-lowercase extensions", () => {
    expect(normalizeFileExtension("file.txt")).toBe("file.txt");
  });

  test("returns filename unchanged when no extension", () => {
    expect(normalizeFileExtension("README")).toBe("README");
  });

  test("handles multiple dots (only lowercases last extension)", () => {
    expect(normalizeFileExtension("archive.tar.GZ")).toBe("archive.tar.gz");
  });

  test("handles empty string", () => {
    expect(normalizeFileExtension("")).toBe("");
  });

  test("handles dotfile", () => {
    expect(normalizeFileExtension(".gitignore")).toBe(".gitignore");
  });
});

describe("buildDriveChildrenUrl", () => {
  test("builds URL with default page size", () => {
    const url = buildDriveChildrenUrl("drive123", "/folder");
    expect(url).toContain("drives/drive123/root:");
    expect(url).toContain("$top=500");
  });

  test("builds URL with custom page size", () => {
    const url = buildDriveChildrenUrl("drive123", "/folder", 100);
    expect(url).toContain("$top=100");
  });

  test("encodes the folder path", () => {
    const url = buildDriveChildrenUrl("d1", "/My Folder/Sub");
    expect(url).toContain(encodeURIComponent("/My Folder/Sub"));
  });
});

describe("fetchWithRetry", () => {
  const originalFetch = globalThis.fetch;

  test("does not retry permanent Graph authorization failures", async () => {
    let requestCount = 0;
    globalThis.fetch = (async () => {
      requestCount += 1;
      return new Response(JSON.stringify({ error: { code: "accessDenied", message: "Denied" } }), { status: 403 });
    }) as typeof fetch;

    try {
      await fetchWithRetry("https://graph.microsoft.com/v1.0/test", {});
      throw new Error("Expected fetchWithRetry to throw");
    } catch (error) {
      expect(error).toBeInstanceOf(SharePointClientError);
      expect((error as Error).message).toContain("HTTP 403: not retryable");
      expect((error as Error).message).toContain("accessDenied");
    } finally {
      globalThis.fetch = originalFetch;
    }

    expect(requestCount).toBe(1);
  });
});
