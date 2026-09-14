// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, useNavigate } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import type { PersistedDeal } from "@/contracts/quarryApi";

const { listDeals } = vi.hoisted(() => ({ listDeals: vi.fn() }));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { listDeals },
    platform: {},
    target: "web",
  },
}));

afterEach(cleanup);

describe("App routes", () => {
  beforeEach(() => {
    listDeals.mockReset();
    listDeals.mockResolvedValue([persistedDeal]);
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("keeps Login eager without starting workspace data loading", () => {
    render(
      <MemoryRouter initialEntries={["/login"]}>
        <App />
      </MemoryRouter>,
    );

    expect(screen.getByRole("heading", { name: "Quarry" })).toBeTruthy();
    expect(listDeals).not.toHaveBeenCalled();
  });

  it("supports direct entry to URL-backed Deal Activity", async () => {
    render(
      <MemoryRouter
        initialEntries={["/hub/deals/project-alpha", "/hub/deals/project-alpha/activity"]}
        initialIndex={1}
      >
        <App />
        <HistoryControls />
      </MemoryRouter>,
    );

    expect(await screen.findByRole("link", { name: "Deal Activity", current: "page" })).toBeTruthy();
    expect((await screen.findAllByRole("heading", { name: "Deal Activity" })).length).toBeGreaterThan(0);
    expect(listDeals).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(await screen.findByRole("link", { name: "Deal Room", current: "page" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Forward" }));
    expect(await screen.findByRole("link", { name: "Deal Activity", current: "page" })).toBeTruthy();
    expect(listDeals).toHaveBeenCalledTimes(1);
  });
});

function HistoryControls() {
  const navigate = useNavigate();
  return (
    <>
      <button onClick={() => navigate(-1)} type="button">Back</button>
      <button onClick={() => navigate(1)} type="button">Forward</button>
    </>
  );
}

const persistedDeal: PersistedDeal = {
  closeDate: "2026-12-31",
  dealId: "project-alpha",
  dealName: "Project Alpha",
  dealSponsor: "Sponsor",
  metadata: null,
  primaryBuyer: "Buyer",
  startDate: "2026-09-01",
  status: "Active",
  targetCompany: "Target",
  transactionType: "Buy-side",
  userId: 1,
};
