import type { WorkspaceDeal, WorkspaceLocationState } from "./workspace";

export type SaveDealAndExtractInput = {
  buyerOrPlatformCompany: string | null;
  carveOutBusiness: string | null;
  dealName: string;
  dealType: string;
  mainDataRoomFolder: string;
  parentOrSellerCompany: string | null;
  peFirm: string;
  targetCompany: string | null;
};

export type DealExtractionSourceFile = {
  filename: string;
  matchedOn: string[];
  path: string;
  relativePath: string;
  sizeBytes: number;
  textExtracted: boolean;
  textTruncated: boolean;
};

export type SavedDeal = {
  buyerOrPlatformCompany: string | null;
  carveOutBusiness: string | null;
  createdAt: string;
  dealName: string;
  dealType: string;
  id: number;
  mainDataRoomFolder: string;
  parentOrSellerCompany: string | null;
  peFirm: string;
  status: string;
  targetCompany: string | null;
  updatedAt: string;
};

export type DealExtractionResult = {
  keyQuestions: string[];
};

export type SavedDealMetadata = {
  createdAt: string;
  dataRoomSizeBytes: number;
  dealId: number;
  documentCount: number;
  id: number;
  keyQuestionsJson: string;
  updatedAt: string;
};

export type SaveDealAndExtractResponse = {
  deal: SavedDeal;
  extraction: DealExtractionResult;
  files: DealExtractionSourceFile[];
  metadata: SavedDealMetadata;
};

export type SaveDealAndFindFilesResponse = {
  deal: SavedDeal;
  files: DealExtractionSourceFile[];
};

export type ExtractDealQuestionsInput = {
  dealId: number;
  projectTimelineFilePath: string | null;
  sowFilePath: string | null;
};

export type DealExtractionLocationState = WorkspaceLocationState & {
  result?: SaveDealAndExtractResponse;
};

export function buildWorkspaceDealFromExtractionResult(
  result: SaveDealAndExtractResponse,
): WorkspaceDeal {
  return buildWorkspaceDeal(result.deal, result.metadata, result.extraction);
}

export function buildWorkspaceDealFromPersisted(
  deal: SavedDeal,
  metadata: SavedDealMetadata | null,
): WorkspaceDeal {
  const keyQuestions = parseQuestions(metadata?.keyQuestionsJson);
  return buildWorkspaceDeal(deal, metadata, { keyQuestions });
}

function buildWorkspaceDeal(
  deal: SavedDeal,
  metadata: SavedDealMetadata | null,
  extraction: DealExtractionResult,
): WorkspaceDeal {
  const insightCount = extraction.keyQuestions.length;
  return {
    colorClassName: "bg-primary",
    complete: true,
    room: {
      id: String(deal.id),
      keyQuestions: extraction.keyQuestions,
      metrics: [
        { label: "Files Analyzed", value: String(metadata?.documentCount ?? 0) },
        { label: "Insights Extracted", value: String(insightCount) },
        {
          label: "Data Room Size",
          value: formatCompactFileSize(metadata?.dataRoomSizeBytes ?? 0),
        },
      ],
      name: deal.dealName,
      overviewSubtitle: `${deal.dealName} Due Diligence Overview`,
      pendingTasks: [],
      phaseLabel: "Phase 1",
      sectorLabel: deal.dealType,
      stageLabel: "In Progress",
      summary: buildDealSummary(deal),
      timeline: [],
    },
  };
}

function parseQuestions(value?: string) {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}

function buildDealSummary(deal: SavedDeal) {
  const company =
    deal.targetCompany ?? deal.carveOutBusiness ?? deal.buyerOrPlatformCompany ?? deal.dealName;
  return `Evaluating ${company} for ${deal.peFirm}. Current focus is on reviewing the selected diligence materials and extracting key questions.`;
}

function formatCompactFileSize(sizeBytes: number) {
  if (sizeBytes < 1024) return `${sizeBytes} B`;
  if (sizeBytes < 1024 * 1024) return `${(sizeBytes / 1024).toFixed(1)} KB`;
  if (sizeBytes < 1024 * 1024 * 1024) {
    return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(sizeBytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}
