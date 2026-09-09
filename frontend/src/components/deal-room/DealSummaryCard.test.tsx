// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import type { DealRoomData } from "../../data/workspace";
import { DealSummaryCard } from "./DealSummaryCard";

const deal: DealRoomData = {
  id: "acme",
  keyQuestions: ["What drives growth?", "Which contracts can terminate?"],
  metrics: [],
  name: "Acme Acquisition",
  overviewSubtitle: "Acme Acquisition Due Diligence Overview",
  pendingTasks: [],
  phaseLabel: "Phase 1",
  resources: [
    { availability: "unavailable", id: "sow", label: "SOW" },
    { availability: "coming-soon", id: "fact-sheet", label: "Fact Sheet" },
    {
      availability: "available",
      href: "https://northwind.sharepoint.com/sites/acme",
      id: "sharepoint",
      label: "SharePoint VDR",
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
    const sowRow = screen.getByText("SOW").closest('[role="listitem"]');
    expect(sowRow).not.toBeNull();
    expect(within(sowRow as HTMLElement).getByText("SOW").parentElement).toBe(
      within(sowRow as HTMLElement).getByText("Unavailable").parentElement,
    );
    const sharepointLink = screen.getByRole("link", { name: "Open SharePoint VDR in a new tab" });
    expect(sharepointLink.getAttribute("href")).toBe("https://northwind.sharepoint.com/sites/acme");
    expect(sharepointLink.getAttribute("rel")).toBe("noopener noreferrer");

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
    expect(screen.queryByRole("link", { name: /SOW|Fact Sheet|SharePoint VDR/ })).toBeNull();
  });

  it("renders every extracted question without hidden grid pagination", () => {
    const questions = Array.from({ length: 12 }, (_, index) => `Question ${index + 1}?`);

    render(<DealSummaryCard deal={{ ...deal, keyQuestions: questions }} />);

    expect(screen.getByText("Question 12?")).not.toBeNull();
  });
});
