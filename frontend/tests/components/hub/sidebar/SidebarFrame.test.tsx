// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderToStaticMarkup } from "react-dom/server";
import { MemoryRouter } from "react-router-dom";
import { afterEach, describe, expect, it } from "vitest";
import { ThemeModeProvider } from "@/hooks/useThemeMode";
import { SidebarFrame } from "@/components/hub/sidebar/SidebarFrame";

describe("SidebarFrame", () => {
  it("opens the profile menu from the keyboard and restores focus on Escape", async () => {
    const user = userEvent.setup();
    render(<MemoryRouter><ThemeModeProvider><SidebarFrame><nav>Navigation</nav></SidebarFrame></ThemeModeProvider></MemoryRouter>);
    const trigger = screen.getByRole("button", { name: /profile$/ });
    trigger.focus();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("menuitem", { name: "Account info" })).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

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
    expect(markup.indexOf("Deal Hub")).toBeLessThan(
      markup.indexOf("Current navigation"),
    );
  });

  it("allows a route-specific header to replace the standard sidebar identity", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter>
        <SidebarFrame
          alignedHeader
          headerContent={<nav>Deal Hub and Chat tabs</nav>}
          showHeaderBackButton={false}
        >
          <nav>Current navigation</nav>
        </SidebarFrame>
      </MemoryRouter>,
    );

    expect(markup).toContain("Deal Hub and Chat tabs");
    expect(markup).not.toContain(">Workspace<");
    expect(markup.indexOf("Deal Hub and Chat tabs")).toBeLessThan(
      markup.indexOf("Current navigation"),
    );
  });

  it("collapses to a narrow rail with only the expand control and restores content", async () => {
    const user = userEvent.setup();

    render(
      <MemoryRouter>
        <SidebarFrame alignedHeader>
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

    const expandButton = screen.getByRole("button", {
      name: /^Expand sidebar$/,
    });
    expect(expandButton.classList.contains("mx-auto")).toBe(true);
    expect(expandButton.closest("aside")?.classList.contains("w-12")).toBe(true);
    expect(screen.getAllByRole("button")).toEqual([expandButton]);
    expect(screen.queryAllByRole("link")).toHaveLength(0);
    expect(screen.queryByRole("img", { name: "Quarry" })).toBeNull();
    expect(expandButton.getAttribute("aria-expanded")).toBe("false");

    await user.keyboard("{Enter}");
    expect(screen.getByRole("link", { name: "Example" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Back to home page" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /profile$/ })).toBeTruthy();
    expect(expandButton.closest("aside")?.classList.contains("w-72")).toBe(true);
  });

  it("renders its back control as a left arrow to the requested destination", () => {
    const markup = renderToStaticMarkup(
      <MemoryRouter>
        <SidebarFrame
          headerBackLabel="Back to Deal Room"
          headerBackTo="/hub/deals/project-alpha"
        >
          <nav>Data room files</nav>
        </SidebarFrame>
      </MemoryRouter>,
    );

    expect(markup).toContain('aria-label="Back to Deal Room"');
    expect(markup).toContain('href="/hub/deals/project-alpha"');
    expect(markup).toContain('d="M19 12H6"');
    expect(markup).not.toContain("rotate-180");
  });
});
