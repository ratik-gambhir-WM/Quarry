// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DealRoomData } from "@/data/workspace";
import { DealSummaryCard } from "@/components/deal-room/DealSummaryCard";

const deal: DealRoomData = {
  id: "acme",
  keyQuestions: ["What drives growth?", "Which contracts can terminate?"],
  metrics: [],
  name: "Acme Acquisition",
  overviewSubtitle: "Acme Acquisition Due Diligence Overview",
  pendingTasks: [],
  phaseLabel: "Phase 1",
  resources: [
    {
      availability: "available",
      href: "https://example.com/sow",
      id: "sow",
      label: "SOW",
    },
    {
      availability: "available",
      href: "https://example.com/fact-sheet",
      id: "fact-sheet",
      label: "Fact Sheet",
    },
    {
      availability: "available",
      href: "https://northwind.sharepoint.com/sites/acme",
      id: "sharepoint",
      label: "SharePoint VDR",
    },
    {
      availability: "available",
      href: "https://example.com/request-list",
      id: "request-list",
      label: "Request List",
    },
  ],
  sectorLabel: "Industrials",
  stageLabel: "Active",
  summary: "A focused diligence summary.",
  timeline: [],
};

afterEach(cleanup);

describe("DealSummaryCard", () => {
  it("omits the date section from the overview", () => {
    render(
      <DealSummaryCard
        deal={{
          ...deal,
          metrics: [
            { label: "Start date", value: "Unavailable" },
            { label: "Expected close", value: "2026-12-31" },
            { label: "Actual close", value: "Unavailable" },
            { label: "Engagement completed", value: "2026-10-15" },
          ],
        }}
      />,
    );
    expect(screen.queryByLabelText("Deal details")).toBeNull();
    for (const label of [
      "Start date",
      "Expected close",
      "Actual close",
      "Engagement completed",
    ]) {
      expect(screen.queryByText(label)).toBeNull();
    }
  });
  it("renders the overview and resources without the Scope table", () => {
    const { container } = render(<DealSummaryCard deal={deal} />);

    expect(
      screen.getByRole("heading", { level: 1, name: "Acme Acquisition" }),
    ).not.toBeNull();
    expect(screen.getByText("A focused diligence summary.")).not.toBeNull();
    expect(screen.getByText("SOW")).not.toBeNull();
    expect(screen.getByText("Fact Sheet")).not.toBeNull();
    expect(screen.queryByText("No source is currently available")).toBeNull();
    expect(screen.queryByText("Source support coming soon")).toBeNull();
    for (const [label, href] of [
      ["SOW", "https://example.com/sow"],
      ["Fact Sheet", "https://example.com/fact-sheet"],
      ["SharePoint VDR", "https://northwind.sharepoint.com/sites/acme"],
      ["Request List", "https://example.com/request-list"],
    ]) {
      const resourceLink = screen.getByRole("link", {
        name: `Open ${label} in a new tab`,
      });
      expect(resourceLink.getAttribute("href")).toBe(href);
      expect(resourceLink.getAttribute("rel")).toBe("noopener noreferrer");
      expect(within(resourceLink).getByText("Available")).not.toBeNull();
    }

    expect(screen.queryByRole("heading", { name: "Key Questions" })).toBeNull();
    expect(container.querySelector('[data-slot="data-grid"]')).toBeNull();
  });

  it("does not link unavailable resources", () => {
    render(
      <DealSummaryCard
        deal={{
          ...deal,
          keyQuestions: [],
          resources: deal.resources.map((resource) => ({
            ...resource,
            availability: "unavailable",
            href: undefined,
          })),
        }}
      />,
    );

    expect(
      screen.queryByRole("link", {
        name: /SOW|Fact Sheet|SharePoint VDR|Request List/,
      }),
    ).toBeNull();
  });

  it("shows the dashboard while keeping saved deal details separate from sample metrics", () => {
    render(<DealSummaryCard deal={{ ...deal, summaryDetails: [
      { label: "Engagement", value: "Buy-side" },
      { label: "Transaction", value: "Add-on acquisition" },
      { label: "Target", value: "Actual target" },
      { label: "Buyer", value: "Actual buyer" },
      { label: "Scope", value: "Corporate IT, SDLC" },
    ] }} />);
    const details = screen.getByLabelText("Engagement overview");
    for (const value of ["Buy-side", "Add-on acquisition", "Actual target", "Actual buyer", "Corporate IT, SDLC"]) {
      expect(within(details).getByText(value)).toBeTruthy();
    }
    expect(screen.getByText("Sample data")).toBeTruthy();
    for (const name of ["Assessment Progress", "Deal Resources", "Action Required", "Upcoming Milestones"]) {
      expect(screen.getByRole("heading", { name })).toBeTruthy();
    }
    const statuses = screen.getByRole("list", { name: "Sample assessment status by scope" });
    expect(within(statuses).queryByRole("progressbar")).toBeNull();
    expect(within(statuses).queryByText(/%/)).toBeNull();
    const rows = within(statuses).getAllByRole("listitem");
    for (const [index, stage] of ["Analysis", "Discovery", "QA", "Not Started"].entries()) {
      expect(within(rows[index]).getByText(stage)).toBeTruthy();
    }
    expect(screen.queryByRole("button", { name: "Edit deal" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Manage resources" })).toBeNull();
  });

  it("connects overview edit and resource management to the provided actions", () => {
    const onEdit = vi.fn();
    const onAddResources = vi.fn();
    render(<DealSummaryCard deal={deal} onEdit={onEdit} onAddResources={onAddResources} />);
    fireEvent.click(screen.getByRole("button", { name: "Edit deal" }));
    fireEvent.click(screen.getByRole("button", { name: "Manage resources" }));
    expect(onEdit).toHaveBeenCalledOnce();
    expect(onAddResources).toHaveBeenCalledOnce();
  });

});
