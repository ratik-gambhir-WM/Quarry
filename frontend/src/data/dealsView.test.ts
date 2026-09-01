import { describe, expect, it } from "vitest";
import {
  buildDealKanbanLanes,
  buildDealPortfolioView,
  filterDealPortfolioViews,
  getDealCounts,
  getDealTransitionName,
} from "./dealsView";
import type { WorkspaceDeal } from "./workspace";

function makeDeal(
  id: string,
  status: string,
  overrides: Partial<WorkspaceDeal> = {},
): WorkspaceDeal {
  return {
    colorClassName: "bg-primary",
    portfolio: {
      dealSponsor: "Northstar Equity",
      primaryBuyer: "Blue River",
      status,
      targetCompany: `${id} Target`,
      transactionType: "Acquisition",
    },
    room: {
      id,
      keyQuestions: ["One?"],
      metrics: [],
      name: id,
      overviewSubtitle: "Overview",
      pendingTasks: [],
      phaseLabel: status,
      sectorLabel: "Industrials",
      stageLabel: status,
      summary: "Summary",
      timeline: [],
    },
    ...overrides,
  };
}

describe("deals view selectors", () => {
  it("filters scope and search without mutating the source array", () => {
    const current = makeDeal("Current Deal", "Active");
    const historic = makeDeal("Historic Deal", "Completed");
    const source = [current, historic];

    expect(filterDealPortfolioViews(source, "northSTAR", "current").map((deal) => deal.id)).toEqual([
      "Current Deal",
    ]);
    expect(filterDealPortfolioViews(source, "historic deal target", "all").map((deal) => deal.id)).toEqual([
      "Historic Deal",
    ]);
    expect(source).toEqual([current, historic]);
    expect(getDealCounts(source)).toEqual({ current: 1, historic: 1, total: 2 });
  });

  it("forces completed deals into the final historic lane and preserves unknown statuses", () => {
    const deals = [
      makeDeal("unknown", "Awaiting IC"),
      makeDeal("historic", "Active", { complete: true }),
      makeDeal("pipeline", "Pipeline"),
      makeDeal("review", "Under Review"),
    ].map(buildDealPortfolioView);

    const lanes = buildDealKanbanLanes(deals);

    expect(lanes.map((lane) => lane.title)).toEqual([
      "Pipeline",
      "Under Review",
      "Awaiting IC",
      "Historic / Closed",
    ]);
    expect(lanes[lanes.length - 1]?.deals.map((deal) => deal.id)).toEqual(["historic"]);
    expect(lanes.flatMap((lane) => lane.deals)).toHaveLength(deals.length);
  });

  it("creates CSS-safe deterministic transition names for arbitrary identifiers", () => {
    expect(getDealTransitionName("Deal / 東京")).toMatch(/^deal-surface-[a-z0-9-]+$/);
    expect(getDealTransitionName("Deal / 東京")).toBe(getDealTransitionName("Deal / 東京"));
    expect(getDealTransitionName("Deal / 東京")).not.toBe(getDealTransitionName("Deal / 大阪"));
  });
});
