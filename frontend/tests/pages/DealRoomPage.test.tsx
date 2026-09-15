// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { DealRoomPage } from "@/pages/DealRoomPage";
import { DealRoomOverviewPage } from "@/pages/deal-room/DealRoomOverviewPage";
import { DeliverablesPage } from "@/pages/deal-room/DeliverablesPage";
import { DeliverableTemplatesPage } from "@/pages/deal-room/DeliverableTemplatesPage";

const { listTemplatePreviews, useWorkspaceMock } = vi.hoisted(() => ({
  listTemplatePreviews: vi.fn(),
  useWorkspaceMock: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: { api: { listTemplatePreviews } },
}));

vi.mock("@/app/WorkspaceProvider", () => ({
  useWorkspace: useWorkspaceMock,
}));

vi.mock("@/components/hub/WorkspaceSidebar", () => ({
  WorkspaceSidebar: () => <aside>Deal room sidebar</aside>,
}));

vi.mock("@/components/deal-room/DeliverableTemplatesView", () => ({
  DeliverableTemplatesView: ({ onSelectSlide }: { onSelectSlide?: (slide: {
    id: string;
    thumbnailAlt: string;
    thumbnailHeight: number;
    thumbnailSrc: string;
    thumbnailWidth: number;
  }) => void }) => (
    <div>
      Template gallery
      <button
        onClick={() => onSelectSlide?.({
          id: "template/one",
          thumbnailAlt: "Template preview",
          thumbnailHeight: 720,
          thumbnailSrc: "data:image/png;base64,",
          thumbnailWidth: 1280,
        })}
        type="button"
      >
        Open template
      </button>
    </div>
  ),
}));

afterEach(cleanup);

describe("DealRoomPage", () => {
  beforeEach(() => {
    useWorkspaceMock.mockReturnValue({
      deals: workspaceDeals,
      dealsResource: { deals: workspaceDeals, source: "demo", status: "success" },
      email: "analyst@example.com",
      navigationState: {},
      retryDeals: vi.fn(),
    });
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
    useWorkspaceMock.mockReturnValue({
      deals: [],
      dealsResource: { status: "loading" },
      email: "analyst@example.com",
      navigationState: {},
      retryDeals: vi.fn(),
    });

    render(
      <MemoryRouter initialEntries={["/hub/deals/pending-deal"]}>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route index element={<DealRoomOverviewPage />} />
          </Route>
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
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route index element={<DealRoomOverviewPage />} />
          </Route>
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
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route element={<DeliverablesPage />} path="deliverables" />
            <Route element={<DeliverableTemplatesPage />} path="deliverables/templates" />
          </Route>
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

  it("navigates from a selected gallery preview to the encoded editor route", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/deliverables/templates"]}>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route element={<DeliverableTemplatesPage />} path="deliverables/templates" />
            <Route element={<LocationProbe />} path="deliverables/templates/:templateId" />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("button", { name: "Open template" }));
    expect(screen.getByText("/hub/deals/project-alpha/deliverables/templates/template%2Fone"))
      .not.toBeNull();
  });

});

function LocationProbe() {
  return <p>{useLocation().pathname}</p>;
}
