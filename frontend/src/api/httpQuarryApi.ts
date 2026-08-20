import type {
  AddUserInput,
  ChunkKeywordSearch,
  ChunkVectorSearch,
  PersistedDeal,
  ProcessDocumentsResponse,
  ProcessFileJobEvent,
  ProcessFileJobEventHandlers,
  ProcessFileJobResponse,
  QuarryApi,
  SummarizableFile,
} from "../contracts/quarryApi";
import type {
  ExtractDealQuestionsInput,
  SaveDealAndExtractInput,
  SaveDealAndExtractResponse,
  SaveDealAndFindFilesResponse,
  SavedDeal,
} from "../data/dealExtraction";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { WorkspaceAccountUser } from "../data/workspace";
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

async function requestJson<TResponse>(path: string, init?: RequestInit, requestDetails?: unknown) {
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
        details: body,
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    finishApiRequest(requestId, {
      details: body,
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

async function getJson<TResponse>(path: string): Promise<TResponse> {
  return requestJson<TResponse>(path);
}

async function postJson<TResponse, TPayload>(
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

function createDeal(input: SaveDealAndExtractInput) {
  return postJson<SaveDealAndFindFilesResponse, SaveDealAndExtractInput>("/api/v1/deals", input);
}

function createDealFromUpload(input: SaveDealAndExtractInput, files: File[]) {
  const form = new FormData();
  form.append("input", JSON.stringify(input));
  appendFiles(form, files);
  return postForm<SaveDealAndFindFilesResponse>("/api/v1/deals/upload", form);
}

function extractDealQuestions(input: ExtractDealQuestionsInput) {
  return postJson<SaveDealAndExtractResponse, ExtractDealQuestionsInput>(
    `/api/v1/deals/${encodeURIComponent(String(input.dealId))}/extraction`,
    input,
  );
}

function extractDealQuestionsFromUpload(dealId: number, files: File[]) {
  const form = new FormData();
  appendFiles(form, files);
  return postForm<SaveDealAndExtractResponse>(`/api/v1/deals/${dealId}/extraction/upload`, form);
}

function listDeals() {
  return getJson<PersistedDeal[]>("/api/v1/deals");
}

function getDeal(dealId: string | number) {
  return getJson<PersistedDeal>(`/api/v1/deals/${encodeURIComponent(String(dealId))}`);
}

function archiveDeal(dealId: string | number) {
  return postJson<SavedDeal, Record<string, never>>(
    `/api/v1/deals/${encodeURIComponent(String(dealId))}/archive`,
    {},
  );
}

function processDocuments(userId: string, files: File[]) {
  const form = new FormData();
  form.append("userId", userId.trim());
  appendFiles(form, files);
  return postForm<ProcessDocumentsResponse>("/api/v1/documents/process", form);
}

async function startProcessFile(userId: string, file: File) {
  const bytes = await file.arrayBuffer();
  const byteFile = new File([bytes], file.name, {
    lastModified: file.lastModified,
    type: file.type,
  });
  const form = new FormData();
  form.append("userId", userId.trim());
  form.append("files", byteFile, file.name);
  return postForm<ProcessFileJobResponse>("/api/v1/documents/process_file", form);
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

function searchDocumentChunksByVector(search: ChunkVectorSearch) {
  return postJson<unknown, ChunkVectorSearch>("/api/v1/documents/search/vector", search);
}

function searchDocumentChunksByKeyword(search: ChunkKeywordSearch) {
  return postJson<unknown, ChunkKeywordSearch>("/api/v1/documents/search/keyword", search);
}

function createUser(input: AddUserInput) {
  return postJson<WorkspaceAccountUser, AddUserInput>("/api/v1/users", input);
}

async function getUserByEmail(email: string) {
  try {
    return await getJson<WorkspaceAccountUser>(
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
  return postJson<SummarizableFile[], { path: string }>("/api/v1/summarize/files", { path });
}

async function summarizeSelected(paths: string[]) {
  const response = await postJson<{ summary: string }, { paths: string[] }>(
    "/api/v1/summarize/selected",
    { paths },
  );
  return response.summary;
}

async function summarizePath(path: string) {
  const response = await postJson<{ summary: string }, { path: string }>("/api/v1/summarize", {
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
  return getJson<DealDataRoom>(`/api/v1/deals/${encodeURIComponent(dealId)}/data-room`);
}

function previewDealDocument(dealId: string, relativePath: string) {
  return postJson<DocumentPreviewResponse, { relativePath: string }>(
    `/api/v1/deals/${encodeURIComponent(dealId)}/data-room/preview`,
    { relativePath },
  );
}

function appendFiles(form: FormData, files: File[]) {
  for (const file of files) {
    const relativeFile = file as File & { webkitRelativePath?: string };
    form.append("files", file, relativeFile.webkitRelativePath || file.name);
  }
}

export const httpQuarryApi: QuarryApi = {
  archiveDeal,
  createDeal,
  createDealFromUpload,
  createUser,
  extractDealQuestions,
  extractDealQuestionsFromUpload,
  getDeal,
  getUserByEmail,
  listDealDataRoom,
  listDeals,
  listSummaryFiles,
  previewDealDocument,
  processDocuments,
  searchDocumentChunksByKeyword,
  searchDocumentChunksByVector,
  startProcessFile,
  subscribeToProcessFileJob,
  summarizePath,
  summarizeSelected,
  summarizeUpload,
  userExistsByEmail,
};
