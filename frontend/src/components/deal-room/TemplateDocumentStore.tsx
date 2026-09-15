import {
  createContext,
  useContext,
  useMemo,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { runtime } from "@quarry/runtime";

import type { DiligenceCanvasDocument } from "@/contracts/diligenceCanvas";

export type TemplateDocumentState =
  | { status: "loading" }
  | { message: string; status: "error" }
  | { document: DiligenceCanvasDocument; isDirty: boolean; status: "success" };

type TemplateDocumentStore = {
  getSnapshot: () => TemplateDocumentState;
  retry: () => void;
  subscribe: (listener: () => void) => () => void;
  updateDocument: (document: DiligenceCanvasDocument) => void;
};

const TemplateDocumentContext = createContext<TemplateDocumentStore | null>(null);

export function TemplateDocumentProvider({
  children,
  requestKey,
  templateId,
}: {
  children: ReactNode;
  requestKey: string;
  templateId: string;
}) {
  const store = useMemo(
    () => createTemplateDocumentStore(templateId),
    [requestKey, templateId],
  );
  return (
    <TemplateDocumentContext.Provider value={store}>
      {children}
    </TemplateDocumentContext.Provider>
  );
}

export function useTemplateDocumentState() {
  const store = useTemplateDocumentStore();
  return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
}

export function useTemplateDocumentActions() {
  const store = useTemplateDocumentStore();
  return { retry: store.retry, updateDocument: store.updateDocument };
}

function useTemplateDocumentStore() {
  const store = useContext(TemplateDocumentContext);
  if (!store) {
    throw new Error("Template document state must be used within TemplateDocumentProvider.");
  }
  return store;
}

function createTemplateDocumentStore(templateId: string): TemplateDocumentStore {
  let state: TemplateDocumentState = { status: "loading" };
  let started = false;
  let generation = 0;
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of listeners) listener();
  };

  const load = () => {
    const requestGeneration = ++generation;
    started = true;
    state = { status: "loading" };
    notify();
    const performLoad = async () => {
      try {
        const document = await runtime.api.getTemplate(templateId);
        if (requestGeneration !== generation || listeners.size === 0) return;
        state = { document, isDirty: false, status: "success" };
        notify();
      } catch {
        if (requestGeneration !== generation || listeners.size === 0) return;
        state = {
          message: "This template could not be opened.",
          status: "error",
        };
        notify();
      }
    };
    void performLoad();
  };

  return {
    getSnapshot: () => state,
    retry: () => {
      if (listeners.size > 0) load();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      if (!started) load();
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          generation += 1;
          started = false;
          state = { status: "loading" };
        }
      };
    },
    updateDocument: (document) => {
      if (state.status !== "success") return;
      state = { document, isDirty: true, status: "success" };
      notify();
    },
  };
}
