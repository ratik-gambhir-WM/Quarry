// @vitest-environment happy-dom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceProvider } from "@/app/WorkspaceProvider";
import { Deals } from "@/pages/Deals";

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

  it("defaults to the portfolio table and keeps view choices in one focused menu", async () => {
    const user = userEvent.setup({ skipHover: true });
    renderDeals();

    const table = await screen.findByRole("table");
    expect(table).toBeTruthy();
    const pageHeader = screen.getByRole("heading", { name: "Deals" }).closest("header");
    const portfolioCounts = screen.getByLabelText("Deal portfolio counts");
    expect(pageHeader?.contains(portfolioCounts)).toBe(true);
    expect(portfolioCounts.textContent).toBe("3 total2 current1 historic");
    expect(screen.queryByRole("heading", { name: "All deals" })).toBeNull();
    expect(table.closest(".workspace-card")).toBeNull();
    const tableFrame = table.closest('[data-slot="data-grid"]')?.parentElement;
    const dealsContent = tableFrame?.parentElement;
    expect(dealsContent?.classList.contains("w-full")).toBe(true);
    expect(Array.from(dealsContent?.classList ?? []).some((className) => className.startsWith("max-w-"))).toBe(false);
    expect(tableFrame?.classList.contains("border-y")).toBe(true);
    expect(tableFrame?.classList.contains("-mx-8")).toBe(true);
    expect(tableFrame?.classList.contains("-mt-8")).toBe(true);
    expect(tableFrame?.classList.contains("w-[calc(100%+4rem)]")).toBe(true);
    expect(tableFrame?.classList.contains("rounded-2xl")).toBe(false);
    expect(tableFrame?.classList.contains("bg-surface-container-lowest")).toBe(false);
    expect(table.classList.contains("bg-[var(--theme-workspace-surface)]")).toBe(true);
    expect(table.classList.contains("text-center")).toBe(true);
    expect(table.querySelectorAll('tbody tr[aria-hidden="true"]')).toHaveLength(0);
    expect(screen.getByRole("link", { name: "Open Project Alpha" })).toBeTruthy();
    const viewMenuTrigger = screen.getByRole("button", { name: "Change deals view" });
    expect(viewMenuTrigger.closest("header")).toBeTruthy();
    expect(viewMenuTrigger.classList.contains("border-0")).toBe(true);
    expect(viewMenuTrigger.classList.contains("shadow-none")).toBe(true);
    expect(viewMenuTrigger.classList.contains("bg-transparent")).toBe(true);
    expect(screen.queryByRole("button", { name: "Table view" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Kanban view" })).toBeNull();
    expect(screen.queryByRole("searchbox", { name: "Search deals" })).toBeNull();

    await user.click(viewMenuTrigger);
    const listOption = await screen.findByRole("menuitemradio", { name: "List" });
    const kanbanOption = screen.getByRole("menuitemradio", { name: "Kanban" });
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(2);
    expect(listOption.getAttribute("aria-checked")).toBe("true");
    expect(kanbanOption.getAttribute("aria-checked")).toBe("false");
    expect(screen.queryByText("Grouping")).toBeNull();
    expect(screen.queryByText("Ordering")).toBeNull();
    expect(screen.queryByText("Show closed projects")).toBeNull();
    await user.keyboard("{Escape}");

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
    const trigger = screen.getByRole("button", { name: "New" });

    expect(trigger.closest("header")).toBeTruthy();
    expect(trigger.textContent).toBe("New");
    expect(trigger.querySelector('path[d="M5 12h14"]')).toBeTruthy();
    expect(trigger.closest("header")?.querySelector('[data-slot="avatar"]')).toBeNull();
    expect(screen.queryByRole("button", { name: "Active deals actions" })).toBeNull();
    await user.click(trigger);
    const addDealItem = await screen.findByRole("menuitem", { name: "Add deal" });
    expect(screen.getByText("Create")).toBeTruthy();
    expect(screen.getAllByRole("menuitem")).toHaveLength(1);
    await user.click(addDealItem);

    const dialog = await screen.findByRole("dialog", { name: "Add deal" });
    const dealIdInput = within(dialog).getByLabelText("Deal ID");
    expect(dialog.getAttribute("data-slot")).toBe("dialog-content");
    expect(document.querySelector('[data-slot="dialog-overlay"]')).toBeTruthy();
    expect(document.querySelector(".modal-backdrop")).toBeNull();
    expect(dealIdInput.getAttribute("data-slot")).toBe("input");
    expect(within(dialog).getByLabelText("Status").textContent).toContain("Active");
    for (const label of [
      "Deal name",
      "Start date",
      "Close date",
      "Transaction type",
      "Target company",
      "Primary buyer",
      "Deal sponsor",
      "SharePoint link",
    ]) {
      expect(within(dialog).getByLabelText(label)).toBeTruthy();
    }
    expect(within(dialog).getByLabelText("SharePoint link").getAttribute("placeholder")).toBe(
      "https://westmonroe.sharepoint.com/sites/ClientTeamYYYY-Project/Shared%20Documents/Forms/AllItems.aspx?FolderCTID=0x...&id=%2Fsites%2FClientTeamYYYY-Project%2FShared%20Documents",
    );
    await waitFor(() => expect(dealIdInput).toBe(document.activeElement));

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

    const viewMenuTrigger = screen.getByRole("button", { name: "Change deals view" });
    await user.click(viewMenuTrigger);
    await user.click(await screen.findByRole("menuitemradio", { name: "Kanban" }));
    expect(await screen.findByRole("region", { name: "Deals by status" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /add column/i })).toBeNull();

    await user.click(viewMenuTrigger);
    expect(screen.getByRole("menuitemradio", { name: "Kanban" }).getAttribute("aria-checked")).toBe("true");
    await user.keyboard("{Escape}");

    const alphaCard = screen.getByRole("link", { name: /Project Alpha/i });
    alphaCard.focus();
    await user.keyboard(" ");
    await waitFor(() => expect(screen.getByTestId("location").textContent).toBe("/hub/deals/project-alpha"));
  });
});

function renderDeals() {
  render(
    <MemoryRouter initialEntries={[{ pathname: "/hub/deals", state: { email: "analyst@example.com" } }]}>
      <WorkspaceProvider dataSource="demo">
        <Deals />
        <LocationProbe />
      </WorkspaceProvider>
    </MemoryRouter>,
  );
}

function LocationProbe() {
  const location = useLocation();
  return <output data-testid="location">{location.pathname}</output>;
}
