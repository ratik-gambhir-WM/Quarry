import { useCallback, useRef, useState } from "react";
import { runtime } from "@quarry/runtime";
import type { DataRoomTreeNode } from "../data/dataRoom";
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

export function useDocumentSession(dealId: string) {
  const [state, setState] = useState<DocumentSessionState>({ status: "closed" });
  const previewRequestId = useRef(0);
  const rawTextRequestId = useRef(0);

  const closeDocument = useCallback(() => {
    previewRequestId.current += 1;
    rawTextRequestId.current += 1;
    setState({ status: "closed" });
  }, []);

  const selectDocument = useCallback(async (document: DataRoomTreeNode) => {
    rawTextRequestId.current += 1;
    const requestId = ++previewRequestId.current;
    setState({
      document,
      preview: { status: "loading" },
      rawText: { status: "idle" },
      status: "open",
    });

    if (!document.storedFileId && !document.relativePath) {
      setState((current) => current.status === "open" && current.document.id === document.id
        ? { ...current, preview: { message: "This selection is not a previewable file.", status: "error" } }
        : current);
      return;
    }

    try {
      const response = document.storedFileId
        ? await runtime.api.getDealDocumentPdf(dealId, document.storedFileId).then((pdf) => ({
            fileName: document.name,
            mimeType: pdf.mimeType,
            pdfBytes: pdf.bytes,
            sourceKind: "stored" as const,
          }))
        : await runtime.api.previewDealDocument(dealId, document.relativePath!);
      if (!isDocumentPreviewResponse(response)) {
        throw new Error("The preview backend returned an invalid PDF response.");
      }
      if (previewRequestId.current === requestId) {
        setState((current) => current.status === "open" && current.document.id === document.id
          ? { ...current, preview: { response, status: "ready" } }
          : current);
      }
    } catch (error) {
      if (previewRequestId.current === requestId) {
        setState((current) => current.status === "open" && current.document.id === document.id
          ? { ...current, preview: { message: toErrorMessage(error), status: "error" } }
          : current);
      }
    }
  }, [dealId]);

  const requestRawText = useCallback(async () => {
    if (state.status !== "open" || !state.document.storedFileId) {
      if (state.status === "open") {
        setState({ ...state, rawText: { message: "Raw text is unavailable for this file.", status: "error" } });
      }
      return;
    }

    const documentId = state.document.id;
    const storedFileId = state.document.storedFileId;
    const requestId = ++rawTextRequestId.current;
    setState({ ...state, rawText: { status: "loading" } });
    try {
      const response = await runtime.api.getDealDocumentText(dealId, storedFileId);
      if (rawTextRequestId.current === requestId) {
        setState((current) => current.status === "open" && current.document.id === documentId
          ? { ...current, rawText: { response, status: "ready" } }
          : current);
      }
    } catch (error) {
      if (rawTextRequestId.current === requestId) {
        setState((current) => current.status === "open" && current.document.id === documentId
          ? { ...current, rawText: { message: toErrorMessage(error), status: "error" } }
          : current);
      }
    }
  }, [dealId, state]);

  return { closeDocument, requestRawText, selectDocument, state };
}

function toErrorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
