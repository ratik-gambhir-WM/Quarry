import type { SavedDeal } from "../../data/dealExtraction";
import type {
  DealDataRoom,
  DocumentPreviewResponse,
} from "../../data/dataRoomPreview";
import { TAURI_COMMANDS } from "../constants";
import { execute } from "../tauri/command";
import type { PersistedDeal, QuarryProductApi, SelectedLocalFile } from "./types";

export const tauriProductApi: QuarryProductApi = {
  archiveDeal(dealId: number) {
    return execute<SavedDeal>(TAURI_COMMANDS.archiveDeal, { dealId });
  },

  describeDocumentFiles(paths: string[]) {
    return execute<SelectedLocalFile[]>(TAURI_COMMANDS.describeDocumentFiles, { paths });
  },

  getDeal(dealId: number) {
    return execute<PersistedDeal>(TAURI_COMMANDS.getDeal, { dealId });
  },

  getDocumentJob(jobId: string) {
    return execute(TAURI_COMMANDS.getDocumentJob, { jobId });
  },

  listDealDataRoom(dealId: string) {
    return execute<DealDataRoom>(TAURI_COMMANDS.listDealDataRoom, { dealId });
  },

  listDeals() {
    return execute<PersistedDeal[]>(TAURI_COMMANDS.listDeals);
  },

  previewDealDocument(dealId: string, relativePath: string) {
    return execute<DocumentPreviewResponse>(
      TAURI_COMMANDS.previewDealDocument,
      { dealId, relativePath },
    );
  },

  searchDocumentChunksKeyword(input) {
    return execute(TAURI_COMMANDS.searchDocumentChunksKeyword, { input });
  },

  searchDocumentChunksVector(input) {
    return execute(TAURI_COMMANDS.searchDocumentChunksVector, { input });
  },

  saveMarkdownSummary(summary: string) {
    return execute(TAURI_COMMANDS.saveMarkdownSummary, { payload: { summary } });
  },

  selectDocumentFiles() {
    return execute(TAURI_COMMANDS.selectDocumentFiles);
  },

  selectDealDataRoomFolder() {
    return execute(TAURI_COMMANDS.selectDealDataRoomFolder);
  },

  selectSummarySource(directory: boolean) {
    return execute(TAURI_COMMANDS.selectSummarySource, { directory });
  },

  startDocumentJobs(input) {
    return execute(TAURI_COMMANDS.startDocumentJobs, { input });
  },
};
