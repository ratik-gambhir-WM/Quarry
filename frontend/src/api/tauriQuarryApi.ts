import type {
  AddUserInput,
  FileChunkKeywordSearch,
  FileChunkVectorSearch,
  KeywordFileChunkHit,
  PersistedDeal,
  ProcessDocumentsResponse,
  ProcessFileJobEvent,
  ProcessFileJobEventHandlers,
  ProcessFileJobResponse,
  QuarryApi,
  SummarizableFile,
  VectorFileChunkHit,
} from "../contracts/quarryApi";
import type {
  SaveDealInput,
  SaveDealMetadataResponse,
  SaveDealResponse,
  SavedDeal,
} from "../data/dealExtraction";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { WorkspaceAccountUser } from "../data/workspace";

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

type TauriTransport = {
  get<T>(path: string): Promise<T>;
  post<T>(path: string, body: unknown): Promise<T>;
  postMultipart<T>(request: TauriMultipartRequest): Promise<T>;
  subscribeJob(
    jobId: string,
    onEvent: (eventName: string, data: string) => void,
    onError: () => void,
  ): Promise<() => void>;
};

export function createTauriQuarryApi(transport: TauriTransport): QuarryApi {
  async function multipartFiles(path: string, files: File[], fields: TauriMultipartRequest["fields"] = []) {
    return {
      fields,
      files: await Promise.all(files.map(fileToMultipart)),
      path,
    } satisfies TauriMultipartRequest;
  }

  return {
    archiveDeal: (dealId) =>
      transport.post<SavedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}/archive`, {}),
    createDeal: (input: SaveDealInput) =>
      transport.post<SaveDealResponse>("/api/v1/deals", input),
    createUser: (input: AddUserInput) =>
      transport.post<WorkspaceAccountUser>("/api/v1/users", input),
    getDeal: (dealId) =>
      transport.get<PersistedDeal>(`/api/v1/deals/${encodeURIComponent(dealId)}`),
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
    listDealDataRoom: (dealId) =>
      transport.get<DealDataRoom>(`/api/v1/deals/${encodeURIComponent(dealId)}/data-room`),
    listDeals: () => transport.get<PersistedDeal[]>("/api/v1/deals"),
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
    saveDealMetadata: async (dealId, files) =>
      transport.postMultipart<SaveDealMetadataResponse>(
        await multipartFiles(`/api/v1/deals/${encodeURIComponent(dealId)}/metadata`, files),
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
    async userExistsByEmail(email) {
      return (await this.getUserByEmail(email)) !== null;
    },
  };
}

async function fileToMultipart(file: File) {
  const relativeFile = file as File & { webkitRelativePath?: string };
  return {
    dataBase64: bytesToBase64(new Uint8Array(await file.arrayBuffer())),
    fieldName: "files",
    filename: relativeFile.webkitRelativePath || file.name,
    mimeType: file.type || "application/octet-stream",
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
