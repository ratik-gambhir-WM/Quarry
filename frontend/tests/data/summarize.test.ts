import { describe, expect, it } from "vitest";
import { buildFileTree, formatBytes, toSummarizableFile } from "@/data/summarize";

describe("summarize data model", () => {
  it("normalizes supported browser files and builds stable nested counts", () => {
    const file = new File(["data"], "report.pdf", { type: "application/pdf" });
    Object.defineProperty(file, "webkitRelativePath", { value: "Folder/Sub/report.pdf" });
    const normalized = toSummarizableFile(file);
    const tree = buildFileTree([normalized], "Folder");

    expect(normalized).toMatchObject({ path: "Folder/Sub/report.pdf", supported: true });
    expect(tree).toMatchObject({ fileCount: 1, name: "Folder", supportedFileCount: 1 });
    expect(tree.folders[0]?.folders[0]?.files[0]?.filename).toBe("report.pdf");
  });

  it("formats zero and binary units", () => {
    expect(formatBytes(0)).toBe("0 bytes");
    expect(formatBytes(1536)).toBe("1.5 KB");
  });
});
