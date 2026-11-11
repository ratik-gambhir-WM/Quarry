// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createUploadWorkflow } from "@/components/data-room/uploadWorkflow";
import type { ProcessFileJobEventHandlers } from "@/contracts/quarryApi";

afterEach(() => vi.useRealTimers());

function setup(fileCount = 5) {
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
  store.add(Array.from({ length: fileCount }, (_, index) => new File(["bytes"], `${index}.pdf`)));
  return { api, store, stop, handlers, stops };
}

describe("upload lifecycle", () => {
  it("reconnects admitted jobs without uploading again and releases queued work on terminal replay", async () => {
    vi.useFakeTimers();
    const { api, store, stop, handlers, stops } = setup();
    store.upload();
    await Promise.resolve();
    handlers.slice().forEach((handler) => handler.onConnectionError?.());
    expect(stops.every((stop) => stop.mock.calls.length === 1)).toBe(true);
    expect(api.startProcessFile).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(1000);
    expect(api.subscribeToProcessFileJob.mock.calls.map(([id]) => id)).toEqual([
      "0.pdf", "1.pdf", "2.pdf", "0.pdf", "1.pdf", "2.pdf",
    ]);
    // A callback from a disconnected stream cannot complete a current attempt.
    handlers[0].onEvent({ filename: "0.pdf", jobId: "0.pdf", status: "completed" });
    expect(store.getSnapshot().entries[0].status).toBe("processing");
    handlers.slice(3).forEach((handler, index) => handler.onEvent({
      filename: `${index}.pdf`, jobId: `${index}.pdf`, status: "completed",
    }));
    await Promise.resolve();
    expect(api.startProcessFile).toHaveBeenCalledTimes(5);
    handlers.slice(6).forEach((handler, index) => handler.onEvent({
      filename: `${index + 3}.pdf`, jobId: `${index + 3}.pdf`, status: "skipped",
    }));
    expect(store.getSnapshot().entries.every((entry) => ["completed", "skipped"].includes(entry.status))).toBe(true);
    expect(store.getSnapshot().processedSeconds).not.toBeNull();
    stop();
    expect(stops.every((stop) => stop.mock.calls.length === 1)).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("backs off repeated disconnects and cancels reconnects on unmount", async () => {
    vi.useFakeTimers();
    const { api, store, stop, handlers } = setup(1);
    store.upload();
    await Promise.resolve();
    handlers[0].onConnectionError?.();
    handlers[0].onConnectionError?.();
    await vi.advanceTimersByTimeAsync(999);
    expect(handlers).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    handlers[1].onConnectionError?.();
    await vi.advanceTimersByTimeAsync(1999);
    expect(handlers).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    handlers[2].onConnectionError?.();
    stop();
    const stopAgain = store.subscribe(() => {});
    await vi.advanceTimersByTimeAsync(10000);
    expect(handlers).toHaveLength(3);
    expect(api.startProcessFile).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
    stopAgain();
  });

  it("recovers from synchronous subscription failures and cleans up terminal replay on reconnect", async () => {
    vi.useFakeTimers();
    const { api, store, stop } = setup(1);
    const cleanup = vi.fn();
    api.subscribeToProcessFileJob
      .mockImplementationOnce((_id, handlers) => {
        handlers.onConnectionError();
        return cleanup;
      })
      .mockImplementationOnce(() => { throw new Error("Disconnected"); })
      .mockImplementationOnce((id, handlers) => {
        handlers.onEvent({ filename: id, jobId: id, status: "completed" });
        return cleanup;
      });
    store.upload();
    await Promise.resolve();
    expect(cleanup).toHaveBeenCalledOnce();
    await vi.advanceTimersByTimeAsync(3000);
    expect(api.startProcessFile).toHaveBeenCalledOnce();
    expect(store.getSnapshot().entries[0].status).toBe("completed");
    expect(cleanup).toHaveBeenCalledTimes(2);
    expect(vi.getTimerCount()).toBe(0);
    stop();
  });

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
