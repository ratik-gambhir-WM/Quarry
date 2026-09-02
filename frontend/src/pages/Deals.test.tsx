// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { Deals } from "./Deals";

const { listDeals } = vi.hoisted(() => ({ listDeals: vi.fn() }));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { listDeals },
    platform: {},
    target: "web",
  },
}));

describe("Deals", () => {
  beforeAll(() => {
    const css = window.CSS ?? {};
    Object.defineProperty(window, "CSS", { configurable: true, value: css });
    Object.defineProperty(css, "escape", {
      configurable: true,
      value: (value: string) => value.replace(/[^a-zA-Z0-9_-]/g, "-"),
    });
  });

  afterEach(cleanup);

  beforeEach(() => {
    listDeals.mockClear();
    listDeals.mockResolvedValue([]);
    window.sessionStorage.clear();
  });

  it("defaults to the portfolio table and combines lifecycle and search filters", async () => {
    const user = userEvent.setup({ skipHover: true });
    renderDeals();

    const table = screen.getByRole("table");
    expect(table).toBeTruthy();
    expect(table.closest(".workspace-card")).toBeNull();
    expect(table.querySelectorAll('tbody [aria-hidden="true"]')).toHaveLength(0);
    expect(screen.getByRole("link", { name: "Open Project Alpha" })).toBeTruthy();
    const tableViewButton = screen.getByRole("button", { name: "Table view", pressed: true });
    expect(tableViewButton.closest("header")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Kanban view", pressed: false })).toBeTruthy();
    expect(screen.queryByRole("searchbox", { name: "Search deals" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "Filter deals by lifecycle" }));
    await user.click(await screen.findByRole("menuitemradio", { name: "Current" }));
    expect(screen.queryByRole("link", { name: "Open Project Alpha" })).toBeNull();
    expect(screen.getByRole("link", { name: "Open Project Beta" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "Search deals" }));
    const searchbox = screen.getByRole("searchbox", { name: "Search deals" });
    expect(searchbox).toBe(document.activeElement);
    await user.type(searchbox, "no matching sponsor");
    expect(await screen.findByText("No deals match these filters")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Reset filters" }));
    expect(await screen.findByRole("link", { name: "Open Project Alpha" })).toBeTruthy();
  });

  it("moves the single add-deal control out of the sidebar and restores focus after Escape", async () => {
    const user = userEvent.setup({ skipHover: true });
    renderDeals();
    const trigger = screen.getByRole("button", { name: "Deal portfolio actions" });

    expect(trigger.closest("header")).toBeTruthy();
    expect(trigger.querySelector('path[d="M5 12h14"]')).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Active deals actions" })).toBeNull();
    await user.click(trigger);
    await user.click(await screen.findByRole("menuitem", { name: "Add deal" }));

    const dialog = await screen.findByRole("dialog", { name: "Add deal" });
    await waitFor(() => expect(within(dialog).getByLabelText("Deal ID")).toBe(document.activeElement));

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add deal" })).toBeNull());
    await waitFor(() => expect(trigger).toBe(document.activeElement));
  });

  it("closes the expanded deal search from its trailing control and restores trigger focus", async () => {
    const user = userEvent.setup({ skipHover: true });
    renderDeals();
    const trigger = screen.getByRole("button", { name: "Search deals" });

    expect(screen.queryByRole("button", { name: "Close deal search" })).toBeNull();
    await user.click(trigger);

    const searchbox = screen.getByRole("searchbox", { name: "Search deals" });
    const closeButton = screen.getByRole("button", { name: "Close deal search" });
    expect(closeButton.parentElement?.lastElementChild).toBe(closeButton);
    await user.type(searchbox, "Project Beta");
    await user.click(closeButton);

    expect(screen.queryByRole("searchbox", { name: "Search deals" })).toBeNull();
    expect(screen.getByRole("link", { name: "Open Project Alpha" })).toBeTruthy();
    await waitFor(() => expect(trigger).toBe(document.activeElement));

    await user.click(trigger);
    expect((screen.getByRole("searchbox", { name: "Search deals" }) as HTMLInputElement).value).toBe("");
  });

  it("lazy-loads the read-only kanban and keeps cards keyboard actionable", async () => {
    const user = userEvent.setup({ skipHover: true });
    renderDeals();

    await user.click(screen.getByRole("button", { name: "Kanban view" }));
    expect(screen.getByRole("button", { name: "Table view", pressed: false })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Kanban view", pressed: true })).toBeTruthy();
    expect(await screen.findByRole("region", { name: "Deals by status" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /add column/i })).toBeNull();

    const alphaCard = screen.getByRole("link", { name: /Project Alpha/i });
    alphaCard.focus();
    await user.keyboard(" ");
    await waitFor(() => expect(screen.getByTestId("location").textContent).toBe("/hub/deals/project-alpha"));
  });
});

function renderDeals() {
  render(
    <MemoryRouter initialEntries={[{ pathname: "/hub/deals", state: { email: "analyst@example.com" } }]}>
      <Deals />
      <LocationProbe />
    </MemoryRouter>,
  );
}

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname}</output>;
}
