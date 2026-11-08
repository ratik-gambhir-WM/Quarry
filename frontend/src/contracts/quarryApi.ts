import type { DealMembershipApi } from "./dealMemberships";
export type * from "./dealMemberships";
import type { NotebookApi } from "./notebooks";
export type * from "./notebooks";
import type {
  Company,
  CompanyInput,
  CompanyPage,
  CompanySearch,
  DealFields,
} from "../data/dealModel";
import type {
  LocalDealDataRoom,
  LocalDealFileContents,
  ReadDealSourceFilesInput,
  SaveDealInput,
  SaveDealMetadataInput,
  SaveDealMetadataResponse,
  SaveDealResponse,
  SavedDeal,
  SavedDealMetadata,
} from "../data/dealExtraction";
import type {
  DealDataRoom,
  DocumentPreviewResponse,
} from "../data/dataRoomPreview";
import type { WorkspaceAccountUser } from "../data/workspace";
import type { DiligenceCanvasDocument } from "./diligenceCanvas";

export type PersistedDeal = SavedDeal & {
  metadata: SavedDealMetadata | null;
};

/** `username` accepts either the registered username or email address. */
export type LoginInput = { username: string; password: string };
export type RegisterInput = LoginInput & {
  email: string;
  firstName: string;
  lastName: string;
  jobTitle?: string;
};
export type SessionResponse = {
  user: WorkspaceAccountUser;
  accessToken: string;
  tokenType: "Bearer";
  expiresAt: string;
};
export type CurrentUserResponse = {
  user: WorkspaceAccountUser;
  expiresAt: string;
};

export type ProcessedDocument = {
  sqliteCommitted?: boolean;
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

export type ChatFileReference = {
  dealId: string;
  fileId: string;
  name: string;
};

export type AssistantThread = {
  dealId: string | null;
  createdAt: string;
  lastMessageAt: string;
  status: "regular" | "archived";
  threadId: string;
  title: string;
  updatedAt: string;
};

export type AssistantMessage = {
  completedAt: string | null;
  content: string;
  createdAt: string;
  messageId: string;
  model: string | null;
  parentMessageId: string | null;
  role: "user" | "assistant";
  sequence: number;
  status: "streaming" | "completed" | "failed" | "cancelled";
  threadId: string;
};

export type AssistantThreadDetail = AssistantThread & {
  messages: AssistantMessage[];
};

export type AssistantThreadPage = {
  nextCursor: string | null;
  threads: AssistantThread[];
};

export type RunAssistantThreadInput = {
  fileReferences?: ChatFileReference[];
  fileScopeDealId?: string;
  assistantMessageId: string;
  files: File[];
  model?: string;
  parentMessageId?: string;
  prompt: string;
  requestId: string;
  systemInstructions?: string;
  threadId: string;
  userMessageId: string;
};

export type SendQueryEvent =
  | {
      assistantMessageId?: string;
      model: string;
      threadId?: string;
      type: "started";
      userMessageId?: string;
    }
  | { delta: string; type: "delta" }
  | { response: string; type: "completed" }
  | { error: string; type: "failed" };

export type SendQueryEventHandlers = {
  onConnectionError?: (message: string) => void;
  onEvent: (event: SendQueryEvent) => void;
};

export type DealDocumentSummary = {
  uploadedByUserId?: number | null;
  uploadedByName?: string | null;
  attributionOrigin?: "authenticated" | "legacy_unknown";
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
};

export type FileChunkKeywordSearch = {
  limit: number;
  queryText: string;
};

export type HybridDocumentSearch = { queryText: string; limit: number };
export type HybridDocumentHit = {
  chunkId: string; dealId: string; fileId: string; versionId: string;
  text: string; chunkIndex: number; pageStart: number | null; pageEnd: number | null;
  sectionPath: string; matchSources: ("keyword" | "semantic")[];
};
export type HybridDocumentResponse = { results: HybridDocumentHit[] };

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
  deal_id: string;
};

export type VectorFileChunkHit = FileChunkResult & { distance: number };
export type KeywordFileChunkHit = FileChunkResult & { score: number };

export type TemplatePreviewPage = {
  pagination: {
    page: number;
    pageSize: number;
    totalItems: number;
    totalPages: number;
    hasNextPage: boolean;
    hasPreviousPage: boolean;
  };
  previews: Array<{
    templateId: string;
    contentType: "image/png";
    dataUrl: string;
    previewUrl: string;
    width: number;
    height: number;
  }>;
};

export type PptxTemplateImportMode = "single" | "batch";
export type TemplateProcessingStatus = "not_ready" | "pending" | "processing" | "ready" | "failed";

export type TemplateRetrievalMetadata = {
  subject: {
    summary: string;
    domains: Array<{ id: string; relevance: string; topics: string[] }>;
    otherTopics: string[];
    technologies: string[];
    entities: string[];
    claims: string[];
    synonyms: string[];
  };
  communication: { intents: string[]; informationTypes: string[]; audience: string[] };
  templateFit: {
    archetype: string;
    contentSlots: Array<{ elementId: string; role: string; capacity: string }>;
  };
  visual: {
    layoutType: string;
    visualElements: string[];
    contentDensity: string;
    structuralFeatures: string[];
  };
  retrievalKeywords: string[];
};

export type TemplateRetrieval = {
  templateId: string;
  retrievalStatus: TemplateProcessingStatus;
  classification: TemplateProcessingProgress;
  embedding: TemplateProcessingProgress;
  metadata: TemplateRetrievalMetadata | null;
  embeddings?: {
    subjectVector: number[] | null;
    capabilityVector: number[] | null;
    dimensions: number | null;
    model: string | null;
    status: TemplateProcessingStatus;
  };
};

export type TemplateProcessingProgress = {
  status: TemplateProcessingStatus;
  attemptCount: number;
  nextAttemptAt: string | null;
  lastErrorCode: string | null;
};

export type TemplateProcessingRetry = {
  templateId: string;
  stage: "classification" | "embedding";
};

export const MAX_PPTX_TEMPLATE_IMPORT_BYTES = 25 * 1024 * 1024;

/** Confirms saved templates; classification and embeddings continue on the server. */
export type PptxTemplateImportResult = {
  importMode: PptxTemplateImportMode;
  importedCount: number;
  warningCount: number;
};

export const POWERPOINT_CONTENT_TYPE =
  "application/vnd.openxmlformats-officedocument.presentationml.presentation" as const;

export type PowerPointExport = {
  dataBase64: string;
  fileName: string;
  mimeType: typeof POWERPOINT_CONTENT_TYPE;
  warningCount: number;
};

export type AssistantThreadListOptions = {
  after?: string;
  archived?: boolean;
  scope?: "all" | "freeform" | "deal";
  dealId?: string;
};
export interface QuarryApi extends NotebookApi, DealMembershipApi {
  createCompany(input: CompanyInput): Promise<Company>;
  updateCompany(companyId: string, input: CompanyInput): Promise<Company>;
  listCompanies(options?: CompanySearch): Promise<CompanyPage>;
  updateDeal(dealId: string, input: DealFields): Promise<SaveDealResponse>;
  archiveAssistantThread(threadId: string): Promise<void>;
  archiveDeal(dealId: string): Promise<SavedDeal>;
  createAssistantThread(
    threadId?: string,
    dealId?: string | null,
  ): Promise<AssistantThread>;
  createDeal(input: SaveDealInput): Promise<SaveDealResponse>;
  login(input: LoginInput): Promise<SessionResponse>;
  register(input: RegisterInput): Promise<SessionResponse>;
  currentUser(): Promise<CurrentUserResponse>;
  logout(): Promise<void>;
  deleteTemplate(templateId: string): Promise<void>;
  deleteAssistantThread(threadId: string): Promise<void>;
  exportPowerPoint(
    document: DiligenceCanvasDocument,
  ): Promise<PowerPointExport>;
  saveDealMetadata(
    dealId: string,
    input: SaveDealMetadataInput,
  ): Promise<SaveDealMetadataResponse>;
  getDeal(dealId: string): Promise<PersistedDeal>;
  getAssistantThread(threadId: string): Promise<AssistantThreadDetail>;
  getDealDocumentPdf(dealId: string, fileId: string): Promise<DealDocumentPdf>;
  getDealDocumentText(
    dealId: string,
    fileId: string,
  ): Promise<DealDocumentText>;
  getTemplate(templateId: string): Promise<DiligenceCanvasDocument>;
  getTemplateRetrieval(templateId: string, includeEmbeddings?: boolean): Promise<TemplateRetrieval>;
  retryTemplateProcessing(templateId: string): Promise<TemplateProcessingRetry>;
  importPptxTemplate(
    file: File,
    importMode: PptxTemplateImportMode,
  ): Promise<PptxTemplateImportResult>;
  listDealDataRoom(dealId: string): Promise<DealDataRoom>;
  listAssistantThreads(
    options?: AssistantThreadListOptions,
  ): Promise<AssistantThreadPage>;
  listDealDocuments(dealId: string): Promise<DealDocumentSummary[]>;
  listDeals(): Promise<PersistedDeal[]>;
  listTemplatePreviews(page: number): Promise<TemplatePreviewPage>;
  previewDealDocument(
    dealId: string,
    relativePath: string,
  ): Promise<DocumentPreviewResponse>;
  processDocuments(
    dealId: string,
    files: File[],
  ): Promise<ProcessDocumentsResponse>;
  renameAssistantThread(threadId: string, title: string): Promise<void>;
  runAssistantThread(
    input: RunAssistantThreadInput,
    handlers: SendQueryEventHandlers,
  ): () => void;
  searchDealDocumentChunksHybrid(dealId: string, search: HybridDocumentSearch, signal?: AbortSignal): Promise<HybridDocumentResponse>;
  searchDocumentChunksHybrid(search: HybridDocumentSearch, signal?: AbortSignal): Promise<HybridDocumentResponse>;
  searchDealDocumentChunksByKeyword(
    dealId: string,
    search: FileChunkKeywordSearch,
  ): Promise<KeywordFileChunkHit[]>;
  searchDealDocumentChunksByVector(
    dealId: string,
    search: FileChunkVectorSearch,
  ): Promise<VectorFileChunkHit[]>;
  searchDocumentChunksByKeyword(
    search: FileChunkKeywordSearch,
  ): Promise<KeywordFileChunkHit[]>;
  searchDocumentChunksByVector(
    search: FileChunkVectorSearch,
  ): Promise<VectorFileChunkHit[]>;
  startProcessFile(dealId: string, file: File): Promise<ProcessFileJobResponse>;
  subscribeToProcessFileJob(
    jobId: string,
    handlers: ProcessFileJobEventHandlers,
  ): () => void;
  summarizeUpload(files: File[]): Promise<string>;
  unarchiveAssistantThread(threadId: string): Promise<void>;
}

export type SaveFileInput = {
  contents: string;
  extensions: string[];
  mimeType: string;
  suggestedName: string;
  title: string;
};

export type SavePowerPointInput = {
  dataBase64: string;
  suggestedName: string;
  title: string;
};

export interface PlatformCapabilities {
  readDealSourceFiles(
    input: ReadDealSourceFilesInput,
  ): Promise<LocalDealFileContents[]>;
  saveFile(input: SaveFileInput): Promise<boolean>;
  savePowerPoint(input: SavePowerPointInput): Promise<boolean>;
  selectDealDataRoom(): Promise<LocalDealDataRoom | null>;
}

export interface QuarryRuntime {
  api: QuarryApi;
  platform: PlatformCapabilities;
  target: "web" | "desktop";
}
