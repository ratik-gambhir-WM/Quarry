// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";
import { SidebarFrame } from "@/components/hub/sidebar/SidebarFrame";

describe("SidebarFrame", () => {
  afterEach(cleanup);

  it("renders the active sidebar identity without a mock switcher", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter>
        <SidebarFrame
          alignedHeader
          showHeaderBackButton={false}
          sidebarIcon="home"
          sidebarLabel="Deal Hub"
        >
          <nav>Current navigation</nav>
        </SidebarFrame>
      </MemoryRouter>,
    );

    expect(markup).toContain("Deal Hub");
    expect(markup).not.toContain("Switch sidebar");
    expect(markup).toContain("relative flex h-12 shrink-0 items-center gap-2");
    expect(markup).not.toContain("border-b border-outline-variant/70");
    expect(markup.indexOf("Deal Hub")).toBeLessThan(markup.indexOf("Current navigation"));
  });

  it("centers the collapsed header and navigation controls on one rail", async () => {
    const user = userEvent.setup();

    render(
      <MemoryRouter>
        <SidebarFrame alignedHeader showHeaderBackButton={false}>
          <nav>
            <a href="/example">
              <svg aria-hidden="true" />
              <span>Example</span>
            </a>
          </nav>
        </SidebarFrame>
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("button", { name: "Collapse sidebar" }));

    const expandButton = screen.getByRole("button", { name: /^Expand sidebar$/ });
    const navigation = document.getElementById("workspace-sidebar-navigation");

    expect(expandButton.classList.contains("mx-auto")).toBe(true);
    expect(navigation?.className).toContain("[&_nav>a]:justify-center");
    expect(navigation?.className).toContain("[&_nav>button]:justify-center");
  });
});
