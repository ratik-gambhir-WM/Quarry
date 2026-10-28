import type { DealResource, WorkspaceDeal, WorkspaceLocationState } from "./workspace";

import { codeLabel, isTerminalDealStatus, type DealFields, type DealParty } from "./dealModel";
export type SaveDealInput = DealFields & {
  dealId: string; localPath: string | null; sharepointLink: string | null;
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

export type SavedDeal = Omit<DealFields, "parties"> & {
  dealId: string; userId: number; archivedAt: string | null; createdAt: string; updatedAt: string; parties: DealParty[];
};

export type SavedDealMetadata = {
  dealId: string;
  factSheetLink: string | null;
  keyQuestionsJson: string;
  localPath: string | null;
  rlLink: string | null;
  sharepointLink: string | null;
  sowLink: string | null;
};

export type SaveDealMetadataInput = {
  factSheetLink: string | null;
  files: File[];
  rlLink: string | null;
  sharepointLink: string | null;
  sowLink: string | null;
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
  const roleSummary = deal.parties.map((p) => `${codeLabel(p.role)}: ${p.company.name}`).join("; ");
  return {
    colorClassName: "bg-primary",
    terminal: isTerminalDealStatus(deal.status),
    persisted: deal,
    persistedMetadata: metadata,
    portfolio: {
      startDate: deal.startDate ?? undefined,
      expectedCloseDate: deal.expectedCloseDate ?? undefined,
      actualCloseDate: deal.actualCloseDate ?? undefined,
      engagementCompletedDate: deal.engagementCompletedDate ?? undefined,
      engagementType: deal.engagementType,
      status: deal.status,
      parties: deal.parties,
      transactionType: deal.transactionType,
    },
    room: {
      id: deal.dealId,
      keyQuestions: extraction.keyQuestions,
      metrics: [
        { label: "Start date", value: deal.startDate ?? "Unavailable" },
        { label: "Expected close", value: deal.expectedCloseDate ?? "Unavailable" },
        { label: "Actual close", value: deal.actualCloseDate ?? "Unavailable" },
        { label: "Engagement completed", value: deal.engagementCompletedDate ?? "Unavailable" },
      ],
      name: deal.dealName,
      overviewSubtitle: `${deal.dealName} Due Diligence Overview`,
      pendingTasks: [],
      phaseLabel: codeLabel(deal.engagementType),
      resources: buildDealResources(metadata, files, sowSourceName),
      sectorLabel: codeLabel(deal.transactionType),
      stageLabel: codeLabel(deal.status),
      summary: `${codeLabel(deal.engagementType)} engagement · ${codeLabel(deal.transactionType)}. ${roleSummary ? `${roleSummary}.` : "Companies are unspecified."}${deal.scopes.length ? ` Scope: ${deal.scopes.map(codeLabel).join(", ")}.` : " Scope is unspecified."}`,
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
  const sowHref = getSafeHttpsHref(metadata?.sowLink);
  const factSheetHref = getSafeHttpsHref(metadata?.factSheetLink);
  const sharepointHref = getSafeSharePointHref(metadata?.sharepointLink);
  const requestListHref = getSafeHttpsHref(metadata?.rlLink);

  return [
    {
      availability: sowFile || sowHref ? "available" : "unavailable",
      href: sowHref,
      id: "sow",
      label: "SOW",
      sourceName: sowFile?.filename,
    },
    {
      availability: factSheetHref ? "available" : "unavailable",
      href: factSheetHref,
      id: "fact-sheet",
      label: "Fact Sheet",
    },
    {
      availability: sharepointHref ? "available" : "unavailable",
      href: sharepointHref,
      id: "sharepoint",
      label: "SharePoint VDR",
    },
    {
      availability: requestListHref ? "available" : "unavailable",
      href: requestListHref,
      id: "request-list",
      label: "Request List",
    },
  ];
}

function getSafeSharePointHref(value?: string | null) {
  const href = getSafeHttpsHref(value);
  if (!href) return undefined;

  try {
    const parsed = new URL(href);
    const safeHost = parsed.hostname.toLowerCase().endsWith(".sharepoint.com");
    return safeHost ? parsed.href : undefined;
  } catch {
    return undefined;
  }
}

function getSafeHttpsHref(value?: string | null) {
  if (!value?.trim()) return undefined;

  try {
    const parsed = new URL(value.trim());
    if (parsed.protocol !== "https:" || parsed.username || parsed.password) {
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
