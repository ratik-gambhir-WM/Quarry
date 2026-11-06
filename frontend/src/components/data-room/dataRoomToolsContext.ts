import { createContext, useContext, useEffect } from "react";
import type { DataRoomDocumentSearchProps } from "./document-search/DataRoomDocumentSearch";

export type DocumentSearchContext = Omit<
  DataRoomDocumentSearchProps,
  "dealId" | "trigger" | "finalFocusEl" | "open" | "onOpenChange"
>;

// The selected viewer registers its live page/focus capabilities. The shell subscribes
// without copying document state or retaining callbacks after the viewer unmounts.
export function createSearchRegistry() {
  let snapshot: DocumentSearchContext | null = null;
  const listeners = new Set<() => void>();
  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    register(value: DocumentSearchContext) {
      snapshot = value;
      listeners.forEach((listener) => listener());
      return () => {
        if (snapshot !== value) return;
        snapshot = null;
        listeners.forEach((listener) => listener());
      };
    },
  };
}

export type DataRoomTool = "assistant" | "search";
export const ToolsContext = createContext<{
  launch: (tool: DataRoomTool, trigger: HTMLButtonElement | null) => void;
  registry: ReturnType<typeof createSearchRegistry>;
} | null>(null);

export function useDataRoomDocumentSearch(value: DocumentSearchContext | null) {
  const registry = useContext(ToolsContext)?.registry;
  useEffect(() => {
    if (value && registry) return registry.register(value);
  }, [registry, value]);
}

