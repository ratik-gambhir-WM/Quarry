import { sessionStore } from "../auth/sessionStore";
import { authFetch } from "./authFetch";
import type { Company, CompanyInput, CompanyPage, CompanySearch, DealFields } from "../data/dealModel";
import type { AssistantThreadListOptions } from "../contracts/quarryApi";
import type {
  LoginInput, RegisterInput, SessionResponse, CurrentUserResponse,
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
    const response = await authFetch(url, init);
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
    const response = await authFetch(url, {
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

function createCompany(input: CompanyInput) { return post<Company, CompanyInput>("/api/v1/companies", input); }
function updateCompany(id: string, input: CompanyInput) { return post<Company, CompanyInput>(`/api/v1/companies/${encodeURIComponent(id)}/update`, input); }
function listCompanies(options: CompanySearch = {}) {
  const params = new URLSearchParams();
  if (options.query !== undefined) params.set("query", options.query);
  if (options.limit !== undefined) params.set("limit", String(options.limit));
  if (options.offset !== undefined) params.set("offset", String(options.offset));
  return get<CompanyPage>(`/api/v1/companies?${params}`);
}
function updateDeal(id: string, input: DealFields) { return post<SaveDealResponse, DealFields>(`/api/v1/deals/${encodeURIComponent(id)}/update`, input); }

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
  if (generation !== sessionStore.getSnapshot().generation) throw new DOMException("Session changed", "AbortError");
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
  const url = apiUrl(`/api/v1/documents/process_file/${encodeURIComponent(jobId)}/events`);
  const controller = new AbortController();
  let active = true;
  let terminal = false;
  void (async () => {
    const response = await authFetch(url, { headers: { Accept: "text/event-stream" }, signal: controller.signal });
    if (!response.ok || !response.body || !response.headers.get("content-type")?.startsWith("text/event-stream")) throw new Error("Unable to connect to processing stream.");
    const reader = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let buffer = "";
    try {
      while (active && !terminal) {
        const chunk = await reader.read();
        if (chunk.done) { buffer += decoder.decode(); break; }
        buffer += decoder.decode(chunk.value, { stream: true });
        if (buffer.length > 1_048_576) throw new Error("Processing event exceeds size limit.");
        let boundary: RegExpExecArray | null;
        while ((boundary = /\r?\n\r?\n/.exec(buffer))) {
          const frame = buffer.slice(0, boundary.index); buffer = buffer.slice(boundary.index + boundary[0].length);
          const lines = frame.split(/\r?\n/);
          const name = lines.find((line) => line.startsWith("event:"))?.slice(6).trim();
          const data = lines.filter((line) => line.startsWith("data:")).map((line) => line.slice(5).trimStart()).join("\n");
          if (!data && !name) continue;
          if (!name || !["processing", "completed", "skipped", "failed"].includes(name)) throw new Error("Invalid processing event.");
          const parsed: unknown = JSON.parse(data);
          if (typeof parsed !== "object" || !parsed || !("jobId" in parsed) || parsed.jobId !== jobId || !("status" in parsed) || parsed.status !== name || !("filename" in parsed) || typeof parsed.filename !== "string") throw new Error("Invalid processing event.");
          if (active) onEvent(parsed as ProcessFileJobEvent);
          terminal = name !== "processing";
          if (terminal) break;
        }
      }
      if (!terminal && active) throw new Error("Processing stream ended unexpectedly.");
    } finally { await reader.cancel(); }
  })().catch(() => { if (active && !terminal && !controller.signal.aborted) onConnectionError?.(); });
  return () => { active = false; controller.abort(); };

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
  console.info("[chat] http.stream.started", { path, requestId, requestDetails });
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
    console.error("[chat] http.stream.failed", { httpStatus, message, path, requestId });
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
      const response = await authFetch(url, {
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
      console.info("[chat] http.stream.connected", { contentType, path, requestId, status: response.status });
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
        console.info("[chat] http.sse.event", {
          event: event.type,
          path,
          requestId,
          valueCharacters: event.type === "delta"
            ? [...event.delta].length
            : event.type === "completed" ? [...event.response].length : undefined,
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
      console.error("[chat] http.stream.exception", { error, path, requestId });
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
  options: AssistantThreadListOptions = {},
) {
  const parameters = new URLSearchParams();
  if (options.after) parameters.set("before", options.after);
  if (options.archived) parameters.set("archived", "true");
  if (options.scope) parameters.set("scope", options.scope);
  if (options.dealId) parameters.set("dealId", options.dealId);
  return get<AssistantThreadPage>(`/api/v1/assistant/threads?${parameters.toString()}`);
}

function createAssistantThread(threadId?: string, dealId?: string | null) {
  return post<AssistantThread, { threadId?: string; dealId?: string | null }>(
    "/api/v1/assistant/threads",
    { ...(threadId ? { threadId } : {}), ...(dealId !== undefined ? { dealId } : {}) },
  );
}

function getAssistantThread(threadId: string) {
  return get<AssistantThreadDetail>(
    `${assistantThreadPath(threadId)}`,
  );
}

function renameAssistantThread(threadId: string, title: string) {
  return post<void, { title: string }>(
    assistantThreadPath(threadId, "/rename"),
    { title },
  );
}

function setAssistantThreadArchived(threadId: string, archived: boolean) {
  return post<void, Record<string, never>>(
    assistantThreadPath(threadId, archived ? "/archive" : "/unarchive"),
    {},
  );
}

function deleteAssistantThread(threadId: string) {
  return requestJson<void>(
    `${assistantThreadPath(threadId)}`,
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
  archiveAssistantThread: (threadId) =>
    setAssistantThreadArchived(threadId, true),
  archiveDeal,
  createAssistantThread,
  createDeal,
  updateDeal,
  createCompany,
  updateCompany,
  listCompanies,
  login: (input: LoginInput) => requestJson<SessionResponse>("/api/v1/auth/login", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(input) }, undefined, false),
  register: (input: RegisterInput) => requestJson<SessionResponse>("/api/v1/auth/register", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(input) }, undefined, false),
  currentUser: () => requestJson<CurrentUserResponse>("/api/v1/auth/me", undefined, undefined, false),
  logout: () => requestJson<void>("/api/v1/auth/logout", { method: "POST" }, undefined, false),
  deleteAssistantThread,
  deleteTemplate,
  exportPowerPoint,
  getDeal,
  getAssistantThread,
  getDealDocumentPdf,
  getDealDocumentText,
  getTemplate,
  importPptxTemplate,
  listDealDataRoom,
  listAssistantThreads,
  listDealDocuments,
  listDeals,
  listTemplatePreviews,
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
  summarizeUpload,
  unarchiveAssistantThread: (threadId) =>
    setAssistantThreadArchived(threadId, false),
};
