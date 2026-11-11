import { sessionStore } from "@/auth/sessionStore";
import { testSession } from "@/test/fixtures/auth";
import { testDeal } from "@/test/fixtures/deals";
// @vitest-environment happy-dom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { MemoryRouter, useNavigate } from "react-router-dom";
import {
  afterAll,
  beforeAll,
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import App from "@/App";
import { RootRoutes } from "@/app/RootRoutes";
import type { PersistedDeal } from "@/contracts/quarryApi";

const { listDeals, runAssistantThread } = vi.hoisted(() => ({
  listDeals: vi.fn(),
  runAssistantThread: vi.fn(),
}));
const lazyRouteWait = { timeout: 5_000 };

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: {
      listDeals,
      getDeal: async () => persistedDeal,
      listDealActivity: async () => ({ items: [], nextCursor: null }),
      runAssistantThread,
      listNotebooks: async () => ({ notebooks: [], hasMore: false }),
    },
    platform: {},
    target: "web",
  },
}));

// Happy DOM rejects canceled Web Animation promises; exercise Motion’s JS fallback.
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

afterEach(() => {
  cleanup();
  sessionStore.set(null);
});

describe("App routes", () => {
  beforeEach(() => {
    sessionStore.set(testSession);
    listDeals.mockReset();
    listDeals.mockResolvedValue([persistedDeal]);
    runAssistantThread.mockReset();
    runAssistantThread.mockReturnValue(() => undefined);
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("loads sign in without starting workspace data loading", async () => {
    sessionStore.set(null);
    render(
      <MemoryRouter initialEntries={["/login"]}>
        <App />
      </MemoryRouter>,
    );

    expect(await screen.findByRole("heading", { name: "Quarry" })).toBeTruthy();
    expect(listDeals).not.toHaveBeenCalled();
  });

  it("isolates the internal template renderer from the product application", async () => {
    render(
      <MemoryRouter initialEntries={["/_internal/template-preview"]}>
        <RootRoutes />
      </MemoryRouter>,
    );

    expect(
      await screen.findByText("No template preview input was provided."),
    ).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Quarry" })).toBeNull();
    expect(listDeals).not.toHaveBeenCalled();
  });

  it("opens legacy activity links in the Deal Room tab and preserves browser history", async () => {
    render(
      <MemoryRouter
        initialEntries={[
          "/hub/deals/project-alpha",
          "/hub/deals/project-alpha/activity",
        ]}
        initialIndex={1}
      >
        <App />
        <HistoryControls />
      </MemoryRouter>,
    );

    expect(
      await screen.findByRole(
        "tab",
        { name: "Deal Activity", selected: true },
        lazyRouteWait,
      ),
    ).toBeTruthy();
    expect(
      (
        await screen.findAllByRole(
          "heading",
          { name: "Deal activity" },
          lazyRouteWait,
        )
      ).length,
    ).toBeGreaterThan(0);
    expect(listDeals).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("link", { name: "Deal Activity" })).toBeNull();
    const main = screen.getByRole("main");
    expect(main.contains(screen.getByRole("tab", { name: "Overview" }))).toBe(
      true,
    );
    expect(
      main.contains(
        screen.getByRole("heading", { level: 1, name: "Deal activity" }),
      ),
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(
      await screen.findByRole(
        "link",
        { name: "Deal Room", current: "page" },
        lazyRouteWait,
      ),
    ).toBeTruthy();
    expect(
      await screen.findByRole(
        "tab",
        { name: "Overview", selected: true },
        lazyRouteWait,
      ),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Forward" }));
    expect(
      await screen.findByRole(
        "tab",
        { name: "Deal Activity", selected: true },
        lazyRouteWait,
      ),
    ).toBeTruthy();
    expect(listDeals).toHaveBeenCalledTimes(1);
  }, 10_000);

  it("opens Notebook from its shared workspace link", async () => {
    render(
      <MemoryRouter initialEntries={["/hub"]}>
        <App />
      </MemoryRouter>,
    );
    const navigation = await screen.findByRole("navigation", {
      name: "Workspace",
    });
    const link = within(navigation).getByRole("link", { name: "Notebook" });
    fireEvent.click(link);
    expect(
      await screen.findByRole("heading", { name: "A little room to think" }),
    ).toBeTruthy();
    expect(screen.getByRole("link", { name: "Back to Deal Hub" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Save" })).toBeTruthy();
  });

  it("registers Assistant at its new route and removes the former summarize leaf", async () => {
    const { unmount } = render(
      <MemoryRouter initialEntries={["/hub/assistant"]}>
        <App />
      </MemoryRouter>,
    );

    expect(
      await screen.findByRole("heading", { name: "How can I help you today?" }, lazyRouteWait),
    ).toBeTruthy();
    expect(
      screen.getByRole("tab", { name: "Chat", selected: true }),
    ).toBeTruthy();
    expect(listDeals).toHaveBeenCalledTimes(1);
    unmount();

    render(
      <MemoryRouter initialEntries={["/hub/summarize"]}>
        <App />
      </MemoryRouter>,
    );

    expect(await screen.findByRole("main")).toBeTruthy();
  });
});

function HistoryControls() {
  const navigate = useNavigate();
  return (
    <>
      <button onClick={() => navigate(-1)} type="button">
        Back
      </button>
      <button onClick={() => navigate(1)} type="button">
        Forward
      </button>
    </>
  );
}

const persistedDeal: PersistedDeal = {
  ...testDeal({ dealId: "project-alpha" }),
  metadata: null,
};
