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
import { parseQueryEvent } from "./querySse";
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

export type TauriMultipartRequest = {
  fields: { name: string; value: string }[];
  files: {
    dataBase64: string;
    fieldName: string;
    filename: string;
    mimeType: string;
  }[];
  path: string;
};

export type TauriQueryRequest = {
  assistantMessageId?: string;
  context?: QueryModelInput["context"];
  files: TauriMultipartRequest["files"];
  model?: string;
  parentMessageId?: string;
  path: string;
  prompt: string;
  requestId?: string;
  systemInstructions?: string;
  userEmail?: string;
  userMessageId?: string;
};

export type TauriQueryPayload =
  | { event: unknown; kind: "serverEvent"; subscriptionId: string }
  | { kind: "connectionError"; message: string; subscriptionId: string };

type TauriTransport = {
  delete(path: string): Promise<void>;
  get<T>(path: string): Promise<T>;
  getPdf(path: string): Promise<ArrayBuffer>;
  post<T>(path: string, body: unknown): Promise<T>;
  postPowerPoint(path: string, body: unknown): Promise<PowerPointExport>;
  postMultipart<T>(request: TauriMultipartRequest): Promise<T>;
  startQuery?(
    request: TauriQueryRequest,
    onPayload: (payload: TauriQueryPayload) => void,
  ): Promise<() => void>;
  subscribeJob(
    jobId: string,
    onEvent: (eventName: string, data: string) => void,
    onError: () => void,
  ): Promise<() => void>;
};

export function createTauriQuarryApi(transport: TauriTransport): QuarryApi {
  async function multipartFiles(
    path: string,
    files: File[],
    fields: TauriMultipartRequest["fields"] = [],
    fallbackMimeType = "application/octet-stream",
  ) {
    return {
      fields,
      files: await Promise.all(files.map((file) => fileToMultipart(file, fallbackMimeType))),
      path,
    } satisfies TauriMultipartRequest;
  }

  return {
    archiveAssistantThread: (threadId, userEmail) =>
      transport.post<void>(
        `/api/v1/assistant/threads/${encodeURIComponent(threadId)}/archive`,
        { userEmail },
      ),
    archiveDeal: (dealId) =>
      transport.post<SavedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}/archive`, {}),
    createAssistantThread: (userEmail, threadId) =>
      transport.post<AssistantThread>("/api/v1/assistant/threads", {
        userEmail,
        ...(threadId ? { threadId } : {}),
      }),
    createDeal: (input: SaveDealInput) =>
      transport.post<SaveDealResponse>("/api/v1/deals", input),
    createUser: (input: AddUserInput) =>
      transport.post<WorkspaceAccountUser>("/api/v1/users", input),
    deleteAssistantThread: (threadId, userEmail) =>
      transport.delete(
        `/api/v1/assistant/threads/${encodeURIComponent(threadId)}?userEmail=${encodeURIComponent(userEmail)}`,
      ),
    deleteTemplate: (templateId) =>
      transport.delete(`/api/v1/templates/${encodeURIComponent(templateId)}`),
    exportPowerPoint: (document) =>
      transport.postPowerPoint("/api/v1/templates/export", document),
    getDeal: (dealId) =>
      transport.get<PersistedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}`),
    getAssistantThread: (threadId, userEmail) =>
      transport.get<AssistantThreadDetail>(
        `/api/v1/assistant/threads/${encodeURIComponent(threadId)}?userEmail=${encodeURIComponent(userEmail)}`,
      ),
    async getDealDocumentPdf(dealId, fileId): Promise<DealDocumentPdf> {
      const bytes = await transport.getPdf(
        `/api/v1/deals/${encodeURIComponent(dealId)}/documents/${encodeURIComponent(fileId)}/pdf`,
      );
      return { bytes: new Uint8Array(bytes), mimeType: "application/pdf" };
    },
    getDealDocumentText: (dealId, fileId) =>
      transport.get<DealDocumentText>(
        `/api/v1/deals/${encodeURIComponent(dealId)}/documents/${encodeURIComponent(fileId)}/text`,
      ),
    getTemplate: async (templateId) => parseDiligenceCanvasDocument(
      await transport.get<unknown>(`/api/v1/templates/${encodeURIComponent(templateId)}`),
    ),
    async getUserByEmail(email) {
      try {
        return await transport.get<WorkspaceAccountUser>(
          `/api/v1/users/by-email?email=${encodeURIComponent(email.trim())}`,
        );
      } catch (error) {
        if (errorMessage(error).toLowerCase().includes("user not found")) return null;
        throw error;
      }
    },
    async importPptxTemplate(
      file,
      importMode: PptxTemplateImportMode,
    ): Promise<PptxTemplateImportResult> {
      if (!file.name.toLowerCase().endsWith(".pptx")) {
        throw new Error("Template imports require a .pptx file.");
      }
      return transport.postMultipart<PptxTemplateImportResult>(
        await multipartFiles(
          `/api/v1/templates/import?mode=${encodeURIComponent(importMode)}`,
          [file],
          [],
          "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
      );
    },
    listDealDataRoom: (dealId) =>
      transport.get<DealDataRoom>(`/api/v1/deals/${encodeURIComponent(dealId)}/data-room`),
    listAssistantThreads: (userEmail, options = {}) => {
      const parameters = new URLSearchParams({ userEmail });
      if (options.after) parameters.set("before", options.after);
      if (options.archived) parameters.set("archived", "true");
      return transport.get<AssistantThreadPage>(
        `/api/v1/assistant/threads?${parameters.toString()}`,
      );
    },
    listDealDocuments: (dealId) =>
      transport.get<DealDocumentSummary[]>(
        `/api/v1/deals/${encodeURIComponent(dealId)}/documents`,
      ),
    listDeals: () => transport.get<PersistedDeal[]>("/api/v1/deals"),
    listTemplatePreviews: (page) =>
      transport.get<TemplatePreviewPage>(
        `/api/v1/templates/previews?page=${encodeURIComponent(String(page))}`,
      ),
    listSummaryFiles: (path) =>
      transport.post<SummarizableFile[]>("/api/v1/summarize/files", { path }),
    previewDealDocument: (dealId, relativePath) =>
      transport.post<DocumentPreviewResponse>(
        `/api/v1/deals/${encodeURIComponent(dealId)}/data-room/preview`,
        { relativePath },
      ),
    async processDocuments(dealId, userId, files) {
      return transport.postMultipart<ProcessDocumentsResponse>(
        await multipartFiles(`/api/v1/deals/${encodeURIComponent(dealId)}/documents/process`, files, [
          { name: "userId", value: userId.trim() },
        ]),
      );
    },
    queryModel(input: QueryModelInput, handlers: SendQueryEventHandlers) {
      let active = true;
      let settled = false;
      let stop: (() => void) | undefined;
      const stopTransport = () => {
        const cleanup = stop;
        stop = undefined;
        cleanup?.();
      };
      void Promise.all(input.files.map((file) => fileToMultipart(file, "application/octet-stream", false)))
        .then((files) => {
          if (!active) return undefined;
          if (!transport.startQuery) {
            throw new Error("The desktop query transport is unavailable.");
          }
          return transport.startQuery({
            context: input.context.map((message) => ({ ...message })),
            files,
            model: input.model,
            path: "/api/v1/query_model",
            prompt: input.prompt,
            systemInstructions: input.systemInstructions,
          }, (payload) => {
            if (!active || settled) return;
            if (payload.kind === "connectionError") {
              settled = true;
              active = false;
              stopTransport();
              handlers.onConnectionError?.(payload.message);
              return;
            }
            try {
              const event = parseQueryEvent(payload.event);
              handlers.onEvent(event);
              if (event.type === "completed" || event.type === "failed") {
                settled = true;
                active = false;
                stopTransport();
              }
            } catch (error) {
              settled = true;
              active = false;
              stopTransport();
              handlers.onConnectionError?.(
                error instanceof Error ? error.message : "The query stream returned an invalid event.",
              );
            }
          });
        })
        .then((cleanup) => {
          if (!cleanup) return;
          if (active) stop = cleanup;
          else cleanup();
        })
        .catch((error) => {
          if (active && !settled) {
            settled = true;
            active = false;
            handlers.onConnectionError?.(
              error instanceof Error ? error.message : "The desktop query connection failed.",
            );
          }
        });
      return () => {
        if (!active) return;
        active = false;
        stopTransport();
      };
    },
    runAssistantThread(input: RunAssistantThreadInput, handlers: SendQueryEventHandlers) {
      let active = true;
      let settled = false;
      let stop: (() => void) | undefined;
      const stopTransport = () => {
        const cleanup = stop;
        stop = undefined;
        cleanup?.();
      };
      void Promise.all(
        input.files.map((file) => fileToMultipart(file, "application/octet-stream", false)),
      )
        .then((files) => {
          if (!active) return undefined;
          if (!transport.startQuery) {
            throw new Error("The desktop query transport is unavailable.");
          }
          return transport.startQuery({
            assistantMessageId: input.assistantMessageId,
            files,
            model: input.model,
            parentMessageId: input.parentMessageId,
            path: `/api/v1/assistant/threads/${encodeURIComponent(input.threadId)}/runs`,
            prompt: input.prompt,
            requestId: input.requestId,
            systemInstructions: input.systemInstructions,
            userEmail: input.userEmail,
            userMessageId: input.userMessageId,
          }, (payload) => {
            if (!active || settled) return;
            if (payload.kind === "connectionError") {
              settled = true;
              active = false;
              stopTransport();
              handlers.onConnectionError?.(payload.message);
              return;
            }
            try {
              const event = parseQueryEvent(payload.event);
              handlers.onEvent(event);
              if (event.type === "completed" || event.type === "failed") {
                settled = true;
                active = false;
                stopTransport();
              }
            } catch (error) {
              settled = true;
              active = false;
              stopTransport();
              handlers.onConnectionError?.(
                error instanceof Error
                  ? error.message
                  : "The query stream returned an invalid event.",
              );
            }
          });
        })
        .then((cleanup) => {
          if (!cleanup) return;
          if (active) stop = cleanup;
          else cleanup();
        })
        .catch((error) => {
          if (active && !settled) {
            settled = true;
            active = false;
            handlers.onConnectionError?.(
              error instanceof Error ? error.message : "The desktop query connection failed.",
            );
          }
        });
      return () => {
        if (!active) return;
        active = false;
        stopTransport();
      };
    },
    renameAssistantThread: (threadId, userEmail, title) =>
      transport.post<void>(
        `/api/v1/assistant/threads/${encodeURIComponent(threadId)}/rename`,
        { title, userEmail },
      ),
    saveDealMetadata: async (dealId, input: SaveDealMetadataInput) =>
      transport.postMultipart<SaveDealMetadataResponse>(
        await multipartFiles(
          `/api/v1/deals/${encodeURIComponent(dealId)}/metadata`,
          input.files,
          [
            { name: "sharepointLink", value: input.sharepointLink ?? "" },
            { name: "sowLink", value: input.sowLink ?? "" },
            { name: "factSheetLink", value: input.factSheetLink ?? "" },
            { name: "rlLink", value: input.rlLink ?? "" },
          ],
        ),
      ),
    searchDocumentChunksByKeyword: (search: FileChunkKeywordSearch) =>
      transport.post<KeywordFileChunkHit[]>("/api/v1/documents/search/keyword", search),
    searchDocumentChunksByVector: (search: FileChunkVectorSearch) =>
      transport.post<VectorFileChunkHit[]>("/api/v1/documents/search/vector", search),
    async startProcessFile(dealId, userId, file) {
      return transport.postMultipart<ProcessFileJobResponse>(
        await multipartFiles(`/api/v1/deals/${encodeURIComponent(dealId)}/documents/process_file`, [file], [
          { name: "userId", value: userId.trim() },
        ]),
      );
    },
    subscribeToProcessFileJob(jobId, handlers: ProcessFileJobEventHandlers) {
      let active = true;
      let unsubscribe: (() => void) | undefined;
      void transport
        .subscribeJob(
          jobId,
          (_eventName, data) => {
            if (!active) return;
            try {
              handlers.onEvent(JSON.parse(data) as ProcessFileJobEvent);
            } catch {
              handlers.onEvent({
                error: "The processing stream returned an invalid event.",
                filename: "",
                jobId,
                status: "failed",
              });
            }
          },
          () => active && handlers.onConnectionError?.(),
        )
        .then((cleanup) => {
          if (active) unsubscribe = cleanup;
          else cleanup();
        })
        .catch(() => active && handlers.onConnectionError?.());
      return () => {
        active = false;
        unsubscribe?.();
      };
    },
    async summarizePath(path) {
      const response = await transport.post<{ summary: string }>("/api/v1/summarize", { path });
      return response.summary;
    },
    async summarizeSelected(paths) {
      const response = await transport.post<{ summary: string }>(
        "/api/v1/summarize/selected",
        { paths },
      );
      return response.summary;
    },
    async summarizeUpload(files) {
      const response = await transport.postMultipart<{ summary: string }>(
        await multipartFiles("/api/v1/summarize/upload", files),
      );
      return response.summary;
    },
    unarchiveAssistantThread: (threadId, userEmail) =>
      transport.post<void>(
        `/api/v1/assistant/threads/${encodeURIComponent(threadId)}/unarchive`,
        { userEmail },
      ),
    async userExistsByEmail(email) {
      return (await this.getUserByEmail(email)) !== null;
    },
  };
}

async function fileToMultipart(file: File, fallbackMimeType: string, preserveRelativePath = true) {
  const relativeFile = file as File & { webkitRelativePath?: string };
  return {
    dataBase64: bytesToBase64(new Uint8Array(await file.arrayBuffer())),
    fieldName: "files",
    filename: preserveRelativePath ? relativeFile.webkitRelativePath || file.name : file.name,
    mimeType: file.type || fallbackMimeType,
  };
}

function bytesToBase64(bytes: Uint8Array) {
  let binary = "";
  const chunkSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + chunkSize));
  }
  return btoa(binary);
}

function errorMessage(error: unknown) {
  if (typeof error === "object" && error && "message" in error) {
    return String(error.message);
  }
  return error instanceof Error ? error.message : String(error);
}
