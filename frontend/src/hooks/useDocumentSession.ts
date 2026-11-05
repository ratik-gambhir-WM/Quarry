import { useEffect, useState, useSyncExternalStore } from "react";
import { runtime } from "@quarry/runtime";
import type { DataRoomTreeNode } from "../data/dataRoom";
import {
  DocumentPreviewCache,
  DOCUMENT_PREVIEW_CACHE_BYTES,
} from "../data/documentPreviewCache";
import {
  isDocumentPreviewResponse,
  type PreviewState,
  type RawTextState,
} from "../data/dataRoomPreview";

export type DocumentSessionState =
  | { status: "closed" }
  | {
      document: DataRoomTreeNode;
      preview: PreviewState;
      rawText: RawTextState;
      status: "open";
    };

type OpenTab = {
  document: DataRoomTreeNode;
  previewRequest?: Promise<void>;
  textRequest?: Promise<void>;
};

/** Owns open-tab identities, request lifetimes, and a byte-bounded payload cache. */
class DocumentSession {
  private tabs = new Map<string, OpenTab>();
  private cache: DocumentPreviewCache;
  private listeners = new Set<() => void>();
  private snapshot: {
    documents: DataRoomTreeNode[];
    state: DocumentSessionState;
  } = {
    documents: [],
    state: { status: "closed" },
  };

  constructor(
    private readonly dealId: string,
    cacheBytes: number,
  ) {
    this.cache = new DocumentPreviewCache(cacheBytes);
  }

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };
  getSnapshot = () => this.snapshot;

  private publish(state: DocumentSessionState) {
    this.snapshot = {
      documents: [...this.tabs.values()].map((tab) => tab.document),
      state,
    };
    this.listeners.forEach((listener) => listener());
  }

  private update(
    tab: OpenTab,
    update: Partial<Extract<DocumentSessionState, { status: "open" }>>,
  ) {
    const current = this.snapshot.state;
    if (
      this.tabs.get(tab.document.id) === tab &&
      current.status === "open" &&
      current.document.id === tab.document.id
    ) {
      this.publish({ ...current, ...update });
    }
  }

  selectDocument = async (document: DataRoomTreeNode) => {
    const current = this.snapshot.state;
    if (
      current.status === "open" &&
      current.document.id === document.id &&
      current.preview.status !== "error"
    )
      return;
    let tab = this.tabs.get(document.id);
    if (!tab) {
      tab = { document };
      this.tabs.set(document.id, tab);
    }
    const cached = this.cache.get(document.id);
    this.publish({
      document,
      status: "open",
      preview: cached
        ? { status: "ready", response: cached.response }
        : { status: "loading" },
      rawText: cached?.rawText ?? { status: "idle" },
    });
    if (cached) return;
    if (tab.previewRequest) return tab.previewRequest;
    tab.previewRequest = this.loadPreview(tab);
    await tab.previewRequest;
    tab.previewRequest = undefined;
  };

  private async loadPreview(tab: OpenTab) {
    const document = tab.document;
    try {
      if (!document.storedFileId && !document.relativePath) {
        throw new Error("This selection is not a previewable file.");
      }
      const response = document.storedFileId
        ? await runtime.api
            .getDealDocumentPdf(this.dealId, document.storedFileId)
            .then((pdf) => ({
              fileName: document.name,
              mimeType: pdf.mimeType,
              pdfBytes: pdf.bytes,
              sourceKind: "stored" as const,
            }))
        : await runtime.api.previewDealDocument(
            this.dealId,
            document.relativePath ?? "",
          );
      if (!isDocumentPreviewResponse(response)) {
        throw new Error(
          "The preview backend returned an invalid PDF response.",
        );
      }
      if (this.tabs.get(document.id) !== tab) return;
      this.cache.set(document.id, { response, rawText: { status: "idle" } });
      this.update(tab, { preview: { response, status: "ready" } });
    } catch (error) {
      this.update(tab, {
        preview: { message: toErrorMessage(error), status: "error" },
      });
    }
  }

  showFiles = () => {
    this.publish({ status: "closed" });
  };

  closeDocument = (id?: string) => {
    const current = this.snapshot.state;
    const documentId =
      id ?? (current.status === "open" ? current.document.id : undefined);
    if (!documentId) return;
    const documents = this.snapshot.documents;
    const index = documents.findIndex((document) => document.id === documentId);
    this.tabs.delete(documentId);
    this.cache.delete(documentId);
    if (current.status === "open" && current.document.id === documentId) {
      const remaining = documents.filter(
        (document) => document.id !== documentId,
      );
      const next = remaining[Math.min(index, remaining.length - 1)];
      this.publish({ status: "closed" });
      if (next) void this.selectDocument(next);
    } else {
      this.publish(current);
    }
  };

  requestRawText = async () => {
    const current = this.snapshot.state;
    if (current.status !== "open") return;
    const tab = this.tabs.get(current.document.id);
    if (!tab || tab.textRequest || current.rawText.status === "ready") return;
    if (!tab.document.storedFileId) {
      this.update(tab, {
        rawText: {
          message: "Raw text is unavailable for this file.",
          status: "error",
        },
      });
      return;
    }
    this.update(tab, { rawText: { status: "loading" } });
    tab.textRequest = this.loadRawText(tab, tab.document.storedFileId);
    await tab.textRequest;
    tab.textRequest = undefined;
  };

  private async loadRawText(tab: OpenTab, storedFileId: string) {
    let rawText: RawTextState;
    try {
      const response = await runtime.api.getDealDocumentText(
        this.dealId,
        storedFileId,
      );
      rawText = { response, status: "ready" };
    } catch (error) {
      rawText = { message: toErrorMessage(error), status: "error" };
    }
    if (this.tabs.get(tab.document.id) !== tab) return;
    const cached = this.cache.get(tab.document.id);
    if (cached) this.cache.set(tab.document.id, { ...cached, rawText });
    this.update(tab, { rawText });
  }

  release = () => {
    this.tabs.clear();
    this.cache.clear();
    this.snapshot = { documents: [], state: { status: "closed" } };
  };
}

export function useDocumentSession(
  dealId: string,
  cacheBytes = DOCUMENT_PREVIEW_CACHE_BYTES,
) {
  // DataRoomWorkspace is keyed by deal ID, keeping payloads scoped to that route mount.
  const [session] = useState(() => new DocumentSession(dealId, cacheBytes));
  const snapshot = useSyncExternalStore(session.subscribe, session.getSnapshot);
  useEffect(() => session.release, [session]);
  return {
    ...snapshot,
    closeDocument: session.closeDocument,
    requestRawText: session.requestRawText,
    selectDocument: session.selectDocument,
    showFiles: session.showFiles,
  };
}

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
