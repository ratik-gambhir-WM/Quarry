import {
  codeLabel,
  dealStatusOptions,
  dealStatusLabels,
  isTerminalDealStatus,
  partyNames,
  type DealStatus,
} from "./dealModel";
import type { WorkspaceDeal } from "./workspace";

export type DealLifecycle = "Current" | "Historic";
export type DealScope = "all" | "current" | "historic";

export type DealPortfolioView = {
  buyer?: string;
  closeDate?: string;
  closeDateLabel: string;
  engagement: string;
  partySearchText: string;
  colorClassName: string;
  id: string;
  lifecycle: DealLifecycle;
  name: string;
  openQuestionCount: number;
  sponsor?: string;
  status: string;
  statusCode: DealStatus;
  targetCompany?: string;
  portfolioCompany?: string;
  sowHref?: string;
  factSheetHref?: string;
  sharepointHref?: string;
  transitionName: string;
  type: string;
};

export type DealCounts = {
  current: number;
  historic: number;
  total: number;
};

export type DealKanbanLane = {
  deals: DealPortfolioView[];
  id: string;
  title: string;
};

export function getDealLifecycle(deal: WorkspaceDeal): DealLifecycle {
  return isTerminalDealStatus(deal.portfolio.status) ? "Historic" : "Current";
}

export function buildDealPortfolioView(deal: WorkspaceDeal): DealPortfolioView {
  const status = deal.portfolio.status;

  return {
    buyer: partyNames(deal.portfolio.parties, "buyer"),
    closeDate:
      deal.portfolio.actualCloseDate ?? deal.portfolio.expectedCloseDate,
    colorClassName: deal.colorClassName,
    id: deal.room.id,
    lifecycle: getDealLifecycle(deal),
    name: deal.room.name,
    openQuestionCount: deal.room.keyQuestions.length,
    sponsor: partyNames(deal.portfolio.parties, "pe_sponsor"),
    status: codeLabel(status),
    statusCode: deal.portfolio.status,
    engagement: codeLabel(deal.portfolio.engagementType),
    partySearchText: deal.portfolio.parties
      .map((p) => `${codeLabel(p.role)} ${p.company.name}`)
      .join(" "),
    closeDateLabel: deal.portfolio.actualCloseDate
      ? "Actual close"
      : "Expected close",
    targetCompany: partyNames(deal.portfolio.parties, "target"),
    portfolioCompany: partyNames(deal.portfolio.parties, "portfolio_company"),
    sowHref: deal.room.resources.find((resource) => resource.id === "sow")
      ?.href,
    factSheetHref: deal.room.resources.find(
      (resource) => resource.id === "fact-sheet",
    )?.href,
    sharepointHref: deal.room.resources.find(
      (resource) => resource.id === "sharepoint",
    )?.href,
    transitionName: getDealTransitionName(deal.room.id),
    type: codeLabel(deal.portfolio.transactionType),
  };
}

export function getDealCounts(deals: readonly WorkspaceDeal[]): DealCounts {
  let historic = 0;
  for (const deal of deals) {
    if (getDealLifecycle(deal) === "Historic") historic += 1;
  }

  return {
    current: deals.length - historic,
    historic,
    total: deals.length,
  };
}

export function filterDealPortfolioViews(
  deals: readonly WorkspaceDeal[],
  query: string,
  scope: DealScope,
): DealPortfolioView[] {
  const normalizedQuery = normalizeSearchText(query);

  return deals.map(buildDealPortfolioView).filter((deal) => {
    const inScope =
      scope === "all" ||
      (scope === "current" && deal.lifecycle === "Current") ||
      (scope === "historic" && deal.lifecycle === "Historic");
    return (
      inScope &&
      (!normalizedQuery || buildDealSearchText(deal).includes(normalizedQuery))
    );
  });
}

export function buildDealSearchText(deal: DealPortfolioView): string {
  return normalizeSearchText(
    [
      deal.name,
      deal.targetCompany,
      deal.sponsor,
      deal.buyer,
      deal.type,
      deal.status,
      deal.lifecycle,
      deal.engagement,
      deal.partySearchText,
    ]
      .filter(Boolean)
      .join(" "),
  );
}

export function buildDealKanbanLanes(
  deals: readonly DealPortfolioView[],
): DealKanbanLane[] {
  if (!deals.length) return [];
  return dealStatusOptions.map((status) => ({
    id: status,
    title: dealStatusLabels[status],
    deals: deals.filter((deal) => deal.statusCode === status),
  }));
}

export function getDealTransitionName(id: string): string {
  const encoded = Array.from(
    id,
    (character) => character.codePointAt(0)?.toString(36) ?? "0",
  ).join("-");
  return `deal-surface-${encoded || "empty"}`;
}

function normalizeSearchText(value: string): string {
  return value.trim().toLocaleLowerCase();
}
