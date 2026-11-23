// @vitest-environment happy-dom
import { describe, expect, it, vi } from "vitest";
import { createUploadWorkflow } from "@/components/data-room/uploadWorkflow";
import type { ProcessFileJobResponse, UploadProgressEvent } from "@/contracts/quarryApi";

function setup(count = 7) {
  const runs: {
    progress: (event: UploadProgressEvent) => void;
    accept: (value: ProcessFileJobResponse) => void;
    fail: (error: Error) => void;
  }[] = [];
  const api = {
    startProcessFile: vi.fn((_deal: string, _file: File, progress?: (event: UploadProgressEvent) => void) =>
      new Promise<ProcessFileJobResponse>((accept, fail) => runs.push({ progress: progress ?? (() => {}), accept, fail }))),
  };
  const store = createUploadWorkflow(api, "deal");
  const stop = store.subscribe(() => {});
  store.add(Array.from({ length: count }, (_, index) => new File(["bytes"], `${index}.pdf`)));
  const event = (status: UploadProgressEvent["status"]): UploadProgressEvent => ({ jobId: "job", filename: "0.pdf", status });
  return { api, store, stop, runs, event };
}

describe("upload admission lifecycle", () => {
  it("bounds uploads at five, rejects duplicate submissions, and releases slots on acceptance without waiting for embedding", async () => {
    const { api, store, stop, runs, event } = setup();
    store.upload();
    store.upload();
    expect(api.startProcessFile).toHaveBeenCalledTimes(5);
    expect(store.getSnapshot().entries.filter(entry => entry.status === "queued")).toHaveLength(2);
    runs[0].progress(event("parsing"));
    expect(store.getSnapshot().entries[0].stage).toBe("parsing");
    runs[0].progress(event("classifying"));
    expect(store.getSnapshot().entries[0].stage).toBe("classifying");
    runs[0].accept(event("accepted"));
    await Promise.resolve();
    expect(store.getSnapshot().entries[0].status).toBe("completed");
    expect(api.startProcessFile).toHaveBeenCalledTimes(6);
    // Late progress from the completed attempt cannot change the accepted row.
    runs[0].progress(event("parsing"));
    expect(store.getSnapshot().entries[0].stage).toBe("accepted");
    stop();
  });

  it("keeps admission failures retryable and ignores callbacks from the previous attempt", async () => {
    const { api, store, stop, runs, event } = setup(1);
    store.upload();
    runs[0].fail(new Error("Connection interrupted"));
    await Promise.resolve();
    expect(store.getSnapshot().entries[0].status).toBe("failed");
    store.upload();
    expect(api.startProcessFile).toHaveBeenCalledTimes(2);
    runs[1].progress(event("classifying"));
    runs[0].progress(event("accepted"));
    expect(store.getSnapshot().entries[0].stage).toBe("classifying");
    runs[1].accept(event("accepted"));
    await Promise.resolve();
    expect(store.getSnapshot().entries[0].status).toBe("completed");
    expect(store.getSnapshot().processedSeconds).not.toBeNull();
    stop();
  });

  it("ignores late progress and acceptance after unmount and resubscription", async () => {
    const { store, stop, runs, event } = setup(1);
    store.upload();
    stop();
    const stopAgain = store.subscribe(() => {});
    const previous = store.getSnapshot();
    runs[0].progress(event("classifying"));
    runs[0].accept(event("accepted"));
    await Promise.resolve();
    expect(store.getSnapshot()).toBe(previous);
    stopAgain();
  });
});
