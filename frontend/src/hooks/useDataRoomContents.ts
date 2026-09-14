import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

type DataRoomResource<T> =
  | { status: "loading"; previous?: T }
  | { status: "ready"; value: T }
  | { message: string; status: "error"; previous?: T };

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

export function useDataRoomContents(dealId: string): DataRoomContents {
  const [local, setLocal] = useState<DataRoomResource<DealDataRoom>>({ status: "loading" });
  const [stored, setStored] = useState<DataRoomResource<DealDocumentSummary[]>>({ status: "loading" });
  const localRequestId = useRef(0);
  const storedRequestId = useRef(0);
  const activeDealId = useRef(dealId);
  activeDealId.current = dealId;

  const loadLocal = useCallback(async () => {
    const requestId = ++localRequestId.current;
    setLocal((current) => ({ status: "loading", previous: getResourceValue(current) }));
    try {
      const value = await runtime.api.listDealDataRoom(dealId);
      if (activeDealId.current === dealId && localRequestId.current === requestId) {
        setLocal({ status: "ready", value });
      }
    } catch (error) {
      if (activeDealId.current !== dealId || localRequestId.current !== requestId) return;
      if (isUnconfiguredDataRoomError(error)) {
        setLocal({
          status: "ready",
          value: { dealId, rootName: "Data Room", rootPath: "", tree: [] },
        });
      } else {
        setLocal((current) => ({
          message: toErrorMessage(error),
          previous: getResourceValue(current),
          status: "error",
        }));
      }
    }
  }, [dealId]);

  const loadStored = useCallback(async () => {
    const requestId = ++storedRequestId.current;
    setStored((current) => ({ status: "loading", previous: getResourceValue(current) }));
    try {
      const value = await runtime.api.listDealDocuments(dealId);
      if (activeDealId.current === dealId && storedRequestId.current === requestId) {
        setStored({ status: "ready", value });
      }
    } catch (error) {
      if (activeDealId.current === dealId && storedRequestId.current === requestId) {
        setStored((current) => ({
          message: toErrorMessage(error),
          previous: getResourceValue(current),
          status: "error",
        }));
      }
    }
  }, [dealId]);

  useEffect(() => {
    void loadLocal();
    void loadStored();
    return () => {
      localRequestId.current += 1;
      storedRequestId.current += 1;
    };
  }, [loadLocal, loadStored]);

  const reloadAll = useCallback(() => {
    void loadLocal();
    void loadStored();
  }, [loadLocal, loadStored]);
  const reloadStored = useCallback(() => {
    void loadStored();
  }, [loadStored]);

  return useMemo(() => {
    const localValue = getResourceValue(local);
    const storedValue = getResourceValue(stored);
    const explorerNodes = [
      ...buildStoredDocumentNodes(storedValue ?? []),
      ...(localValue?.tree ?? []),
    ];
    const errorMessage = [
      stored.status === "error" ? stored.message : "",
      local.status === "error" ? local.message : "",
    ].filter(Boolean).join(" ");
    const isLoading = local.status === "loading" || stored.status === "loading";
    const hasFiles = hasDataRoomFiles(explorerNodes);

    return {
      errorMessage,
      explorerNodes,
      hasFiles,
      isEmpty: localValue !== undefined && !isLoading && !errorMessage && !hasFiles,
      isLoading,
      isUnavailable: !isLoading && Boolean(errorMessage),
      reloadAll,
      reloadStored,
      reviewFiles: flattenDataRoomFiles(explorerNodes),
      rootPath: localValue?.rootPath || undefined,
    };
  }, [local, reloadAll, reloadStored, stored]);
}

function getResourceValue<T>(resource: DataRoomResource<T>) {
  return resource.status === "ready" ? resource.value : resource.previous;
}

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
