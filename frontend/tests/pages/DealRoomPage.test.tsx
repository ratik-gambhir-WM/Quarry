// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { DealRoomPage } from "@/pages/DealRoomPage";

const { listTemplatePreviews, useWorkspaceDealsMock } = vi.hoisted(() => ({
  listTemplatePreviews: vi.fn(),
  useWorkspaceDealsMock: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { listTemplatePreviews } },
}));

vi.mock("@/hooks/useWorkspaceDeals", () => ({
  useWorkspaceDeals: useWorkspaceDealsMock,
}));

vi.mock("@/hooks/useWorkspaceSession", () => ({
  useWorkspaceSession: () => ({ email: "analyst@example.com", navigationState: {} }),
}));

vi.mock("@/components/hub/WorkspaceSidebar", () => ({
  WorkspaceSidebar: () => <aside>Deal room sidebar</aside>,
}));

vi.mock("@/components/deal-room/DeliverableTemplatesView", () => ({
  DeliverableTemplatesView: () => <div>Template gallery</div>,
}));

afterEach(cleanup);

describe("DealRoomPage", () => {
  beforeEach(() => {
    useWorkspaceDealsMock.mockReturnValue({ deals: workspaceDeals, loaded: true });
    listTemplatePreviews.mockResolvedValue({
      pagination: {
        hasNextPage: false,
        hasPreviousPage: false,
        page: 1,
        pageSize: 10,
        totalItems: 0,
        totalPages: 0,
      },
      previews: [],
    });
  });

  it("renders the shared workspace skeleton while the deal request is pending", () => {
    useWorkspaceDealsMock.mockReturnValue({ deals: [], loaded: false });

    render(
      <MemoryRouter initialEntries={["/hub/deals/pending-deal"]}>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId" />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByText("Loading deal")).not.toBeNull();
    expect(document.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
  });

  it("renders the requested section order and keeps file summary content in its own tab", async () => {
    const user = userEvent.setup({ skipHover: true });
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha"]}>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId" />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([
      "Overview",
      "File Summary",
      "Evidence",
      "Findings",
      "Data Points",
      "Open Items",
      "History",
    ]);
    expect(screen.getByRole("heading", { level: 1, name: "Project Alpha" })).not.toBeNull();
    expect(screen.queryByText("Analyzed Files")).toBeNull();
    expect(screen.getByRole("tab", { name: "Evidence, coming soon" })).toHaveProperty("disabled", true);

    const overviewTab = screen.getByRole("tab", { name: "Overview" });
    const fileSummaryTab = screen.getByRole("tab", { name: "File Summary" });
    overviewTab.focus();
    await user.keyboard("{ArrowRight}");

    expect(document.activeElement).toBe(fileSummaryTab);
    expect(screen.getByText("Analyzed Files")).not.toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "Project Alpha" })).not.toBeNull();
    expect(screen.queryByText(workspaceDeals[0]?.room.summary ?? "")).toBeNull();
    expect(document.querySelector('[data-slot="data-grid"]')).not.toBeNull();
  });

  it("navigates from the two-section Deliverables page to Templates and back", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/deliverables"]}>
        <Routes>
          <Route
            element={<DealRoomPage initialView="deliverables" />}
            path="/hub/deals/:dealId/deliverables"
          />
          <Route
            element={<DealRoomPage initialView="deliverable-templates" />}
            path="/hub/deals/:dealId/deliverables/templates"
          />
        </Routes>
      </MemoryRouter>,
    );

    expect(screen.getByRole("region", { name: "Completed Slide(s)" })).not.toBeNull();
    expect(screen.getByRole("region", { name: "In-progress slides" })).not.toBeNull();
    expect(screen.queryByText("Start from templates")).toBeNull();

    await user.click(screen.getByRole("button", { name: "View Templates" }));
    expect(screen.getByRole("heading", { level: 1, name: "Templates" })).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Back to Deliverables" }));
    expect(screen.getByRole("heading", { level: 1, name: "Deliverables" })).not.toBeNull();
  });
});
