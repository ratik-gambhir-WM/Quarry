import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";

import { DealRoomWorkspaceSidebar } from "@/components/hub/sidebar/DealRoomWorkspaceSidebar";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";

describe("DealRoomWorkspaceSidebar", () => {
  it("links Artifacts to the deliverables route and keeps it active on the templates page", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/deliverables/templates"]}>
        <DealRoomWorkspaceSidebar
          activeDealId="project-alpha"
          activeSection="deliverables"
          deals={workspaceDeals}
        />
      </MemoryRouter>,
    );

    const artifactsLink = markup.match(/<a[^>]*aria-label="Artifacts"[^>]*>.*?<\/a>/u)?.[0];
    expect(markup).toContain('href="/hub/deals/project-alpha/deliverables"');
    expect(artifactsLink).toContain('fill="currentColor"');
    expect(markup).toContain('aria-current="page"');
  });
});
