import { sessionStore } from "../../auth/sessionStore";
import { authFetch } from "../authFetch";
import type {
  HybridDocumentResponse,
  DealDocumentPdf,
  DealDocumentSummary,
  DealDocumentText,
  FileChunkKeywordSearch,
  FileChunkVectorSearch,
  KeywordFileChunkHit,
  ProcessDocumentsResponse,
  ProcessFileJobEvent,
  ProcessFileJobEventHandlers,
  ProcessFileJobResponse,
  QuarryApi,
  VectorFileChunkHit,
} from "../../contracts/quarryApi";
import type {
  DealDataRoom,
  DocumentPreviewResponse,
} from "../../data/dataRoomPreview";
import { beginApiRequest, finishApiRequest } from "../../lib/activityLog";

import {
  get,
  post,
  postForm,
  appendFiles,
  apiUrl,
  BackendApiError,
  requestJson,
} from "./transport";

async function requestPdfBytes(path: string): Promise<DealDocumentPdf> {
  const url = apiUrl(path);
  const requestId = beginApiRequest({ method: "GET", url });
  const startedAt = performance.now();

  try {
    const response = await authFetch(url);
    if (!response.ok) {
      const bodyText = await response.text();
      let body: { error?: string; message?: string } | undefined;
      try {
        body = JSON.parse(bodyText) as { error?: string; message?: string };
      } catch {
        body = undefined;
      }
      const message =
        body?.error ||
        body?.message ||
        bodyText ||
        response.statusText ||
        `Request failed with status ${response.status}`;
      finishApiRequest(requestId, {
        details: body ?? bodyText,
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    const mimeType = response.headers
      .get("content-type")
      ?.split(";", 1)[0]
      .trim();
    if (mimeType !== "application/pdf") {
      throw new Error(
        `The preview backend returned ${mimeType || "an unknown content type"}.`,
      );
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    finishApiRequest(requestId, {
      details: { byteLength: bytes.byteLength, mimeType },
      durationMs: performance.now() - startedAt,
      httpStatus: response.status,
      status: "success",
    });
    return { bytes, mimeType };
  } catch (error) {
    if (!(error instanceof BackendApiError)) {
      finishApiRequest(requestId, {
        details: error,
        durationMs: performance.now() - startedAt,
        message:
          error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

function listDealDocuments(dealId: string) {
  return get<DealDocumentSummary[]>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents`,
  );
}

function getDealDocumentPdf(dealId: string, fileId: string) {
  return requestPdfBytes(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents/${encodeURIComponent(fileId)}/pdf`,
  );
}

function getDealDocumentText(dealId: string, fileId: string) {
  return get<DealDocumentText>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents/${encodeURIComponent(fileId)}/text`,
  );
}

function processDocuments(dealId: string, files: File[]) {
  const form = new FormData();
  appendFiles(form, files);
  return postForm<ProcessDocumentsResponse>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents/process`,
    form,
  );
}

async function startProcessFile(dealId: string, file: File) {
  const generation = sessionStore.getSnapshot().generation;
  const bytes = await file.arrayBuffer();
  if (generation !== sessionStore.getSnapshot().generation)
    throw new DOMException("Session changed", "AbortError");
  const byteFile = new File([bytes], file.name, {
    lastModified: file.lastModified,
    type: file.type,
  });
  const form = new FormData();
  form.append("files", byteFile, file.name);
  return postForm<ProcessFileJobResponse>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents/process_file`,
    form,
  );
}

function subscribeToProcessFileJob(
  jobId: string,
  { onConnectionError, onEvent }: ProcessFileJobEventHandlers,
) {
  const url = apiUrl(
    `/api/v1/documents/process_file/${encodeURIComponent(jobId)}/events`,
  );
  const controller = new AbortController();
  let active = true;
  let terminal = false;
  void (async () => {
    const response = await authFetch(url, {
      headers: { Accept: "text/event-stream" },
      signal: controller.signal,
    });
    if (
      !response.ok ||
      !response.body ||
      !response.headers.get("content-type")?.startsWith("text/event-stream")
    )
      throw new Error("Unable to connect to processing stream.");
    const reader = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let buffer = "";
    try {
      while (active && !terminal) {
        const chunk = await reader.read();
        if (chunk.done) {
          buffer += decoder.decode();
          break;
        }
        buffer += decoder.decode(chunk.value, { stream: true });
        if (buffer.length > 1_048_576)
          throw new Error("Processing event exceeds size limit.");
        let boundary: RegExpExecArray | null;
        while ((boundary = /\r?\n\r?\n/.exec(buffer))) {
          const frame = buffer.slice(0, boundary.index);
          buffer = buffer.slice(boundary.index + boundary[0].length);
          const lines = frame.split(/\r?\n/);
          const name = lines
            .find((line) => line.startsWith("event:"))
            ?.slice(6)
            .trim();
          const data = lines
            .filter((line) => line.startsWith("data:"))
            .map((line) => line.slice(5).trimStart())
            .join("\n");
          if (!data && !name) continue;
          if (
            !name ||
            !["processing", "completed", "skipped", "failed"].includes(name)
          )
            throw new Error("Invalid processing event.");
          const parsed: unknown = JSON.parse(data);
          if (
            typeof parsed !== "object" ||
            !parsed ||
            !("jobId" in parsed) ||
            parsed.jobId !== jobId ||
            !("status" in parsed) ||
            parsed.status !== name ||
            !("filename" in parsed) ||
            typeof parsed.filename !== "string"
          )
            throw new Error("Invalid processing event.");
          if (active) onEvent(parsed as ProcessFileJobEvent);
          terminal = name !== "processing";
          if (terminal) break;
        }
      }
      if (!terminal && active)
        throw new Error("Processing stream ended unexpectedly.");
    } finally {
      await reader.cancel();
    }
  })().catch(() => {
    if (active && !terminal && !controller.signal.aborted)
      onConnectionError?.();
  });
  return () => {
    active = false;
    controller.abort();
  };
}

function searchDocumentChunksByVector(search: FileChunkVectorSearch) {
  return post<VectorFileChunkHit[], FileChunkVectorSearch>(
    "/api/v1/documents/search/vector",
    search,
  );
}

function searchDocumentChunksByKeyword(search: FileChunkKeywordSearch) {
  return post<KeywordFileChunkHit[], FileChunkKeywordSearch>(
    "/api/v1/documents/search/keyword",
    search,
  );
}

function listDealDataRoom(dealId: string) {
  return get<DealDataRoom>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/data-room`,
  );
}

function previewDealDocument(dealId: string, relativePath: string) {
  return post<DocumentPreviewResponse, { relativePath: string }>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/data-room/preview`,
    { relativePath },
  );
}

export const httpDocumentsApi: Pick<
  QuarryApi,
  | "listDealDocuments"
  | "getDealDocumentPdf"
  | "getDealDocumentText"
  | "processDocuments"
  | "startProcessFile"
  | "subscribeToProcessFileJob"
  | "searchDocumentChunksByVector"
  | "searchDocumentChunksByKeyword"
  | "listDealDataRoom"
  | "previewDealDocument"
  | "searchDealDocumentChunksHybrid"
  | "searchDocumentChunksHybrid"
  | "searchDealDocumentChunksByKeyword"
  | "searchDealDocumentChunksByVector"
> = {
  listDealDocuments,
  getDealDocumentPdf,
  getDealDocumentText,
  processDocuments,
  startProcessFile,
  subscribeToProcessFileJob,
  searchDocumentChunksByVector,
  searchDocumentChunksByKeyword,
  listDealDataRoom,
  previewDealDocument,
  searchDealDocumentChunksHybrid: (d, body, signal) => requestJson<HybridDocumentResponse>(
    `/api/v1/deals/${encodeURIComponent(d)}/documents/search/hybrid`,
    { method: "POST", body: JSON.stringify(body), headers: { "Content-Type": "application/json" }, signal },
    undefined, false,
  ),
  searchDocumentChunksHybrid: (body, signal) => requestJson<HybridDocumentResponse>(
    "/api/v1/documents/search/hybrid",
    { method: "POST", body: JSON.stringify(body), headers: { "Content-Type": "application/json" }, signal },
    undefined, false,
  ),
  searchDealDocumentChunksByKeyword: (d, body) =>
    requestJson(
      `/api/v1/deals/${encodeURIComponent(d)}/documents/search/keyword`,
      {
        method: "POST",
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" },
      },
      undefined,
      false,
    ),
  searchDealDocumentChunksByVector: (d, body) =>
    requestJson(
      `/api/v1/deals/${encodeURIComponent(d)}/documents/search/vector`,
      {
        method: "POST",
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" },
      },
      undefined,
      false,
    ),
};
