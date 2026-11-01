/** Lifecycle-owned async resource. React only subscribes; requests never start during render. */
export type RequestState<T> =
  | { status: "loading" }
  | { status: "ready"; value: T; pending: boolean; message?: string }
  | { status: "error"; error: unknown; message: string };

export function createRequestResource<T>(
  load: () => Promise<T>,
  options: {
    onSuccess?: (value: T) => void;
    onError?: (error: unknown) => void;
    pollMs?: number;
    refreshOnFocus?: boolean;
  } = {},
) {
  let snapshot: RequestState<T> = { status: "loading" };
  let generation = 0;
  let running = false;
  let interval: ReturnType<typeof setInterval> | undefined;
  const listeners = new Set<() => void>();
  const publish = (next: RequestState<T>) => {
    snapshot = next;
    for (const listener of listeners) listener();
  };
  const perform = async (request: () => Promise<T>, retainValue: boolean) => {
    if (running || listeners.size === 0) return;
    const current = ++generation;
    running = true;
    const previous = snapshot.status === "ready" ? snapshot.value : undefined;
    if (retainValue && snapshot.status === "ready") {
      publish({ ...snapshot, pending: true, message: undefined });
    } else if (!retainValue) publish({ status: "loading" });
    try {
      const value = await request();
      if (current !== generation || listeners.size === 0) return;
      options.onSuccess?.(value);
      publish({ status: "ready", value, pending: false });
    } catch (error) {
      if (current !== generation || listeners.size === 0) return;
      options.onError?.(error);
      const message =
        error instanceof Error ? error.message : "Request failed. Try again.";
      if (
        retainValue &&
        previous !== undefined &&
        !options.onError &&
        !accessDenied(error)
      ) {
        publish({ status: "ready", value: previous, pending: false, message });
      } else publish({ status: "error", error, message });
    } finally {
      if (current === generation) running = false;
    }
  };
  const refresh = () => void perform(load, false);
  const revalidate = () => void perform(load, true);
  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (listeners.size === 1) {
        refresh();
        if (options.refreshOnFocus)
          window.addEventListener("focus", revalidate);
        if (options.pollMs) interval = setInterval(revalidate, options.pollMs);
      }
      return () => {
        listeners.delete(listener);
        if (listeners.size !== 0) return;
        generation++;
        running = false;
        if (interval) clearInterval(interval);
        if (options.refreshOnFocus)
          window.removeEventListener("focus", revalidate);
      };
    },
    refresh,
    revalidate,
    update: (request: (value: T) => Promise<T>) => {
      if (snapshot.status !== "ready" || running) return;
      const value = snapshot.value;
      void perform(() => request(value), true);
    },
  };
}

function accessDenied(error: unknown) {
  return (
    typeof error === "object" &&
    error !== null &&
    "status" in error &&
    (error.status === 401 || error.status === 403 || error.status === 404)
  );
}
