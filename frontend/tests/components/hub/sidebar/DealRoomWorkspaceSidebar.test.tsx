import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { DealRoomWorkspaceSidebar } from "@/components/hub/sidebar/DealRoomWorkspaceSidebar";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";

describe("DealRoomWorkspaceSidebar", () => {
  it("orders the sidebar links and renders the requested animated icons", () => {
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
    const dealActivityItem = markup.match(/<button[^>]*aria-label="Deal Activity"[^>]*>.*?<\/button>/u)?.[0];
    const artifactsSectionIndex = markup.indexOf(">Deal Artifacts</h2>");
    const dataRoomIndex = markup.indexOf('aria-label="Data Room"');
    const deliverableIndex = markup.indexOf('aria-label="Deliverable"');
    const analysisIndex = markup.indexOf('aria-label="Analysis"');
    const analysisItem = markup.match(/<button[^>]*aria-label="Analysis"[^>]*>.*?<\/button>/u)?.[0];
    const dataRoomLink = markup.match(/<a[^>]*aria-label="Data Room"[^>]*>.*?<\/a>/u)?.[0];

    expect(markup).toContain('href="/hub/deals/project-alpha/deliverables"');
    expect(deliverableLink).toContain("bg-sidebar-selected");
    expect(deliverableLink).toContain('d="M12 2v3"');
    expect(dataRoomLink).toContain('d="M3 3v13a2 2 0 0 0 2 2h3"');
    expect(markup).toContain('aria-current="page"');
    expect(dealActivityItem).toContain('d="M3 10h18"');
    expect(dealRoomIndex).toBeLessThan(dealActivityIndex);
    expect(dealActivityIndex).toBeLessThan(artifactsSectionIndex);
    expect(artifactsSectionIndex).toBeLessThan(dataRoomIndex);
    expect(dataRoomIndex).toBeLessThan(analysisIndex);
    expect(analysisIndex).toBeLessThan(deliverableIndex);
    expect(analysisItem).toContain('d="M21 7h-3a2 2 0 0 1-2-2V2"');
    expect(markup).not.toContain('aria-label="Data Room Vault"');
    expect(markup).not.toContain('aria-label="Artifacts"');
    expect(markup).not.toContain('aria-label="Site Visits"');
  });
});
