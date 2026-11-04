import { useMemo, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import type { DealDocumentSummary } from "../contracts/quarryApi";
import {
  buildStoredDocumentNodes,
  flattenDataRoomFiles,
  hasDataRoomFiles,
  isUnconfiguredDataRoomError,
  type DataRoomFileEntry,
  type DataRoomTreeNode,
} from "../data/dataRoom";
import type { DealDataRoom } from "../data/dataRoomPreview";
type Resource<T> =
  | { status: "loading"; previous?: T }
  | { status: "ready"; value: T }
  | { status: "error"; message: string; previous?: T };
export type DataRoomContents = {
  errorMessage: string;
  explorerNodes: DataRoomTreeNode[];
  hasFiles: boolean;
  isEmpty: boolean;
  isLoading: boolean;
  isUnavailable: boolean;
  reloadAll: () => void;
  reloadStored: () => void;
  reviewFiles: DataRoomFileEntry[];
  rootPath?: string;
};
function value<T>(resource: Resource<T>) {
  return resource.status === "ready" ? resource.value : resource.previous;
}
function createContentsStore(dealId: string, localAllowed: boolean) {
  let local: Resource<DealDataRoom> = localAllowed
    ? { status: "loading" }
    : {
        status: "ready",
        value: { dealId, rootName: "Data Room", rootPath: "", tree: [] },
      };
  let stored: Resource<DealDocumentSummary[]> = { status: "loading" };
  let localRequest = 0;
  let storedRequest = 0;
  const listeners = new Set<() => void>();
  let snapshot: {
    local: Resource<DealDataRoom>;
    stored: Resource<DealDocumentSummary[]>;
  } = { local, stored };
  const publish = () => {
    snapshot = { local, stored };
    for (const listener of listeners) listener();
  };
  const loadLocal = () => {
    if (!localAllowed) return;
    const request = ++localRequest;
    local = { status: "loading", previous: value(local) };
    publish();
    void runtime.api
      .listDealDataRoom(dealId)
      .then((next) => {
        if (request !== localRequest) return;
        local = { status: "ready", value: next };
        publish();
      })
      .catch((error: unknown) => {
        if (request !== localRequest) return;
        local = isUnconfiguredDataRoomError(error)
          ? {
              status: "ready",
              value: { dealId, rootName: "Data Room", rootPath: "", tree: [] },
            }
          : {
              status: "error",
              message: error instanceof Error ? error.message : String(error),
            };
        publish();
      });
  };
  const loadStored = () => {
    const request = ++storedRequest;
    stored = { status: "loading", previous: value(stored) };
    publish();
    void runtime.api
      .listDealDocuments(dealId)
      .then((next) => {
        if (request !== storedRequest) return;
        stored = { status: "ready", value: next };
        publish();
      })
      .catch((error: unknown) => {
        if (request !== storedRequest) return;
        stored = {
          status: "error",
          message: error instanceof Error ? error.message : String(error),
        };
        publish();
      });
  };
  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (listeners.size === 1) {
        loadLocal();
        loadStored();
      }
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0) {
          localRequest++;
          storedRequest++;
        }
      };
    },
    loadAll: () => {
      loadLocal();
      loadStored();
    },
    loadStored,
  };
}
export function useDataRoomContents(
  dealId: string,
  localAllowed = true,
): DataRoomContents {
  const store = useMemo(
    () => createContentsStore(dealId, localAllowed),
    [dealId, localAllowed],
  );
  const { local, stored } = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
  return useMemo(() => {
    const localValue = value(local);
    const storedValue = value(stored);
    const explorerNodes = [
      ...buildStoredDocumentNodes(storedValue ?? []),
      ...(localValue?.tree ?? []),
    ];
    const errorMessage = [
      stored.status === "error" ? stored.message : "",
      local.status === "error" ? local.message : "",
    ]
      .filter(Boolean)
      .join(" ");
    const isLoading = local.status === "loading" || stored.status === "loading";
    const hasFiles = hasDataRoomFiles(explorerNodes);
    return {
      errorMessage,
      explorerNodes,
      hasFiles,
      isEmpty: !isLoading && !errorMessage && !hasFiles,
      isLoading,
      isUnavailable: !isLoading && Boolean(errorMessage),
      reloadAll: store.loadAll,
      reloadStored: store.loadStored,
      reviewFiles: flattenDataRoomFiles(explorerNodes),
      rootPath: localValue?.rootPath || undefined,
    };
  }, [local, stored, store]);
}
