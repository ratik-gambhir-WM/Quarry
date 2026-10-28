import { testParty } from "@/test/fixtures/deals";
import { dealStatusLabels, dealStatusOptions, type DealStatus } from "@/data/dealModel";
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
  it("shares labels and distinguishes stopped and completed work", () => {
    const deals = dealStatusOptions.map((status) => makeDeal(status, status));
    expect(getDealCounts(deals)).toEqual({ current: 7, historic: 2, total: 9 });
    for (const status of dealStatusOptions) {
      expect(buildDealPortfolioView(makeDeal(status, status)).status).toBe(dealStatusLabels[status]);
      expect(filterDealPortfolioViews(deals, dealStatusLabels[status], "all").map((deal) => deal.statusCode)).toContain(status);
    }
    const lanes = buildDealKanbanLanes(deals.map(buildDealPortfolioView));
    expect(lanes.find((lane) => lane.id === "closed")?.deals.map((deal) => deal.statusCode)).toEqual(["closed"]);
    expect(lanes.find((lane) => lane.id === "pencils_down")?.deals.map((deal) => deal.statusCode)).toEqual(["pencils_down"]);
    expect(buildDealKanbanLanes([])).toEqual([]);
  });

  it("filters scope and search without mutating the source array", () => {
    const current = makeDeal("Current Deal", "prepare");
    const historic = makeDeal("Historic Deal", "closed");
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
      makeDeal("unknown", "follow_ups_analysis"),
      makeDeal("historic", "closed"),
      makeDeal("pipeline", "pipeline"),
      makeDeal("review", "prepare"),
    ].map(buildDealPortfolioView);

    const lanes = buildDealKanbanLanes(deals);

    expect(lanes.map((lane) => lane.title)).toEqual([
      "Pipeline",
      "Prepare",
      "Review & plan discovery",
      "Interviews & site visit (SV)",
      "Debrief & flash report",
      "Follow-ups & analysis",
      "QA, readout & closeout",
      "Closed",
      "Pencils Down",
    ]);
    expect(lanes.find((lane) => lane.id === "closed")?.deals.map((deal) => deal.id)).toEqual(["historic"]);
    expect(lanes.flatMap((lane) => lane.deals)).toHaveLength(deals.length);
  });

  it("creates CSS-safe deterministic transition names for arbitrary identifiers", () => {
    expect(getDealTransitionName("Deal / 東京")).toMatch(/^deal-surface-[a-z0-9-]+$/);
    expect(getDealTransitionName("Deal / 東京")).toBe(getDealTransitionName("Deal / 東京"));
    expect(getDealTransitionName("Deal / 東京")).not.toBe(getDealTransitionName("Deal / 大阪"));
  });
});
