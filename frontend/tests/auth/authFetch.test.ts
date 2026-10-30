import { afterEach, expect, it, vi } from "vitest";
import { BackendApiError, httpQuarryApi } from "@/api/httpQuarryApi";
import { sessionStore } from "@/auth/sessionStore";
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
