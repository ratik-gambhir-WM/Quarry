// @vitest-environment happy-dom

import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DealRoomWorkspaceSidebar } from "@/components/hub/sidebar/DealRoomWorkspaceSidebar";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";

afterEach(cleanup);

function CurrentLocation() {
  const location = useLocation();
  return (
    <output aria-label="Current location">
      {location.pathname} {location.state?.email}
    </output>
  );
}

describe("DealRoomWorkspaceSidebar", () => {
  it("orders URL-backed sidebar links and keeps only the active route current", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter
        initialEntries={["/hub/deals/project-alpha/deliverables/templates"]}
      >
        <DealRoomWorkspaceSidebar
          activeDealId="project-alpha"
          activeSection="deliverables"
          deals={workspaceDeals}
        />
      </MemoryRouter>,
    );

    const deliverableLink = markup.match(
      /<a[^>]*aria-label="Deliverable"[^>]*>.*?<\/a>/u,
    )?.[0];
    const dealRoomIndex = markup.indexOf('aria-label="Deal Room"');
    const artifactsSectionIndex = markup.indexOf(">Deal Artifacts</h2>");
    const dataRoomIndex = markup.indexOf('aria-label="Data Room"');
    const deliverableIndex = markup.indexOf('aria-label="Deliverable"');
    const analysisIndex = markup.indexOf('aria-label="Analysis"');

    expect(markup).toContain('href="/hub/deals/project-alpha/deliverables"');
    expect(markup).not.toContain('aria-label="Deal Activity"');
    expect(markup).toContain('href="/hub/deals/project-alpha/analysis"');
    expect(deliverableLink).toContain("bg-sidebar-selected");
    expect(markup.match(/aria-current="page"/gu)).toHaveLength(1);
    expect(dealRoomIndex).toBeLessThan(artifactsSectionIndex);
    expect(artifactsSectionIndex).toBeLessThan(dataRoomIndex);
    expect(dataRoomIndex).toBeLessThan(analysisIndex);
    expect(analysisIndex).toBeLessThan(deliverableIndex);
    expect(markup).not.toContain('aria-label="Data Room Vault"');
    expect(markup).not.toContain('aria-label="Artifacts"');
    expect(markup).not.toContain('aria-label="Site Visits"');
  });
  it("switches to another active deal while preserving navigation state", async () => {
    const user = userEvent.setup();
    const base = workspaceDeals[0];
    const deals = [
      { ...base, room: { ...base.room, id: "current", name: "Current deal" } },
      {
        ...base,
        room: { ...base.room, id: "other deal", name: "Other active deal" },
        portfolio: { ...base.portfolio, status: "prepare" as const },
      },
      {
        ...base,
        room: { ...base.room, id: "closed", name: "Closed deal" },
        portfolio: { ...base.portfolio, status: "closed" as const },
      },
      {
        ...base,
        room: { ...base.room, id: "paused", name: "Pencils down deal" },
        portfolio: { ...base.portfolio, status: "pencils_down" as const },
      },
    ];
    render(
      <MemoryRouter initialEntries={["/hub/deals/current/analysis"]}>
        <DealRoomWorkspaceSidebar
          activeDealId="current"
          activeSection="analysis"
          deals={deals}
          navigationState={{ email: "analyst@example.com" }}
        />
        <CurrentLocation />
      </MemoryRouter>,
    );
    const trigger = screen.getByRole("button", { name: "Switch deal room" });
    await user.click(trigger);
    expect(screen.getAllByRole("menuitem")).toHaveLength(1);
    expect(
      screen
        .getByRole("menuitem", { name: "Other active deal" })
        .getAttribute("href"),
    ).toBe("/hub/deals/other%20deal");
    await user.keyboard("{Escape}");
    expect(document.activeElement).toBe(trigger);
    await user.keyboard("{Enter}");
    await user.click(
      screen.getByRole("menuitem", { name: "Other active deal" }),
    );
    expect(screen.getByLabelText("Current location").textContent).toBe(
      "/hub/deals/other%20deal analyst@example.com",
    );
    expect(screen.queryByRole("menu")).toBeNull();
  });

  it("shows an empty state when there are no other active deals", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <DealRoomWorkspaceSidebar
          activeDealId={workspaceDeals[0].room.id}
          activeSection="deal-room"
          deals={[workspaceDeals[0]]}
        />
      </MemoryRouter>,
    );
    await user.click(screen.getByRole("button", { name: "Switch deal room" }));
    expect(screen.getByText("No other active deals.")).not.toBeNull();
    expect(screen.queryByRole("menuitem")).toBeNull();
  });
});
