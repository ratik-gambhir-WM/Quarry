// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SidebarSwitcher } from "./SidebarSwitcher";

describe("SidebarSwitcher", () => {
  afterEach(cleanup);

  it("renders the same destinations as a divided text-only menu", async () => {
    const user = userEvent.setup();
    const onSpaceChange = vi.fn();

    render(
      <SidebarSwitcher
        activeSpaceId="current"
        currentIcon="home"
        currentLabel="Deal Hub"
        onSpaceChange={onSpaceChange}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Switch sidebar. Current sidebar: Deal Hub" }));

    const menu = screen.getByRole("menu", { name: "Switch sidebar" });
    const destinations = within(menu).getAllByRole("menuitemradio");

    expect(destinations).toHaveLength(4);
    expect(destinations.map((destination) => destination.textContent)).toEqual([
      "Deal HubCurrent page navigation",
      "DiligenceWorkstreams and findings",
      "ResearchResearch and saved evidence",
      "OperationsTeam workflows and tools",
    ]);
    expect(menu.querySelectorAll("svg")).toHaveLength(0);
    expect(menu.classList.contains("divide-y")).toBe(true);
    expect(destinations.every((destination) => destination.classList.contains("py-1.5"))).toBe(true);
    expect(menu.parentElement?.classList.contains("rounded-[22px]")).toBe(true);
    expect(screen.queryByText("Switch sidebar")).toBeNull();
    expect(screen.queryByText(/Prototype navigation/)).toBeNull();

    await user.click(within(menu).getByRole("menuitemradio", { name: /Diligence/ }));

    expect(onSpaceChange).toHaveBeenCalledWith("diligence");
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
