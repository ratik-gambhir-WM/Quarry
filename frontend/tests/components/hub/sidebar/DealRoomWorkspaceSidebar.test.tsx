import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { DealRoomWorkspaceSidebar } from "@/components/hub/sidebar/DealRoomWorkspaceSidebar";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";

describe("DealRoomWorkspaceSidebar", () => {
  it("orders URL-backed sidebar links and keeps only the active route current", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/deliverables/templates"]}>
        <DealRoomWorkspaceSidebar
          activeDealId="project-alpha"
          activeSection="deliverables"
          deals={workspaceDeals}
        />
      </MemoryRouter>,
    );

    const deliverableLink = markup.match(/<a[^>]*aria-label="Deliverable"[^>]*>.*?<\/a>/u)?.[0];
    const dealRoomIndex = markup.indexOf('aria-label="Deal Room"');
    const dealActivityIndex = markup.indexOf('aria-label="Deal Activity"');
    const artifactsSectionIndex = markup.indexOf(">Deal Artifacts</h2>");
    const dataRoomIndex = markup.indexOf('aria-label="Data Room"');
    const deliverableIndex = markup.indexOf('aria-label="Deliverable"');
    const analysisIndex = markup.indexOf('aria-label="Analysis"');

    expect(markup).toContain('href="/hub/deals/project-alpha/deliverables"');
    expect(markup).toContain('href="/hub/deals/project-alpha/activity"');
    expect(markup).toContain('href="/hub/deals/project-alpha/analysis"');
    expect(deliverableLink).toContain("bg-sidebar-selected");
    expect(markup.match(/aria-current="page"/gu)).toHaveLength(1);
    expect(dealRoomIndex).toBeLessThan(dealActivityIndex);
    expect(dealActivityIndex).toBeLessThan(artifactsSectionIndex);
    expect(artifactsSectionIndex).toBeLessThan(dataRoomIndex);
    expect(dataRoomIndex).toBeLessThan(analysisIndex);
    expect(analysisIndex).toBeLessThan(deliverableIndex);
    expect(markup).not.toContain('aria-label="Data Room Vault"');
    expect(markup).not.toContain('aria-label="Artifacts"');
    expect(markup).not.toContain('aria-label="Site Visits"');
  });
});
