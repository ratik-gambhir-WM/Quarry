import { afterEach, expect, it, vi } from "vitest";
import { BackendApiError, httpQuarryApi } from "@/api/httpQuarryApi";
import { sessionStore } from "@/auth/sessionStore";
import { authFetch } from "@/api/authFetch";
import { testSession } from "@/test/fixtures/auth";

afterEach(() => {
  sessionStore.set(null);
  vi.unstubAllGlobals();
});

it.each([
  ['{"error":"Session revoked"}', "Session revoked"],
  ["Session expired", "Session expired"],
])(
  "preserves a 401 body (%s) while clearing the session and cancelling other work",
  async (body, message) => {
    sessionStore.set(testSession);
    const otherRequest = new AbortController();
    sessionStore.track(otherRequest);
    vi.stubGlobal(
      "fetch",
      vi.fn((_url: string, init: RequestInit) => {
        // Model fetch's body cancellation: aborting after headers still rejects body reads.
        const stream = new ReadableStream<Uint8Array>(
          {
            start(controller) {
              init.signal?.addEventListener(
                "abort",
                () => {
                  controller.error(
                    new DOMException("Request aborted", "AbortError"),
                  );
                },
                { once: true },
              );
            },
            pull(controller) {
              controller.enqueue(new TextEncoder().encode(body));
              controller.close();
            },
          },
          { highWaterMark: 0 },
        );
        return Promise.resolve(new Response(stream, { status: 401 }));
      }),
    );

    const request = httpQuarryApi.listDeals();
    await expect(request).rejects.toBeInstanceOf(BackendApiError);
    await expect(request).rejects.toMatchObject({ status: 401, message });
    expect(sessionStore.getSnapshot().session).toBeNull();
    expect(otherRequest.signal.aborted).toBe(true);
  },
);

it.each([204, 205, 304])(
  "preserves bodyless status %s from a fetch bridge and releases cancellation tracking",
  async (status) => {
    sessionStore.set(testSession);
    const response = new Response(
      new ReadableStream<Uint8Array>({
        start(controller) {
          controller.close();
        },
      }),
    );
    // Model a bridge that represents the empty body as a stream.
    Object.defineProperty(response, "status", { value: status });
    let signal: AbortSignal | null | undefined;
    vi.stubGlobal(
      "fetch",
      vi.fn((_url: string, init: RequestInit) => {
        signal = init.signal;
        return Promise.resolve(response);
      }),
    );
    expect(
      await authFetch("/api/v1/deals/DEAL-1/teams/team/members", {
        method: "POST",
      }),
    ).toBe(response);
    sessionStore.set(null);
    expect(signal?.aborted).toBe(false);
  },
);
