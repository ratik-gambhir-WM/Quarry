// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
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
  it("renders the overview, resources, and extracted questions in response order", () => {
    const { container } = render(<DealSummaryCard deal={deal} />);

    expect(screen.getByRole("heading", { level: 1, name: "Acme Acquisition" })).not.toBeNull();
    expect(screen.getByText("A focused diligence summary.")).not.toBeNull();
    expect(screen.getByText("SOW")).not.toBeNull();
    expect(screen.getByText("Fact Sheet")).not.toBeNull();
    expect(screen.getByText("Questions extracted from submitted SOW")).not.toBeNull();
    const keyQuestionsHeading = screen.getByRole("heading", { level: 2, name: "Key Questions" });
    expect(keyQuestionsHeading.parentElement?.parentElement?.querySelector("svg")).toBeNull();
    expect(screen.queryByText("No source is currently available")).toBeNull();
    expect(screen.queryByText("Source support coming soon")).toBeNull();
    for (const [label, href] of [
      ["SOW", "https://example.com/sow"],
      ["Fact Sheet", "https://example.com/fact-sheet"],
      ["SharePoint VDR", "https://northwind.sharepoint.com/sites/acme"],
      ["Request List", "https://example.com/request-list"],
    ]) {
      const resourceLink = screen.getByRole("link", { name: `Open ${label} in a new tab` });
      expect(resourceLink.getAttribute("href")).toBe(href);
      expect(resourceLink.getAttribute("rel")).toBe("noopener noreferrer");
      expect(within(resourceLink).getByText("Available")).not.toBeNull();
    }

    const markup = container.textContent ?? "";
    expect(markup.indexOf("What drives growth?")).toBeLessThan(markup.indexOf("Which contracts can terminate?"));
    expect(container.querySelector('[data-slot="data-grid"]')).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Question" })).toBeNull();
  });

  it("renders an explicit empty state and does not link unavailable resources", () => {
    render(
      <DealSummaryCard
        deal={{
          ...deal,
          keyQuestions: [],
          resources: deal.resources.map((resource) => ({ ...resource, availability: "unavailable", href: undefined })),
        }}
      />,
    );

    expect(screen.getByText("No key questions were extracted from the submitted source files.")).not.toBeNull();
    expect(screen.queryByRole("link", { name: /SOW|Fact Sheet|SharePoint VDR|Request List/ })).toBeNull();
  });

  it("renders every extracted question without hidden grid pagination", () => {
    const questions = Array.from({ length: 12 }, (_, index) => `Question ${index + 1}?`);

    render(<DealSummaryCard deal={{ ...deal, keyQuestions: questions }} />);

    expect(screen.getByText("Question 12?")).not.toBeNull();
  });
});
