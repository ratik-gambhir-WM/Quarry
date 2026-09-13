import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { inspectBundle } from "../../scripts/check-bundle-size.mjs";

const temporaryDirectories = [];

afterEach(async () => {
  const { rm } = await import("node:fs/promises");
  await Promise.all(temporaryDirectories.splice(0).map((directory) => rm(directory, { force: true, recursive: true })));
});

describe("bundle size inspection", () => {
  it("accepts hashed application chunks and excludes worker assets not represented as JavaScript chunks", async () => {
    const distDirectory = await makeBundle({
      "assets/feature-HASH.js": { contents: "dynamic", manifest: { isDynamicEntry: true } },
      "assets/index-HASH.js": { contents: "entry", manifest: { isEntry: true } },
      "assets/pdf.worker-HASH.mjs": { contents: "x".repeat(600_000), manifest: {} },
    });

    const result = await inspectBundle({ distDirectory, entryBudget: 100, target: "web" });

    expect(result.failures).toEqual([]);
    expect(result.chunks.map((chunk) => chunk.file)).not.toContain("assets/pdf.worker-HASH.mjs");
  });

  it("reports entry and dynamic chunk budget failures independently", async () => {
    const distDirectory = await makeBundle({
      "assets/feature-HASH.js": { contents: "x".repeat(500_000), manifest: { isDynamicEntry: true } },
      "assets/index-HASH.js": { contents: "entry-over-budget", manifest: { isEntry: true } },
    });

    const result = await inspectBundle({ distDirectory, entryBudget: 5, target: "web" });

    expect(result.failures).toHaveLength(2);
    expect(result.failures[0]).toContain("entry");
    expect(result.failures[1]).toContain("application chunk");
  });

  it("rejects missing output and an output built for another target", async () => {
    const missingDirectory = await mkdtemp(path.join(tmpdir(), "quarry-bundle-test-"));
    temporaryDirectories.push(missingDirectory);
    await expect(inspectBundle({ distDirectory: missingDirectory, entryBudget: 100, target: "web" }))
      .rejects.toThrow("Run the matching Vite build first");

    const desktopDirectory = await makeBundle({
      "assets/index.js": { contents: "entry", manifest: { isEntry: true } },
    }, "desktop");
    await expect(inspectBundle({ distDirectory: desktopDirectory, entryBudget: 100, target: "web" }))
      .rejects.toThrow("Bundle target mismatch");
  });
});

async function makeBundle(files, target = "web") {
  const distDirectory = await mkdtemp(path.join(tmpdir(), "quarry-bundle-test-"));
  temporaryDirectories.push(distDirectory);
  await mkdir(path.join(distDirectory, ".vite"), { recursive: true });
  const manifest = {};

  for (const [file, definition] of Object.entries(files)) {
    await mkdir(path.dirname(path.join(distDirectory, file)), { recursive: true });
    await writeFile(path.join(distDirectory, file), definition.contents);
    manifest[file] = { file, ...definition.manifest };
  }

  await writeFile(path.join(distDirectory, ".vite", "manifest.json"), JSON.stringify(manifest));
  await writeFile(path.join(distDirectory, ".vite", "bundle-target.json"), JSON.stringify({ target }));
  return distDirectory;
}
