// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { createRequestResource } from "@/data/requests/requestResource";
afterEach(() => vi.useRealTimers());
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};
describe("request resource lifecycle", () => {
  it("does not make requests during construction, shares its subscription and ignores disposed responses", async () => {
    let finish: (value: number) => void = () => {};
    const load = vi.fn(
      () =>
        new Promise<number>((resolve) => {
          finish = resolve;
        }),
    );
    const accepted = vi.fn();
    const resource = createRequestResource(load, { onSuccess: accepted });
    expect(load).not.toHaveBeenCalled();
    const first = resource.subscribe(vi.fn());
    const second = resource.subscribe(vi.fn());
    expect(load).toHaveBeenCalledTimes(1);
    first();
    second();
    finish(42);
    await flush();
    expect(accepted).not.toHaveBeenCalled();
    expect(resource.getSnapshot().status).toBe("loading");
  });
  it("revalidates on focus and polling, fails closed on access loss, and cleans up listeners", async () => {
    vi.useFakeTimers();
    const load = vi.fn().mockResolvedValue(1);
    const denied = vi.fn();
    const resource = createRequestResource(load, {
      refreshOnFocus: true,
      pollMs: 5000,
      onError: denied,
    });
    const stop = resource.subscribe(vi.fn());
    await flush();
    expect(resource.getSnapshot()).toEqual({
      status: "ready",
      value: 1,
      pending: false,
    });
    window.dispatchEvent(new Event("focus"));
    await flush();
    expect(load).toHaveBeenCalledTimes(2);
    load.mockRejectedValueOnce(
      Object.assign(new Error("revoked"), { status: 404 }),
    );
    await vi.advanceTimersByTimeAsync(5000);
    expect(resource.getSnapshot().status).toBe("error");
    expect(denied).toHaveBeenCalledOnce();
    stop();
    window.dispatchEvent(new Event("focus"));
    await vi.advanceTimersByTimeAsync(5000);
    expect(load).toHaveBeenCalledTimes(3);
  });
  it("keeps prior pagination on a retryable failure but clears it for permission errors", async () => {
    const resource = createRequestResource(async () => [1]);
    const stop = resource.subscribe(vi.fn());
    await flush();
    resource.update(async () => {
      throw new Error("temporarily unavailable");
    });
    await flush();
    expect(resource.getSnapshot()).toMatchObject({
      status: "ready",
      value: [1],
      message: "temporarily unavailable",
    });
    resource.update(async () => {
      throw Object.assign(new Error("revoked"), { status: 403 });
    });
    await flush();
    expect(resource.getSnapshot().status).toBe("error");
    stop();
  });
});
