import type {
  SavedDeal,
  SavedDealMetadata,
} from "../../data/dealExtraction";
import type {
  DealDataRoom,
  DocumentPreviewResponse,
} from "../../data/dataRoomPreview";

export type PersistedDeal = SavedDeal & {
  metadata: SavedDealMetadata | null;
};

export type SelectedLocalFile = {
  name: string;
  path: string;
  sizeBytes: number;
};

export type DocumentJobStatus = "completed" | "failed" | "processing" | "skipped";

export type DocumentJobEvent = {
  chunkCount?: number;
  documentId?: string;
  error?: string;
  filename: string;
  jobId: string;
  status: DocumentJobStatus;
};

export type DocumentChunkSearchResult = {
  distance?: number;
  documentId: string;
  pageNumbers?: number[];
  score?: number;
  sectionTitle?: string;
  sequenceNumber: number;
  text: string;
};

export interface QuarryProductApi {
  archiveDeal(dealId: number): Promise<SavedDeal>;
  describeDocumentFiles(paths: string[]): Promise<SelectedLocalFile[]>;
  getDeal(dealId: number): Promise<PersistedDeal>;
  getDocumentJob(jobId: string): Promise<DocumentJobEvent>;
  listDealDataRoom(dealId: string): Promise<DealDataRoom>;
  listDeals(): Promise<PersistedDeal[]>;
  previewDealDocument(
    dealId: string,
    relativePath: string,
  ): Promise<DocumentPreviewResponse>;
  searchDocumentChunksKeyword(input: {
    limit?: number;
    queryText: string;
    userId: string;
  }): Promise<DocumentChunkSearchResult[]>;
  searchDocumentChunksVector(input: {
    limit?: number;
    queryEmbedding: number[];
    userId: string;
  }): Promise<DocumentChunkSearchResult[]>;
  saveMarkdownSummary(summary: string): Promise<boolean>;
  selectDocumentFiles(): Promise<SelectedLocalFile[]>;
  selectDealDataRoomFolder(): Promise<string | null>;
  selectSummarySource(directory: boolean): Promise<string | null>;
  startDocumentJobs(input: {
    paths: string[];
    userId: string;
  }): Promise<{ jobs: DocumentJobEvent[] }>;
}
