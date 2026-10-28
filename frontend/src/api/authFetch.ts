import { credentials, sessionStore } from "../auth/sessionStore";

/** One credential snapshot covers identity, cancellation and 401 handling for each request. */
export async function authFetch(url: string, init: RequestInit = {}): Promise<Response> {
  const snapshot = credentials();
  const publicRequest = /\/auth\/(login|register)$/.test(url);
  const headers = new Headers(init.headers);
  if (!publicRequest && snapshot.auth) {
    headers.set("Authorization", `Bearer ${snapshot.auth.token}`);
    headers.set("X-User-Id", String(snapshot.auth.userId));
  }
  const controller = new AbortController();
  const untrack = sessionStore.track(controller);
  const cancel = () => controller.abort();
  const cleanup = () => {
    untrack();
    init.signal?.removeEventListener("abort", cancel);
  };
  if (init.signal?.aborted) cancel();
  init.signal?.addEventListener("abort", cancel, { once: true });

  try {
    const response = await fetch(url, {
      ...init, headers, signal: controller.signal, redirect: "error",
    });
    if (!publicRequest && response.status === 401) {
      sessionStore.clearIfCurrent(snapshot.generation, "Your session expired. Sign in again.");
    }
    // Keep cancellation active while streaming or reading the response body.
    if (!response.body) {
      cleanup();
      return response;
    }
    const reader = response.body.getReader();
    const body = new ReadableStream<Uint8Array>({
      async pull(target) {
        try {
          const chunk = await reader.read();
          if (chunk.done) {
            cleanup();
            target.close();
          } else {
            target.enqueue(chunk.value);
          }
        } catch (error) {
          cleanup();
          target.error(error);
        }
      },
      async cancel() {
        controller.abort();
        cleanup();
        await reader.cancel();
      },
    });
    return new Response(body, {
      status: response.status, statusText: response.statusText, headers: response.headers,
    });
  } catch (error) {
    cleanup();
    throw error;
  }
}
