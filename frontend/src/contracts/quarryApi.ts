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

export type DealDocumentSummary = {
  displayName: string;
  fileId: string;
};

export type DealDocumentPdf = {
  bytes: Uint8Array;
  mimeType: "application/pdf";
};

export type DealDocumentText = {
  fileName: string;
  sourceKind: "docx" | "pdf";
  text: string;
};

export type FileChunkVectorSearch = {
  limit: number;
  queryEmbedding: number[];
  workspaceId: string;
};

export type FileChunkKeywordSearch = {
  limit: number;
  queryText: string;
  workspaceId: string;
};

export type FileChunkResult = {
  char_end: number;
  char_start: number;
  chunk_id: string;
  chunk_index: number;
  chunk_sha256: string;
  created_at: string;
  file_id: string;
  index_generation: string;
  page_end: number | null;
  page_start: number | null;
  section_path: string;
  text: string;
  token_count: number;
  version_id: string;
  workspace_id: string;
};

export type VectorFileChunkHit = FileChunkResult & { distance: number };
export type KeywordFileChunkHit = FileChunkResult & { score: number };

export interface QuarryApi {
  archiveDeal(dealId: string): Promise<SavedDeal>;
  createDeal(input: SaveDealInput): Promise<SaveDealResponse>;
  createUser(input: AddUserInput): Promise<WorkspaceAccountUser>;
  saveDealMetadata(dealId: string, files: File[]): Promise<SaveDealMetadataResponse>;
  getDeal(dealId: string): Promise<PersistedDeal>;
  getDealDocumentPdf(dealId: string, fileId: string): Promise<DealDocumentPdf>;
  getDealDocumentText(dealId: string, fileId: string): Promise<DealDocumentText>;
  getUserByEmail(email: string): Promise<WorkspaceAccountUser | null>;
  listDealDataRoom(dealId: string): Promise<DealDataRoom>;
  listDealDocuments(dealId: string): Promise<DealDocumentSummary[]>;
  listDeals(): Promise<PersistedDeal[]>;
  listSummaryFiles(path: string): Promise<SummarizableFile[]>;
  previewDealDocument(
    dealId: string,
    relativePath: string,
  ): Promise<DocumentPreviewResponse>;
  processDocuments(
    dealId: string,
    userId: string,
    files: File[],
  ): Promise<ProcessDocumentsResponse>;
  searchDocumentChunksByKeyword(
    search: FileChunkKeywordSearch,
  ): Promise<KeywordFileChunkHit[]>;
  searchDocumentChunksByVector(
    search: FileChunkVectorSearch,
  ): Promise<VectorFileChunkHit[]>;
  startProcessFile(
    dealId: string,
    userId: string,
    file: File,
  ): Promise<ProcessFileJobResponse>;
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
