import type { WorkspaceDeal } from "./workspace";

export type DealLifecycle = "Current" | "Historic";
export type DealScope = "all" | "current" | "historic";

export type DealPortfolioView = {
  buyer?: string;
  closeDate?: string;
  colorClassName: string;
  id: string;
  lifecycle: DealLifecycle;
  name: string;
  openQuestionCount: number;
  sponsor?: string;
  status: string;
  targetCompany?: string;
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

const knownLaneOrder = ["pipeline", "active", "review", "risk", "historic"] as const;
const knownLaneTitles: Record<(typeof knownLaneOrder)[number], string> = {
  active: "Active / In Progress",
  historic: "Historic / Closed",
  pipeline: "Pipeline",
  review: "Under Review",
  risk: "On Hold / Risk Watch",
};

export function getDealLifecycle(deal: WorkspaceDeal): DealLifecycle {
  const status = normalizeStatus(deal.portfolio.status || deal.room.stageLabel);
  return deal.complete === true || status === "closed" || status === "completed"
    ? "Historic"
    : "Current";
}

export function buildDealPortfolioView(deal: WorkspaceDeal): DealPortfolioView {
  const status = deal.portfolio.status.trim() || deal.room.stageLabel.trim() || "Unknown";

  return {
    buyer: optionalText(deal.portfolio.primaryBuyer),
    closeDate: optionalText(deal.portfolio.closeDate),
    colorClassName: deal.colorClassName,
    id: deal.room.id,
    lifecycle: getDealLifecycle(deal),
    name: deal.room.name,
    openQuestionCount: deal.room.keyQuestions.length,
    sponsor: optionalText(deal.portfolio.dealSponsor),
    status,
    targetCompany: optionalText(deal.portfolio.targetCompany),
    transitionName: getDealTransitionName(deal.room.id),
    type: deal.portfolio.transactionType?.trim() || deal.room.sectorLabel,
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
  if (deal.lifecycle === "Historic") {
    return { id: "historic", title: knownLaneTitles.historic };
  }

  const status = normalizeStatus(deal.status);
  if (status === "pipeline") return { id: "pipeline", title: knownLaneTitles.pipeline };
  if (status === "active" || status === "in progress" || status === "in-progress") {
    return { id: "active", title: knownLaneTitles.active };
  }
  if (status === "under review") return { id: "review", title: knownLaneTitles.review };
  if (status === "on hold" || status === "risk watch" || status === "at risk") {
    return { id: "risk", title: knownLaneTitles.risk };
  }

  return {
    id: `status-${getDealTransitionName(status)}`,
    title: deal.status || "Unknown",
  };
}

function normalizeStatus(value: string): string {
  return value.trim().replace(/\s+/g, " ").toLowerCase();
}

function normalizeSearchText(value: string): string {
  return value.trim().toLocaleLowerCase();
}

function optionalText(value?: string): string | undefined {
  const normalized = value?.trim();
  return normalized || undefined;
}
