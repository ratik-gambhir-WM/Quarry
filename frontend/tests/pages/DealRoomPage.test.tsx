// @vitest-environment happy-dom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { workspaceDeals } from "@/fixtures/workspace/portfolio";
import { DealRoomPage } from "@/pages/DealRoomPage";
import { DealRoomOverviewPage } from "@/pages/deal-room/DealRoomOverviewPage";
import { DeliverablesPage } from "@/pages/deal-room/DeliverablesPage";
import { DeliverableTemplatesPage } from "@/pages/deal-room/DeliverableTemplatesPage";

// happy-dom's Web Animations cancellation rejects promises; use Motion's JS fallback.
const animationDescriptor = Object.getOwnPropertyDescriptor(
  Element.prototype,
  "animate",
);
beforeAll(() => {
  Reflect.deleteProperty(Element.prototype, "animate");
});
afterAll(() => {
  if (animationDescriptor)
    Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});

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
  DeliverableTemplatesView: ({
    onSelectSlide,
  }: {
    onSelectSlide?: (slide: {
      id: string;
      thumbnailAlt: string;
      thumbnailHeight: number;
      thumbnailSrc: string;
      thumbnailWidth: number;
    }) => void;
  }) => (
    <div>
      Template gallery
      <button
        onClick={() =>
          onSelectSlide?.({
            id: "template/one",
            thumbnailAlt: "Template preview",
            thumbnailHeight: 720,
            thumbnailSrc: "data:image/png;base64,",
            thumbnailWidth: 1280,
          })
        }
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
      dealsResource: {
        deals: workspaceDeals,
        source: "demo",
        status: "success",
      },
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

  it.each([
    ["data-room", true],
    ["analysis", false],
    ["", false],
  ])(
    "enables logo tools only for the Data Room route (%s)",
    (section, enabled) => {
      render(
        <MemoryRouter initialEntries={[`/hub/deals/project-alpha/${section}`]}>
          <Routes>
            <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
              <Route index element={<p>Overview</p>} />
              <Route path="data-room" element={<p>Data Room</p>} />
              <Route path="analysis" element={<p>Analysis</p>} />
            </Route>
          </Routes>
        </MemoryRouter>,
      );
      expect(
        screen.queryByRole("button", { name: "Data Room tools" }) !== null,
      ).toBe(enabled);
    },
  );

  it("keeps the sidebar mounted when entering and leaving the Data Room", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/analysis"]}>
        <Link to="/hub/deals/project-alpha/data-room">Open data room</Link>
        <Link to="/hub/deals/project-alpha/analysis">Open analysis</Link>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route path="data-room" element={<p>Data Room</p>} />
            <Route path="analysis" element={<p>Analysis</p>} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );
    const sidebar = screen.getByText("Deal room sidebar");
    await user.click(screen.getByRole("link", { name: "Open data room" }));
    expect(screen.getByText("Deal room sidebar")).toBe(sidebar);
    expect(screen.getByRole("button", { name: "Data Room tools" })).toBeTruthy();
    await user.click(screen.getByRole("link", { name: "Open analysis" }));
    expect(screen.getByText("Deal room sidebar")).toBe(sidebar);
    expect(screen.queryByRole("button", { name: "Data Room tools" })).toBeNull();
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
    expect(
      document.querySelectorAll('[data-slot="skeleton"]').length,
    ).toBeGreaterThan(0);
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
      "Deal Activity",
      "Site Visits",
      "File Summary",
      "Evidence",
      "Findings",
      "Open Items",
      "Settings",
      "Search documents",
    ]);
    expect(
      screen.getByRole("heading", { level: 1, name: "Project Alpha" }),
    ).not.toBeNull();
    expect(screen.queryByText("Analyzed Files")).toBeNull();
    expect(
      screen.getByRole("tab", { name: "Evidence, coming soon" }),
    ).toHaveProperty("disabled", true);

    expect(screen.getByRole("tab", { name: "Settings" })).toHaveProperty(
      "disabled",
      false,
    );
    const overviewTab = screen.getByRole("tab", { name: "Overview" });
    const fileSummaryTab = screen.getByRole("tab", { name: "File Summary" });
    overviewTab.focus();
    await user.keyboard("{ArrowRight}");

    const activityTab = await screen.findByRole("tab", {
      name: "Deal Activity",
      selected: true,
    });
    expect(document.activeElement).toBe(activityTab);
    expect(
      screen
        .getByRole("heading", { level: 1, name: "Deal Activity" })
        .closest("main"),
    ).toBe(overviewTab.closest("main"));
    await user.keyboard("{ArrowRight}");
    const siteVisitsTab = await screen.findByRole("tab", {
      name: "Site Visits",
      selected: true,
    });
    expect(document.activeElement).toBe(siteVisitsTab);
    expect(
      screen.getByRole("heading", { level: 1, name: "Site Visits" }),
    ).not.toBeNull();
    expect(
      screen.getByRole("heading", { name: "Activity Calendar" }),
    ).not.toBeNull();
    await user.keyboard("{ArrowRight}");
    await screen.findByRole("tab", { name: "File Summary", selected: true });
    expect(document.activeElement).toBe(fileSummaryTab);
    expect(screen.getByText("Analyzed Files")).not.toBeNull();
    expect(
      screen.getByRole("heading", { level: 1, name: "Project Alpha" }),
    ).not.toBeNull();
    expect(
      screen.queryByText(workspaceDeals[0]?.room.summary ?? ""),
    ).toBeNull();
    expect(document.querySelector('[data-slot="data-grid"]')).not.toBeNull();
  });

  it("opens Site Visits from a direct link and preserves calendar edits across tabs", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter
        initialEntries={["/hub/deals/project-alpha?section=site-visits"]}
      >
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route index element={<DealRoomOverviewPage />} />
          </Route>
        </Routes>
      </MemoryRouter>,
    );
    expect(
      screen.getByRole("tab", { name: "Site Visits", selected: true }),
    ).not.toHaveProperty("disabled", true);
    expect(
      screen.getByRole("heading", { name: "Activity Calendar" }),
    ).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Log Activity" }));
    await user.type(
      screen.getByLabelText("Activity Title"),
      "Site walkthrough",
    );
    await user.type(
      screen.getByLabelText("Date", { exact: true }),
      "2026-10-15",
    );
    await user.tab();
    const dialog = screen.getByRole("dialog", { name: "New Activity" });
    await user.click(
      within(dialog).getByRole("button", { name: "Log Activity" }),
    );
    expect(screen.queryByRole("dialog", { name: "New Activity" })).toBeNull();
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    expect(
      screen.queryByRole("heading", { name: "Activity Calendar" }),
    ).toBeNull();
    await user.click(screen.getByRole("tab", { name: "Site Visits" }));
    expect(screen.getAllByText("Site walkthrough").length).toBeGreaterThan(0);
  });

  it("navigates from the two-section Deliverables page to Templates and back", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/hub/deals/project-alpha/deliverables"]}>
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route element={<DeliverablesPage />} path="deliverables" />
            <Route
              element={<DeliverableTemplatesPage />}
              path="deliverables/templates"
            />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    expect(
      screen.getByRole("region", { name: "Completed Slide(s)" }),
    ).not.toBeNull();
    expect(
      screen.getByRole("region", { name: "In-progress slides" }),
    ).not.toBeNull();
    expect(screen.queryByText("Start from templates")).toBeNull();

    await user.click(screen.getByRole("button", { name: "View Templates" }));
    expect(
      screen.getByRole("heading", { level: 1, name: "Templates" }),
    ).not.toBeNull();

    await user.click(
      screen.getByRole("button", { name: "Back to Deliverables" }),
    );
    expect(
      screen.getByRole("heading", { level: 1, name: "Deliverables" }),
    ).not.toBeNull();
  });

  it("navigates from a selected gallery preview to the encoded editor route", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter
        initialEntries={["/hub/deals/project-alpha/deliverables/templates"]}
      >
        <Routes>
          <Route element={<DealRoomPage />} path="/hub/deals/:dealId">
            <Route
              element={<DeliverableTemplatesPage />}
              path="deliverables/templates"
            />
            <Route
              element={<LocationProbe />}
              path="deliverables/templates/:templateId"
            />
          </Route>
        </Routes>
      </MemoryRouter>,
    );

    await user.click(screen.getByRole("button", { name: "Open template" }));
    expect(
      screen.getByText(
        "/hub/deals/project-alpha/deliverables/templates/template%2Fone",
      ),
    ).not.toBeNull();
  });
});

function LocationProbe() {
  return <p>{useLocation().pathname}</p>;
}
