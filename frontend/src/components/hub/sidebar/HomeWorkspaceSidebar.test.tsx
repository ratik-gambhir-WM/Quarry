import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { workspaceDeals, workspaceTools } from "../../../data/workspace";
import { HomeWorkspaceSidebar } from "./HomeWorkspaceSidebar";

describe("HomeWorkspaceSidebar", () => {
  it("renders the canonical portfolio hierarchy and keeps Logs in the bottom utility group", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/hub/deals"]}>
        <HomeWorkspaceSidebar
          activeHomeSection="deals"
          deals={workspaceDeals}
          initiatives={[]}
          tools={workspaceTools}
        />
      </MemoryRouter>,
    );

    const labels = ["Vault", "Research", "Explore", "Topics", "Workspace", "Deals", "Notebook", "Templates", "Logs"];
    for (let index = 1; index < labels.length; index += 1) {
      expect(markup.indexOf(`>${labels[index - 1]}<`)).toBeLessThan(markup.indexOf(`>${labels[index]}<`));
    }

    expect(markup).toContain('href="/hub/deals"');
    expect(markup).toContain('aria-current="page"');
    expect(markup).toContain('href="/hub/logs"');
    expect(markup).toContain('aria-label="Switch sidebar. Current sidebar: Deal Hub"');
    expect(markup).toContain('d="M2.97 12.92');
    expect(markup).toContain('d="m10.065 12.493');
    expect(markup).toContain('d="M12 6.04168C10.4077');
    expect(markup).not.toContain("Active Deals");
    expect(markup).not.toContain("Active deals actions");
    expect(markup).not.toContain('href="/hub/deals/project-alpha"');
    expect(markup).not.toContain("Quick Chat");
  });
});
