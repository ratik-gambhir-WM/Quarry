// The browser owns focus events; subscribers only observe a revision.
let revision = 0;
const listeners = new Set<() => void>();
function focus() {
  revision++;
  for (const listener of listeners) listener();
}
export const browserFocusStore = {
  getSnapshot: () => revision,
  subscribe(listener: () => void) {
    listeners.add(listener);
    if (listeners.size === 1) window.addEventListener("focus", focus);
    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) window.removeEventListener("focus", focus);
    };
  },
};
