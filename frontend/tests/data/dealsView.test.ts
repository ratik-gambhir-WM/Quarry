import { testParty } from "@/test/fixtures/deals";
import type { DealStatus } from "@/data/dealModel";
import { describe, expect, it } from "vitest";
import {
  buildDealKanbanLanes,
  buildDealPortfolioView,
  filterDealPortfolioViews,
  getDealCounts,
  getDealTransitionName,
} from "@/data/dealsView";
import type { WorkspaceDeal } from "@/data/workspace";

function makeDeal(
  id: string,
  status: DealStatus,
  overrides: Partial<WorkspaceDeal> = {},
): WorkspaceDeal {
  return {
    colorClassName: "bg-primary",
    portfolio: {
      engagementType: "sell_side",
      parties: [testParty("Northstar Equity", "pe_sponsor"), testParty("Blue River", "buyer"), testParty(`${id} Target`, "target"), testParty("Parent Name", "parent_company")],
      status,
      transactionType: "strategic_acquisition",
    },
    room: {
      id,
      keyQuestions: ["One?"],
      metrics: [],
      name: id,
      overviewSubtitle: "Overview",
      pendingTasks: [],
      phaseLabel: status,
      resources: [],
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
    const current = makeDeal("Current Deal", "active");
    const historic = makeDeal("Historic Deal", "completed");
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

  it("maps typed statuses to explicit lanes with historic last", () => {
    const deals = [
      makeDeal("unknown", "on_hold"),
      makeDeal("historic", "completed"),
      makeDeal("pipeline", "pipeline"),
      makeDeal("review", "active"),
    ].map(buildDealPortfolioView);

    const lanes = buildDealKanbanLanes(deals);

    expect(lanes.map((lane) => lane.title)).toEqual([
      "Pipeline",
      "Active / In Progress",
      "On hold",
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
