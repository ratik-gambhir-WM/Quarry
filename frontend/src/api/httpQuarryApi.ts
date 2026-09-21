import type {
  AddUserInput,
  AssistantThread,
  AssistantThreadDetail,
  AssistantThreadPage,
  DealDocumentPdf,
  DealDocumentSummary,
  DealDocumentText,
  FileChunkKeywordSearch,
  FileChunkVectorSearch,
  KeywordFileChunkHit,
  PersistedDeal,
  ProcessDocumentsResponse,
  ProcessFileJobEvent,
  ProcessFileJobEventHandlers,
  ProcessFileJobResponse,
  PowerPointExport,
  QueryModelInput,
  QuarryApi,
  RunAssistantThreadInput,
  SendQueryEventHandlers,
  SummarizableFile,
  PptxTemplateImportMode,
  PptxTemplateImportResult,
  TemplatePreviewPage,
  VectorFileChunkHit,
} from "../contracts/quarryApi";
import { QuerySseParser } from "./querySse";
import { POWERPOINT_CONTENT_TYPE } from "../contracts/quarryApi";
import type { DiligenceCanvasDocument } from "../contracts/diligenceCanvas";
import type {
  SaveDealInput,
  SaveDealMetadataInput,
  SaveDealMetadataResponse,
  SaveDealResponse,
  SavedDeal,
} from "../data/dealExtraction";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { WorkspaceAccountUser } from "../data/workspace";
import { parseDiligenceCanvasDocument } from "../contracts/diligenceCanvas";
import {
  beginApiRequest,
  finishApiRequest,
  logSseEvent,
  summarizeFormData,
} from "../lib/activityLog";

const API_BASE_URL = (import.meta.env.VITE_API_BASE_URL ?? "").replace(/\/$/, "");

validateApiBaseUrl(API_BASE_URL);

export class BackendApiError extends Error {
  status: number;

  constructor(message: string, status: number) {
    super(message);
    this.name = "BackendApiError";
    this.status = status;
  }
}

function apiUrl(path: string) {
  if (import.meta.env.MODE === "desktop" && !import.meta.env.DEV && !API_BASE_URL) {
    throw new Error("VITE_API_BASE_URL is required for packaged desktop builds.");
  }

  return `${API_BASE_URL}${path}`;
}

function validateApiBaseUrl(baseUrl: string) {
  if (!baseUrl) {
    return;
  }

  let parsed: URL;
  try {
    parsed = new URL(baseUrl);
  } catch {
    throw new Error("VITE_API_BASE_URL must be an absolute URL.");
  }
  const localDevelopmentUrl = import.meta.env.DEV
    && parsed.protocol === "http:"
    && (parsed.hostname === "localhost" || parsed.hostname === "127.0.0.1");
  if (parsed.protocol !== "https:" && !localDevelopmentUrl) {
    throw new Error("VITE_API_BASE_URL must use HTTPS outside local development.");
  }
}

async function requestJson<TResponse>(
  path: string,
  init?: RequestInit,
  requestDetails?: unknown,
  logResponseBody = true,
) {
  const url = apiUrl(path);
  const method = init?.method ?? "GET";
  const requestId = beginApiRequest({ method, request: requestDetails, url });
  const startedAt = performance.now();

  try {
    const response = await fetch(url, init);
    const bodyText = response.status === 204 ? "" : await response.text();
    let body: unknown;

    if (bodyText) {
      try {
        body = JSON.parse(bodyText);
      } catch {
        body = bodyText;
      }
    }

    if (!response.ok) {
      const errorBody = body as { error?: string; message?: string } | undefined;
      const message =
        (typeof errorBody === "object" ? errorBody?.error ?? errorBody?.message : bodyText) ||
        response.statusText ||
        `Request failed with status ${response.status}`;

      finishApiRequest(requestId, {
        details: logResponseBody ? body : undefined,
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    finishApiRequest(requestId, {
      details: logResponseBody ? body : undefined,
      durationMs: performance.now() - startedAt,
      httpStatus: response.status,
      status: "success",
    });
    return body as TResponse;
  } catch (error) {
    if (!(error instanceof BackendApiError)) {
      finishApiRequest(requestId, {
        details: error,
        durationMs: performance.now() - startedAt,
        message: error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

async function requestPdfBytes(path: string): Promise<DealDocumentPdf> {
  const url = apiUrl(path);
  const requestId = beginApiRequest({ method: "GET", url });
  const startedAt = performance.now();

  try {
    const response = await fetch(url);
    if (!response.ok) {
      const bodyText = await response.text();
      let body: { error?: string; message?: string } | undefined;
      try {
        body = JSON.parse(bodyText) as { error?: string; message?: string };
      } catch {
        body = undefined;
      }
      const message =
        body?.error || body?.message || bodyText || response.statusText ||
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

    const mimeType = response.headers.get("content-type")?.split(";", 1)[0].trim();
    if (mimeType !== "application/pdf") {
      throw new Error(`The preview backend returned ${mimeType || "an unknown content type"}.`);
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
        message: error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

const MAX_POWERPOINT_EXPORT_BYTES = 64 * 1024 * 1024;

async function exportPowerPoint(
  document: DiligenceCanvasDocument,
): Promise<PowerPointExport> {
  const path = "/api/v1/templates/export";
  const url = apiUrl(path);
  const requestId = beginApiRequest({
    method: "POST",
    request: {
      slideCount: document.presentation.slides.length,
    },
    url,
  });
  const startedAt = performance.now();

  try {
    const response = await fetch(url, {
      body: JSON.stringify(document),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
    if (!response.ok) {
      const bodyText = await response.text();
      let body: { error?: string; message?: string } | undefined;
      try {
        body = JSON.parse(bodyText) as { error?: string; message?: string };
      } catch {
        body = undefined;
      }
      const message = body?.error || body?.message || bodyText || response.statusText
        || `Request failed with status ${response.status}`;
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    const mimeType = response.headers.get("content-type")?.split(";", 1)[0].trim();
    if (mimeType !== POWERPOINT_CONTENT_TYPE) {
      throw new Error("The export backend returned an invalid PowerPoint content type.");
    }
    const fileName = parsePowerPointFileName(response.headers.get("content-disposition"));
    const warningCount = parsePowerPointWarningCount(
      response.headers.get("x-powerpoint-warning-count"),
    );
    const declaredLength = response.headers.get("content-length");
    if (declaredLength) {
      const length = Number(declaredLength);
      if (!/^\d+$/.test(declaredLength) || !Number.isSafeInteger(length)) {
        throw new Error("The export backend returned an invalid PowerPoint size.");
      }
      if (length > MAX_POWERPOINT_EXPORT_BYTES) {
        throw new Error("The export backend returned an oversized PowerPoint file.");
      }
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (
      bytes.byteLength === 0
      || bytes.byteLength > MAX_POWERPOINT_EXPORT_BYTES
      || bytes[0] !== 0x50
      || bytes[1] !== 0x4b
      || bytes[2] !== 0x03
      || bytes[3] !== 0x04
    ) {
      throw new Error("The export backend returned invalid or oversized PowerPoint bytes.");
    }
    const dataBase64 = bytesToBase64(bytes);
    finishApiRequest(requestId, {
      details: { byteLength: bytes.byteLength, warningCount },
      durationMs: performance.now() - startedAt,
      httpStatus: response.status,
      status: "success",
    });
    return { dataBase64, fileName, mimeType: POWERPOINT_CONTENT_TYPE, warningCount };
  } catch (error) {
    if (!(error instanceof BackendApiError)) {
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        message: error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

function parsePowerPointFileName(value: string | null) {
  const match = value?.match(/^attachment; filename="([^"\\/\r\n]+\.pptx)"$/i);
  if (!match || match[1].length > 255 || match[1].trim() !== match[1]) {
    throw new Error("The export backend returned an invalid PowerPoint filename.");
  }
  return match[1];
}

function parsePowerPointWarningCount(value: string | null) {
  if (!value || !/^\d+$/.test(value)) {
    throw new Error("The export backend returned an invalid warning count.");
  }
  const count = Number(value);
  if (!Number.isSafeInteger(count) || count > 10_000) {
    throw new Error("The export backend returned an invalid warning count.");
  }
  return count;
}

function bytesToBase64(bytes: Uint8Array) {
  let binary = "";
  const chunkSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

async function get<TResponse>(path: string): Promise<TResponse> {
  return requestJson<TResponse>(path);
}

async function post<TResponse, TPayload>(
  path: string,
  payload: TPayload,
): Promise<TResponse> {
  return requestJson<TResponse>(path, {
    body: JSON.stringify(payload),
    headers: { "Content-Type": "application/json" },
    method: "POST",
  }, payload);
}

async function postForm<TResponse>(path: string, formData: FormData): Promise<TResponse> {
  return requestJson<TResponse>(path, {
    body: formData,
    method: "POST",
  }, summarizeFormData(formData));
}

function createDeal(input: SaveDealInput) {
  return post<SaveDealResponse, SaveDealInput>("/api/v1/deals", input);
}

function saveDealMetadata(dealId: string, input: SaveDealMetadataInput) {
  const form = new FormData();
  appendFiles(form, input.files);
  appendDealMetadataLinks(form, input);
  return postForm<SaveDealMetadataResponse>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/metadata`,
    form,
  );
}

function appendDealMetadataLinks(form: FormData, input: SaveDealMetadataInput) {
  form.append("sharepointLink", input.sharepointLink ?? "");
  form.append("sowLink", input.sowLink ?? "");
  form.append("factSheetLink", input.factSheetLink ?? "");
  form.append("rlLink", input.rlLink ?? "");
}

function listDeals() {
  return get<PersistedDeal[]>("/api/v1/deals");
}

function listTemplatePreviews(page: number) {
  return get<TemplatePreviewPage>(
    `/api/v1/templates/previews?page=${encodeURIComponent(String(page))}`,
  );
}

async function getTemplate(templateId: string) {
  const value = await requestJson<unknown>(
    `/api/v1/templates/${encodeURIComponent(templateId)}`,
    undefined,
    undefined,
    false,
  );
  return parseDiligenceCanvasDocument(value);
}

function importPptxTemplate(file: File, importMode: PptxTemplateImportMode) {
  const form = new FormData();
  form.append("files", file, file.name);
  return postForm<PptxTemplateImportResult>(
    `/api/v1/templates/import?mode=${encodeURIComponent(importMode)}`,
    form,
  );
}

async function deleteTemplate(templateId: string) {
  await requestJson<void>(
    `/api/v1/templates/${encodeURIComponent(templateId)}`,
    { method: "DELETE" },
  );
}

function getDeal(dealId: string) {
  return get<PersistedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}`);
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

function archiveDeal(dealId: string) {
  return post<SavedDeal, Record<string, never>>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/archive`,
    {},
  );
}

function processDocuments(dealId: string, userId: string, files: File[]) {
  const form = new FormData();
  form.append("userId", userId.trim());
  appendFiles(form, files);
  return postForm<ProcessDocumentsResponse>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/documents/process`,
    form,
  );
}

async function startProcessFile(dealId: string, userId: string, file: File) {
  const bytes = await file.arrayBuffer();
  const byteFile = new File([bytes], file.name, {
    lastModified: file.lastModified,
    type: file.type,
  });
  const form = new FormData();
  form.append("userId", userId.trim());
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
  const url = apiUrl(`/api/v1/documents/process_file/${encodeURIComponent(jobId)}/events`);
  const source = new EventSource(url);
  const eventNames = ["processing", "completed", "skipped", "failed"] as const;

  const handleJobEvent = (event: MessageEvent<string>) => {
    try {
      const parsed = JSON.parse(event.data) as ProcessFileJobEvent;
      logSseEvent({
        data: parsed,
        eventName: event.type,
        status: event.type === "failed" ? "error" : "success",
        title: `${event.type} event received for ${parsed.filename || jobId}`,
        url,
      });
      onEvent(parsed);
    } catch {
      logSseEvent({
        data: event.data,
        eventName: event.type,
        status: "error",
        title: "Invalid SSE event received",
        url,
      });
      onEvent({
        error: "The processing stream returned an invalid event.",
        filename: "",
        jobId,
        status: "failed",
      });
    }
  };

  for (const eventName of eventNames) {
    source.addEventListener(eventName, handleJobEvent as EventListener);
  }
  const handleOpen = () => {
    logSseEvent({ eventName: "open", status: "success", title: `SSE connected for job ${jobId}`, url });
  };
  const handleError = () => {
    logSseEvent({
      data: { readyState: source.readyState },
      eventName: "error",
      status: "error",
      title: `SSE connection error for job ${jobId}`,
      url,
    });
    onConnectionError?.();
  };
  source.addEventListener("open", handleOpen);
  source.addEventListener("error", handleError);

  return () => {
    for (const eventName of eventNames) {
      source.removeEventListener(eventName, handleJobEvent as EventListener);
    }
    source.removeEventListener("open", handleOpen);
    source.removeEventListener("error", handleError);
    source.close();
    logSseEvent({ eventName: "close", status: "info", title: `SSE closed for job ${jobId}`, url });
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

function createUser(input: AddUserInput) {
  return post<WorkspaceAccountUser, AddUserInput>("/api/v1/users", input);
}

async function getUserByEmail(email: string) {
  try {
    return await get<WorkspaceAccountUser>(
      `/api/v1/users/by-email?email=${encodeURIComponent(email.trim())}`,
    );
  } catch (error) {
    if (error instanceof BackendApiError && error.status === 404) {
      return null;
    }
    throw error;
  }
}

async function userExistsByEmail(email: string) {
  return (await getUserByEmail(email)) !== null;
}

function listSummaryFiles(path: string) {
  return post<SummarizableFile[], { path: string }>("/api/v1/summarize/files", { path });
}

async function summarizeSelected(paths: string[]) {
  const response = await post<{ summary: string }, { paths: string[] }>(
    "/api/v1/summarize/selected",
    { paths },
  );
  return response.summary;
}

async function summarizePath(path: string) {
  const response = await post<{ summary: string }, { path: string }>("/api/v1/summarize", {
    path,
  });
  return response.summary;
}

function summarizeUpload(files: File[]) {
  const form = new FormData();
  appendFiles(form, files);
  return postForm<{ summary: string }>("/api/v1/summarize/upload", form).then(
    (response) => response.summary,
  );
}

function listDealDataRoom(dealId: string) {
  return get<DealDataRoom>(`/api/v1/deals/${encodeURIComponent(dealId)}/data-room`);
}

function previewDealDocument(dealId: string, relativePath: string) {
  return post<DocumentPreviewResponse, { relativePath: string }>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/data-room/preview`,
    { relativePath },
  );
}

function queryModel(input: QueryModelInput, handlers: SendQueryEventHandlers) {
  const form = new FormData();
  form.append("prompt", input.prompt);
  form.append("context", JSON.stringify(input.context));
  if (input.model !== undefined) form.append("model", input.model);
  if (input.systemInstructions !== undefined) {
    form.append("systemInstructions", input.systemInstructions);
  }
  for (const file of input.files) form.append("files", file, file.name);
  return streamQuery("/api/v1/query_model", form, {
    contextMessageCount: input.context.length,
    fileCount: input.files.length,
    fileBytes: input.files.reduce((total, file) => total + file.size, 0),
    model: input.model,
  }, handlers);
}

function runAssistantThread(
  input: RunAssistantThreadInput,
  handlers: SendQueryEventHandlers,
) {
  const form = new FormData();
  form.append("prompt", input.prompt);
  form.append("userEmail", input.userEmail);
  form.append("userMessageId", input.userMessageId);
  form.append("assistantMessageId", input.assistantMessageId);
  form.append("requestId", input.requestId);
  if (input.parentMessageId !== undefined) form.append("parentMessageId", input.parentMessageId);
  if (input.model !== undefined) form.append("model", input.model);
  if (input.systemInstructions !== undefined) {
    form.append("systemInstructions", input.systemInstructions);
  }
  for (const file of input.files) form.append("files", file, file.name);
  return streamQuery(
    `/api/v1/assistant/threads/${encodeURIComponent(input.threadId)}/runs`,
    form,
    {
      fileCount: input.files.length,
      fileBytes: input.files.reduce((total, file) => total + file.size, 0),
      model: input.model,
      threadId: input.threadId,
    },
    handlers,
  );
}

function streamQuery(
  path: string,
  form: FormData,
  requestDetails: Record<string, unknown>,
  { onConnectionError, onEvent }: SendQueryEventHandlers,
) {
  const url = apiUrl(path);
  const controller = new AbortController();
  const startedAt = performance.now();
  const requestId = beginApiRequest({
    method: "POST",
    request: requestDetails,
    url,
  });
  let active = true;
  let settled = false;
  let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;

  const stopStream = () => {
    controller.abort();
    void reader?.cancel().catch(() => undefined);
  };

  const failConnection = (message: string, httpStatus?: number) => {
    if (!active || settled) return;
    settled = true;
    active = false;
    stopStream();
    finishApiRequest(requestId, {
      durationMs: performance.now() - startedAt,
      httpStatus,
      message,
      status: "error",
    });
    onConnectionError?.(message);
  };

  void (async () => {
    try {
      const response = await fetch(url, {
        body: form,
        headers: { Accept: "text/event-stream" },
        method: "POST",
        signal: controller.signal,
      });
      if (!response.ok) {
        failConnection(`The query request failed with status ${response.status}.`, response.status);
        return;
      }
      const contentType = response.headers.get("content-type")?.split(";", 1)[0].trim();
      if (contentType !== "text/event-stream" || !response.body) {
        failConnection("The query backend returned an invalid streaming response.", response.status);
        return;
      }
      const parser = new QuerySseParser((event) => {
        if (!active || settled) return;
        logSseEvent({
          data: event.type === "delta"
            ? { characterCount: [...event.delta].length }
            : { type: event.type },
          eventName: event.type,
          status: event.type === "failed" ? "error" : "success",
          title: `Query ${event.type} event`,
          url,
        });
        onEvent(event);
        if (event.type === "completed" || event.type === "failed") {
          settled = true;
          active = false;
          stopStream();
          finishApiRequest(requestId, {
            durationMs: performance.now() - startedAt,
            httpStatus: response.status,
            status: event.type === "completed" ? "success" : "error",
          });
        }
      });
      reader = response.body.getReader();
      while (active) {
        const { done, value } = await reader.read();
        if (done) break;
        parser.push(value);
      }
      if (active) parser.finish();
    } catch (error) {
      if (!active && error instanceof DOMException && error.name === "AbortError") return;
      failConnection(error instanceof Error ? error.message : "The query connection failed.");
    }
  })();

  return () => {
    if (!active) return;
    active = false;
    stopStream();
    if (!settled) {
      settled = true;
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        message: "Query cancelled",
        status: "success",
      });
    }
  };
}

function assistantThreadPath(threadId: string, suffix = "") {
  return `/api/v1/assistant/threads/${encodeURIComponent(threadId)}${suffix}`;
}

function listAssistantThreads(
  userEmail: string,
  options: { after?: string; archived?: boolean } = {},
) {
  const parameters = new URLSearchParams({ userEmail });
  if (options.after) parameters.set("before", options.after);
  if (options.archived) parameters.set("archived", "true");
  return get<AssistantThreadPage>(`/api/v1/assistant/threads?${parameters.toString()}`);
}

function createAssistantThread(userEmail: string, threadId?: string) {
  return post<AssistantThread, { threadId?: string; userEmail: string }>(
    "/api/v1/assistant/threads",
    { userEmail, ...(threadId ? { threadId } : {}) },
  );
}

function getAssistantThread(threadId: string, userEmail: string) {
  return get<AssistantThreadDetail>(
    `${assistantThreadPath(threadId)}?userEmail=${encodeURIComponent(userEmail)}`,
  );
}

function renameAssistantThread(threadId: string, userEmail: string, title: string) {
  return post<void, { title: string; userEmail: string }>(
    assistantThreadPath(threadId, "/rename"),
    { title, userEmail },
  );
}

function setAssistantThreadArchived(threadId: string, userEmail: string, archived: boolean) {
  return post<void, { userEmail: string }>(
    assistantThreadPath(threadId, archived ? "/archive" : "/unarchive"),
    { userEmail },
  );
}

function deleteAssistantThread(threadId: string, userEmail: string) {
  return requestJson<void>(
    `${assistantThreadPath(threadId)}?userEmail=${encodeURIComponent(userEmail)}`,
    { method: "DELETE" },
  );
}

function appendFiles(form: FormData, files: File[]) {
  for (const file of files) {
    const relativeFile = file as File & { webkitRelativePath?: string };
    form.append("files", file, relativeFile.webkitRelativePath || file.name);
  }
}

export const httpQuarryApi: QuarryApi = {
  archiveAssistantThread: (threadId, userEmail) =>
    setAssistantThreadArchived(threadId, userEmail, true),
  archiveDeal,
  createAssistantThread,
  createDeal,
  createUser,
  deleteAssistantThread,
  deleteTemplate,
  exportPowerPoint,
  getDeal,
  getAssistantThread,
  getDealDocumentPdf,
  getDealDocumentText,
  getTemplate,
  getUserByEmail,
  importPptxTemplate,
  listDealDataRoom,
  listAssistantThreads,
  listDealDocuments,
  listDeals,
  listTemplatePreviews,
  listSummaryFiles,
  previewDealDocument,
  processDocuments,
  queryModel,
  renameAssistantThread,
  runAssistantThread,
  saveDealMetadata,
  searchDocumentChunksByKeyword,
  searchDocumentChunksByVector,
  startProcessFile,
  subscribeToProcessFileJob,
  summarizePath,
  summarizeSelected,
  summarizeUpload,
  unarchiveAssistantThread: (threadId, userEmail) =>
    setAssistantThreadArchived(threadId, userEmail, false),
  userExistsByEmail,
};
