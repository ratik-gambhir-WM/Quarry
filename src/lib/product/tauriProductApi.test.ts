import { beforeEach, describe, expect, it, vi } from "vitest";
import { execute } from "../tauri/command";
import { tauriProductApi } from "./tauriProductApi";

vi.mock("../tauri/command", () => ({ execute: vi.fn() }));

describe("tauriProductApi", () => {
  beforeEach(() => {
    vi.mocked(execute).mockReset();
  });

  it("uses the stable frontend argument name when loading a deal", async () => {
    vi.mocked(execute).mockResolvedValue({ id: 42 });

    await tauriProductApi.getDeal(42);

    expect(execute).toHaveBeenCalledWith("get_deal", { dealId: 42 });
  });

  it("loads active deals without transport-specific arguments", async () => {
    vi.mocked(execute).mockResolvedValue([]);

    await tauriProductApi.listDeals();

    expect(execute).toHaveBeenCalledWith("list_deals");
  });

  it("starts native document jobs without file bytes", async () => {
    vi.mocked(execute).mockResolvedValue({ jobs: [] });

    await tauriProductApi.startDocumentJobs({
      paths: ["/selected/memo.pdf"],
      userId: "user-1",
    });

    expect(execute).toHaveBeenCalledWith("start_document_jobs", {
      input: { paths: ["/selected/memo.pdf"], userId: "user-1" },
    });
  });

  it("describes native file drops without sending file bytes", async () => {
    vi.mocked(execute).mockResolvedValue([]);

    await tauriProductApi.describeDocumentFiles(["/dropped/memo.pdf"]);

    expect(execute).toHaveBeenCalledWith("describe_document_files", {
      paths: ["/dropped/memo.pdf"],
    });
  });

  it("exposes bounded Helix search through the typed adapter", async () => {
    vi.mocked(execute).mockResolvedValue([]);

    await tauriProductApi.searchDocumentChunksKeyword({
      limit: 10,
      queryText: "revenue",
      userId: "user-1",
    });

    expect(execute).toHaveBeenCalledWith("search_document_chunks_keyword", {
      input: { limit: 10, queryText: "revenue", userId: "user-1" },
    });
  });
});
