import { codeLabel, partyNames, type DealStatus } from "./dealModel";
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

const knownLaneOrder = ["pipeline", "active", "on_hold", "historic"] as const;
const knownLaneTitles = { pipeline: "Pipeline", active: "Active / In Progress", on_hold: "On hold", historic: "Historic / Closed" };

export function getDealLifecycle(deal: WorkspaceDeal): DealLifecycle {
  const status = normalizeStatus(deal.portfolio.status || deal.room.stageLabel);
  return status === "completed" || status === "cancelled"
    ? "Historic"
    : "Current";
}

export function buildDealPortfolioView(deal: WorkspaceDeal): DealPortfolioView {
  const status = deal.portfolio.status.trim() || deal.room.stageLabel.trim() || "Unknown";

  return {
    buyer: partyNames(deal.portfolio.parties, "buyer"),
    closeDate: deal.portfolio.actualCloseDate ?? deal.portfolio.expectedCloseDate,
    colorClassName: deal.colorClassName,
    id: deal.room.id,
    lifecycle: getDealLifecycle(deal),
    name: deal.room.name,
    openQuestionCount: deal.room.keyQuestions.length,
    sponsor: partyNames(deal.portfolio.parties, "pe_sponsor"),
    status: codeLabel(status),
    statusCode: deal.portfolio.status,
    engagement: codeLabel(deal.portfolio.engagementType),
    partySearchText: deal.portfolio.parties.map((p) => `${codeLabel(p.role)} ${p.company.name}`).join(" "),
    closeDateLabel: deal.portfolio.actualCloseDate ? "Actual close" : "Expected close",
    targetCompany: partyNames(deal.portfolio.parties, "target"),
    portfolioCompany: partyNames(deal.portfolio.parties, "portfolio_company"),
    sowHref: deal.room.resources.find((resource) => resource.id === "sow")?.href,
    factSheetHref: deal.room.resources.find((resource) => resource.id === "fact-sheet")?.href,
    sharepointHref: deal.room.resources.find((resource) => resource.id === "sharepoint")?.href,
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

  return deals
    .map(buildDealPortfolioView)
    .filter((deal) => {
      const inScope =
        scope === "all" ||
        (scope === "current" && deal.lifecycle === "Current") ||
        (scope === "historic" && deal.lifecycle === "Historic");
      return inScope && (!normalizedQuery || buildDealSearchText(deal).includes(normalizedQuery));
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

export function buildDealKanbanLanes(deals: readonly DealPortfolioView[]): DealKanbanLane[] {
  const lanes = new Map<string, DealKanbanLane>();

  for (const deal of deals) {
    const lane = getDealKanbanLane(deal);
    const existing = lanes.get(lane.id);
    if (existing) {
      existing.deals.push(deal);
    } else {
      lanes.set(lane.id, { ...lane, deals: [deal] });
    }
  }

  return Array.from(lanes.values()).sort((left, right) => {
    const leftIndex = knownLaneOrder.indexOf(left.id as (typeof knownLaneOrder)[number]);
    const rightIndex = knownLaneOrder.indexOf(right.id as (typeof knownLaneOrder)[number]);
    const leftOrder = leftIndex === -1 ? knownLaneOrder.length - 1 : leftIndex;
    const rightOrder = rightIndex === -1 ? knownLaneOrder.length - 1 : rightIndex;

    if (left.id === "historic") return right.id === "historic" ? 0 : 1;
    if (right.id === "historic") return -1;
    return leftOrder - rightOrder || left.title.localeCompare(right.title);
  });
}

export function getDealTransitionName(id: string): string {
  const encoded = Array.from(id, (character) => character.codePointAt(0)?.toString(36) ?? "0").join("-");
  return `deal-surface-${encoded || "empty"}`;
}

function getDealKanbanLane(deal: DealPortfolioView): Omit<DealKanbanLane, "deals"> {
  switch (deal.statusCode) {
    case "completed": case "cancelled": return { id: "historic", title: knownLaneTitles.historic };
    case "pipeline": return { id: "pipeline", title: knownLaneTitles.pipeline };
    case "active": return { id: "active", title: knownLaneTitles.active };
    case "on_hold": return { id: "on_hold", title: knownLaneTitles.on_hold };
  }
}

function normalizeStatus(value: string): string {
  return value.trim().replace(/\s+/g, " ").toLowerCase();
}

function normalizeSearchText(value: string): string {
  return value.trim().toLocaleLowerCase();
}
