import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { workspaceTools } from "@/fixtures/workspace/navigation";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { HomeWorkspaceSidebar } from "@/components/hub/sidebar/HomeWorkspaceSidebar";

describe("HomeWorkspaceSidebar", () => {
  it("renders Assistant below Vault and keeps Workspace and Logs in their respective groups", () => {
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

    const labels = [
      "Home",
      "Vault",
      "Assistant",
      "Workspace",
      "Deals",
      "Notebook",
      "Logs",
    ];
    for (let index = 1; index < labels.length; index += 1) {
      expect(markup.indexOf(`>${labels[index - 1]}<`)).toBeLessThan(
        markup.indexOf(`>${labels[index]}<`),
      );
    }

    expect(markup).toContain('href="/hub"');
    expect(markup).toContain('href="/hub/deals"');
    expect(markup).toContain('aria-current="page"');
    expect(markup).toContain('href="/hub/logs"');
    expect(markup).toContain('href="/hub/assistant"');
    expect(markup).toContain('href="/hub/notebook"');
    expect(markup).toContain('d="M12 7.60279L12.4521 10.5685M15.9543 7L16.4064 9.96574"');
    expect(markup).toContain("Deal Hub");
    expect(markup).not.toContain(">Research<");
    expect(markup).not.toContain(">Topics<");
    expect(markup).not.toContain(">Templates<");
    expect(markup).not.toContain("Switch sidebar");
    expect(markup).not.toContain('d="M8.5 4v16');
    expect(markup).not.toContain("Active Deals");
    expect(markup).not.toContain("Active deals actions");
    expect(markup).not.toContain('href="/hub/deals/project-alpha"');
    expect(markup).not.toContain("Quick Chat");
  });

  it("links Home to the Hub route and marks it active at the route root", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/hub"]}>
        <HomeWorkspaceSidebar
          activeHomeSection="hub"
          deals={workspaceDeals}
          initiatives={[]}
          tools={workspaceTools}
        />
      </MemoryRouter>,
    );

    const homeLink = markup.match(
      /<a\b[^>]*href="\/hub"[^>]*>[\s\S]*?<\/a>/,
    )?.[0];

    expect(homeLink).toContain('aria-current="page"');
    expect(homeLink).toContain(">Home<");
  });

  it("selects the Assistant link with the assistant home-section key", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <HomeWorkspaceSidebar
          activeHomeSection="assistant"
          deals={workspaceDeals}
          initiatives={[]}
          tools={workspaceTools}
        />
      </MemoryRouter>,
    );

    expect(markup).toContain('href="/hub/assistant"');
    expect(markup).toContain('href="/hub/notebook"');
    expect(markup).toContain('aria-current="page"');
    expect(markup).not.toContain('href="/hub/summarize"');
  });
});
