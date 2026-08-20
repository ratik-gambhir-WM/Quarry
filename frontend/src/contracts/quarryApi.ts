import type {
  LocalDealDataRoom,
  LocalDealFileContents,
  ReadDealSourceFilesInput,
  SaveDealInput,
  SaveDealMetadataResponse,
  SaveDealResponse,
  SavedDeal,
  SavedDealMetadata,
} from "../data/dealExtraction";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { WorkspaceAccountUser } from "../data/workspace";

export type PersistedDeal = SavedDeal & {
  metadata: SavedDealMetadata | null;
};

export type SummarizableFile = {
  filename: string;
  mimeType: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
  supported: boolean;
};

export type AddUserInput = {
  apiKey: string;
  email: string;
  firstName: string;
  lastName: string;
  role: string;
};

export type ProcessedDocument = {
  chunkCount: number;
  documentId: string | null;
  error: string | null;
  filename: string;
  skipped: boolean;
  success: boolean;
};

export type ProcessDocumentsResponse = {
  documents: ProcessedDocument[];
  failed: number;
  skipped: number;
  succeeded: number;
  total: number;
};

export type ProcessFileJobResponse = {
  filename: string;
  jobId: string;
};

export type ProcessFileJobEvent = {
  chunkCount?: number;
  documentId?: string;
  error?: string;
  filename: string;
  jobId: string;
  status: "processing" | "completed" | "skipped" | "failed";
};

export type ProcessFileJobEventHandlers = {
  onConnectionError?: () => void;
  onEvent: (event: ProcessFileJobEvent) => void;
};

export type ChunkVectorSearch = {
  limit?: number;
  queryEmbedding: number[];
  userId: string;
};

export type ChunkKeywordSearch = {
  limit?: number;
  queryText: string;
  userId: string;
};

export interface QuarryApi {
  archiveDeal(dealId: string): Promise<SavedDeal>;
  createDeal(input: SaveDealInput): Promise<SaveDealResponse>;
  createUser(input: AddUserInput): Promise<WorkspaceAccountUser>;
  saveDealMetadata(dealId: string, files: File[]): Promise<SaveDealMetadataResponse>;
  getDeal(dealId: string): Promise<PersistedDeal>;
  getUserByEmail(email: string): Promise<WorkspaceAccountUser | null>;
  listDealDataRoom(dealId: string): Promise<DealDataRoom>;
  listDeals(): Promise<PersistedDeal[]>;
  listSummaryFiles(path: string): Promise<SummarizableFile[]>;
  previewDealDocument(
    dealId: string,
    relativePath: string,
  ): Promise<DocumentPreviewResponse>;
  processDocuments(userId: string, files: File[]): Promise<ProcessDocumentsResponse>;
  searchDocumentChunksByKeyword(search: ChunkKeywordSearch): Promise<unknown>;
  searchDocumentChunksByVector(search: ChunkVectorSearch): Promise<unknown>;
  startProcessFile(userId: string, file: File): Promise<ProcessFileJobResponse>;
  subscribeToProcessFileJob(
    jobId: string,
    handlers: ProcessFileJobEventHandlers,
  ): () => void;
  summarizePath(path: string): Promise<string>;
  summarizeSelected(paths: string[]): Promise<string>;
  summarizeUpload(files: File[]): Promise<string>;
  userExistsByEmail(email: string): Promise<boolean>;
}

export type SaveFileInput = {
  contents: string;
  extensions: string[];
  mimeType: string;
  suggestedName: string;
  title: string;
};

export interface PlatformCapabilities {
  readDealSourceFiles(input: ReadDealSourceFilesInput): Promise<LocalDealFileContents[]>;
  saveFile(input: SaveFileInput): Promise<boolean>;
  selectDealDataRoom(): Promise<LocalDealDataRoom | null>;
}

export interface QuarryRuntime {
  api: QuarryApi;
  platform: PlatformCapabilities;
  target: "web" | "desktop";
}
