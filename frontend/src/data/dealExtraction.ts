import type { DealResource, WorkspaceDeal, WorkspaceLocationState } from "./workspace";

export type SaveDealInput = {
  closeDate: string;
  dealId: string;
  dealName: string;
  dealSponsor: string;
  localPath: string | null;
  primaryBuyer: string;
  sharepointLink: string | null;
  startDate: string;
  status: string;
  targetCompany: string;
  transactionType: string;
  userEmail: string;
};

export type DealExtractionSourceFile = {
  filename: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
};

export type LocalDealSourceFile = DealExtractionSourceFile & {
  matchedOn: string[];
  mimeType: string;
  textExtracted: boolean;
  textTruncated: boolean;
};

export type LocalDealDataRoom = {
  files: LocalDealSourceFile[];
  rootName: string;
  rootPath: string;
};

export type LocalDealFileContents = DealExtractionSourceFile & {
  dataBase64: string;
  mimeType: string;
};

export type ReadDealSourceFilesInput = {
  paths: string[];
  rootPath: string;
};

export type SavedDeal = {
  closeDate: string;
  dealId: string;
  dealName: string;
  dealSponsor: string;
  primaryBuyer: string;
  startDate: string;
  status: string;
  targetCompany: string;
  transactionType: string;
  userId: number;
};

export type SavedDealMetadata = {
  dealId: string;
  keyQuestionsJson: string;
  localPath: string | null;
  sharepointLink: string | null;
  userId: number;
};

export type DealExtractionResult = {
  keyQuestions: string[];
};

export type SaveDealResponse = {
  deal: SavedDeal;
  metadata: SavedDealMetadata;
};

export type SaveDealMetadataResponse = SaveDealResponse & {
  extraction: DealExtractionResult;
  files: DealExtractionSourceFile[];
};

export type DealExtractionLocationState = WorkspaceLocationState & {
  result?: SaveDealMetadataResponse;
  sowSourceName?: string;
};

export function buildWorkspaceDealFromExtractionResult(
  result: SaveDealMetadataResponse,
  sowSourceName?: string,
): WorkspaceDeal {
  return buildWorkspaceDeal(
    result.deal,
    result.metadata,
    result.extraction,
    result.files,
    sowSourceName,
  );
}

export function buildWorkspaceDealFromPersisted(
  deal: SavedDeal,
  metadata: SavedDealMetadata | null,
): WorkspaceDeal {
  return buildWorkspaceDeal(deal, metadata, {
    keyQuestions: parseQuestions(metadata?.keyQuestionsJson),
  });
}

function buildWorkspaceDeal(
  deal: SavedDeal,
  metadata: SavedDealMetadata | null,
  extraction: DealExtractionResult,
  files: readonly DealExtractionSourceFile[] = [],
  sowSourceName?: string,
): WorkspaceDeal {
  const normalizedStatus = deal.status.trim().toLowerCase();

  return {
    colorClassName: "bg-primary",
    complete: normalizedStatus === "closed" || normalizedStatus === "completed",
    portfolio: {
      closeDate: deal.closeDate || undefined,
      dealSponsor: deal.dealSponsor || undefined,
      primaryBuyer: deal.primaryBuyer || undefined,
      startDate: deal.startDate || undefined,
      status: deal.status || "Unknown",
      targetCompany: deal.targetCompany || undefined,
      transactionType: deal.transactionType || undefined,
    },
    room: {
      id: deal.dealId,
      keyQuestions: extraction.keyQuestions,
      metrics: [
        { label: "Start Date", value: deal.startDate },
        { label: "Close Date", value: deal.closeDate },
        { label: "Insights Extracted", value: String(extraction.keyQuestions.length) },
      ],
      name: deal.dealName,
      overviewSubtitle: `${deal.dealName} Due Diligence Overview`,
      pendingTasks: [],
      phaseLabel: deal.status,
      resources: buildDealResources(metadata, files, sowSourceName),
      sectorLabel: deal.transactionType,
      stageLabel: deal.status,
      summary: `${deal.targetCompany} is the target company in this ${deal.transactionType.toLowerCase()} opportunity for ${deal.primaryBuyer}, with ${deal.dealSponsor} serving as the deal sponsor. This workspace brings together the available diligence materials, findings, and open questions needed to evaluate the company and support the deal team's review.`,
      timeline: [],
    },
  };
}

export function buildDealResources(
  metadata: SavedDealMetadata | null,
  files: readonly DealExtractionSourceFile[] = [],
  sowSourceName?: string,
): DealResource[] {
  const normalizedSowSourceName = sowSourceName?.trim();
  const sowFile = normalizedSowSourceName
    ? files.find((file) => file.filename === normalizedSowSourceName)
    : undefined;
  const sharepointHref = getSafeSharePointHref(metadata?.sharepointLink);

  return [
    {
      availability: sowFile ? "available" : "unavailable",
      id: "sow",
      label: "SOW",
      sourceName: sowFile?.filename,
    },
    {
      availability: "coming-soon",
      id: "fact-sheet",
      label: "Fact Sheet",
    },
    {
      availability: sharepointHref ? "available" : "unavailable",
      href: sharepointHref,
      id: "sharepoint",
      label: "SharePoint VDR",
    },
  ];
}

function getSafeSharePointHref(value?: string | null) {
  if (!value?.trim()) return undefined;

  try {
    const parsed = new URL(value.trim());
    const safeHost = parsed.hostname.toLowerCase().endsWith(".sharepoint.com");
    if (parsed.protocol !== "https:" || !safeHost || parsed.username || parsed.password) {
      return undefined;
    }
    return parsed.href;
  } catch {
    return undefined;
  }
}

function parseQuestions(value?: string) {
  if (!value) return [];
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed)
      ? parsed.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}
