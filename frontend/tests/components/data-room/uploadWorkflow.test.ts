// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { createUploadWorkflow } from "@/components/data-room/uploadWorkflow";
import type { ProcessFileJobEventHandlers } from "@/contracts/quarryApi";

function setup() {
  const handlers: ProcessFileJobEventHandlers[] = [];
  const stops: ReturnType<typeof vi.fn>[] = [];
  const api = {
    startProcessFile: vi.fn().mockImplementation(async (_id, file) => ({ jobId: file.name, filename: file.name })),
    subscribeToProcessFileJob: vi.fn().mockImplementation((_id, next) => {
      handlers.push(next);
      const stop = vi.fn();
      stops.push(stop);
      return stop;
    }),
  };
  const store = createUploadWorkflow(api, "deal");
  const stop = store.subscribe(() => {});
  store.add(Array.from({ length: 5 }, (_, index) => new File(["bytes"], `${index}.pdf`)));
  return { api, store, stop, handlers, stops };
}

describe("upload lifecycle", () => {
  it("bounds active jobs, releases slots after terminal events, and retries failures once", async () => {
    const { api, store, stop, handlers, stops } = setup();
    store.upload();
    store.upload();
    await vi.waitFor(() => expect(handlers).toHaveLength(3));
    expect(api.startProcessFile).toHaveBeenCalledTimes(3);
    expect(store.getSnapshot().entries.filter((entry) => entry.status === "queued")).toHaveLength(2);
    handlers[0].onEvent({ filename: "test.pdf", jobId: "0.pdf", status: "failed", error: "Try again" });
    await vi.waitFor(() => expect(handlers).toHaveLength(4));
    expect(stops[0]).toHaveBeenCalledOnce();
    handlers[0].onEvent({ filename: "test.pdf", jobId: "0.pdf", status: "completed" });
    expect(store.getSnapshot().entries[0].status).toBe("failed");
    store.upload();
    expect(api.startProcessFile).toHaveBeenCalledTimes(4);
    handlers[1].onEvent({ filename: "test.pdf", jobId: "1.pdf", status: "completed" });
    await vi.waitFor(() => expect(handlers).toHaveLength(5));
    expect(api.startProcessFile.mock.calls[4][1].name).toBe("0.pdf");
    handlers[0].onEvent({ filename: "test.pdf", jobId: "old-attempt", status: "completed" });
    expect(store.getSnapshot().entries[0].status).toBe("processing");
    stop();
    expect(stops.slice(2).every((stop) => stop.mock.calls.length === 1)).toBe(true);
  });

  it("ignores admission responses after unmount, including a resubscription", async () => {
    const { api, store, stop } = setup();
    let resolve = (_job: { jobId: string }) => {};
    api.startProcessFile.mockImplementation(() => new Promise((done) => { resolve = done; }));
    store.upload();
    stop();
    const stopAgain = store.subscribe(() => {});
    resolve({ jobId: "late" });
    await Promise.resolve();
    expect(api.subscribeToProcessFileJob).not.toHaveBeenCalled();
    expect(api.startProcessFile).toHaveBeenCalledTimes(3);
    stopAgain();
  });

  it("does not subscribe if its owner unmounts as admission completes", async () => {
    const api = {
      startProcessFile: vi.fn().mockResolvedValue({ jobId: "job", filename: "test.pdf" }),
      subscribeToProcessFileJob: vi.fn(),
    };
    const store = createUploadWorkflow(api, "deal");
    const stop = store.subscribe(() => {
      if (store.getSnapshot().entries[0]?.status === "processing") stop();
    });
    store.add([new File(["bytes"], "test.pdf")]);
    store.upload();
    await Promise.resolve();
    expect(api.startProcessFile).toHaveBeenCalledOnce();
    expect(api.subscribeToProcessFileJob).not.toHaveBeenCalled();
  });

  it("cleans up synchronous terminal replay and completes queued jobs without duplicates", async () => {
    const { api, store, stop } = setup();
    const unsubscribe = vi.fn();
    api.subscribeToProcessFileJob.mockImplementation((id, handlers) => {
      handlers.onEvent({ filename: "test.pdf", jobId: id, status: "skipped" });
      return unsubscribe;
    });
    store.upload();
    await vi.waitFor(() => expect(api.startProcessFile).toHaveBeenCalledTimes(5));
    expect(store.getSnapshot().entries.every((entry) => entry.status === "skipped")).toBe(true);
    expect(store.getSnapshot().processedSeconds).toBe(1);
    expect(unsubscribe).toHaveBeenCalledTimes(5);
    stop();
    expect(unsubscribe).toHaveBeenCalledTimes(5);
  });
});
