import { UserProvider } from "@/auth/UserProvider";
import { sessionStore } from "@/auth/sessionStore";
import { testSession } from "@/test/fixtures/auth";
// @vitest-environment happy-dom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Link, MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceProvider, useWorkspace } from "@/app/WorkspaceProvider";
import { AddDealModal } from "@/components/hub/sidebar/AddDealModal";
import { DealRoomPage } from "@/pages/DealRoomPage";
import { DealRoomOverviewPage } from "@/pages/deal-room/DealRoomOverviewPage";
import { testDeal, testMetadata } from "@/test/fixtures/deals";

// Match the member-panel suite: use Motion's JS fallback in happy-dom.
const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => {
  if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});

const { api } = vi.hoisted(() => ({
  api: {
    listDeals: vi.fn(),
    getDeal: vi.fn(),
    createDeal: vi.fn(),
    saveDealMetadata: vi.fn(),
    listCompanies: vi.fn(),
    updateCompany: vi.fn(),
    updateDeal: vi.fn(),
    listDealMembers: vi.fn(),
    listDealTeams: vi.fn(),
  },
}));
vi.mock("@quarry/runtime", () => ({
  runtime: { target: "web", api, platform: {} },
}));
vi.mock("@/components/hub/WorkspaceSidebar", () => ({
  WorkspaceSidebar: () => null,
}));

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
beforeEach(() => {
  sessionStore.set(testSession);
  window.localStorage.clear();
  window.sessionStorage.clear();
  api.listDeals.mockResolvedValue([]);
  api.getDeal.mockImplementation(async () => ({
    ...testDeal(),
    metadata: testMetadata(),
  }));
  api.listDealMembers.mockResolvedValue({ items: [], nextCursor: null });
  api.listDealTeams.mockResolvedValue({ items: [], nextCursor: null });
  api.listCompanies.mockResolvedValue({ companies: [], hasMore: false });
});

function PortfolioProbe() {
  const { deals } = useWorkspace();
  return (
    <output aria-label="Portfolio deals">
      {deals.map((d) => d.room.id).join(",")}
    </output>
  );
}

function renderDealRoutes(pathname: string) {
  return render(
    <MemoryRouter
      initialEntries={[{ pathname, state: { email: "owner@example.com" } }]}
    >
      <UserProvider>
        <WorkspaceProvider dataSource="api">
          <PortfolioProbe />
          <Routes>
            <Route
              path="/hub/deals"
              element={
                <AddDealModal email="owner@example.com" onClose={vi.fn()} />
              }
            />
            <Route path="/hub/deals/:dealId" element={<DealRoomPage />}>
              <Route index element={<DealRoomOverviewPage />} />
            </Route>
          </Routes>
        </WorkspaceProvider>
      </UserProvider>
    </MemoryRouter>,
  );
}

describe("DealRoomPage", () => {
  function AccessProbe() {
    const { forgetDeal } = useWorkspace();
    const location = useLocation();
    return (
      <>
        <button onClick={() => forgetDeal(testDeal().dealId)}>Forget access</button>
        <Link to={`/hub/deals/${testDeal().dealId}`}>Open restored deal</Link>
        <output aria-label="Current route">{location.pathname}</output>
        <output aria-label="Access notice">{location.state?.notice}</output>
        <PortfolioProbe />
      </>
    );
  }
  function renderAccessRoutes(pathname: string) {
    render(
      <MemoryRouter initialEntries={[pathname]}>
        <UserProvider>
          <WorkspaceProvider dataSource="api">
            <AccessProbe />
            <Routes>
              <Route path="/hub/deals" element={<p>Accessible deals</p>} />
              <Route path="/hub/deals/:dealId" element={<DealRoomPage />}>
                <Route index element={<DealRoomOverviewPage />} />
              </Route>
            </Routes>
          </WorkspaceProvider>
        </UserProvider>
      </MemoryRouter>,
    );
  }

  it.each([403, 404])("redirects and evicts with the real workspace store on %s", async (status) => {
    const deal = testDeal();
    api.listDeals.mockResolvedValue([{ ...deal, metadata: null }]);
    renderAccessRoutes(`/hub/deals/${deal.dealId}`);
    await screen.findByRole("heading", { name: deal.dealName });
    api.getDeal.mockRejectedValue({ status });
    act(() => window.dispatchEvent(new Event("focus")));
    await screen.findByText("Accessible deals");
    expect(api.getDeal).toHaveBeenCalledTimes(2);
    expect(screen.getByLabelText("Portfolio deals").textContent).toBe("");
    expect(screen.getByLabelText("Current route").textContent).toBe("/hub/deals");
    expect(screen.getByLabelText("Access notice").textContent).toBe(
      "You no longer have access to this deal.",
    );
    expect(screen.queryByRole("heading", { name: deal.dealName })).toBeNull();
  });

  it("opens a freshly authorized direct link after the deal was forgotten", async () => {
    const user = userEvent.setup();
    renderAccessRoutes("/hub/deals");
    await screen.findByText("Accessible deals");
    await user.click(screen.getByRole("button", { name: "Forget access" }));
    await user.click(screen.getByRole("link", { name: "Open restored deal" }));
    await screen.findByRole("heading", { name: testDeal().dealName });
    expect(screen.getByLabelText("Portfolio deals").textContent).toBe(testDeal().dealId);
    expect(api.listDeals).toHaveBeenCalledTimes(1);
  });

  it("shows an uploaded SOW after creation and retains it when core deal fields are edited", async () => {
    const user = userEvent.setup();
    const deal = testDeal();
    const metadata = testMetadata();
    api.createDeal.mockResolvedValue({ deal, metadata });
    api.saveDealMetadata.mockResolvedValue({
      deal,
      metadata,
      extraction: { keyQuestions: [] },
      files: [
        {
          filename: "sow.pdf",
          path: "sow.pdf",
          relativePath: "sow.pdf",
          sizeBytes: 10,
        },
      ],
    });
    api.updateDeal.mockResolvedValue({
      deal: { ...deal, dealName: "Edited project" },
      metadata,
    });
    renderDealRoutes("/hub/deals");
    await user.type(screen.getByLabelText("Deal name"), deal.dealName);
    await user.click(screen.getByRole("button", { name: "Next" }));
    await user.click(screen.getByRole("button", { name: "Next" }));
    await user.click(screen.getByRole("button", { name: "Next" }));
    const fileInput =
      document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!fileInput) throw new Error("expected SOW file picker");
    fireEvent.change(fileInput, {
      target: {
        files: [new File(["source"], "sow.pdf", { type: "application/pdf" })],
      },
    });
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    await screen.findByRole("heading", { name: deal.dealName });
    expect(await screen.findByText("sow.pdf")).toBeTruthy();
    await user.click(screen.getByRole("tab", { name: "Team" }));
    await user.click(screen.getByRole("button", { name: "Edit deal" }));
    await user.clear(screen.getByLabelText("Deal name"));
    await user.type(screen.getByLabelText("Deal name"), "Edited project");
    await user.click(screen.getByRole("button", { name: "Save deal" }));
    api.getDeal.mockResolvedValue({
      ...deal,
      dealName: "Edited project",
      metadata,
    });
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    expect(
      await screen.findByRole("heading", { name: "Edited project" }),
    ).toBeTruthy();
    expect(screen.getByText("sow.pdf")).toBeTruthy();
  });

  it("saves resources from Overview, retains failed drafts, and never recreates the deal", async () => {
    const user = userEvent.setup();
    const deal = testDeal();
    const metadata = {
      ...testMetadata(),
      sowLink: "https://example.com/original-sow",
      keyQuestionsJson: '["Existing question?"]',
    };
    api.listDeals.mockResolvedValue([{ ...deal, metadata }]);
    api.getDeal.mockResolvedValue({ ...deal, metadata });
    api.saveDealMetadata
      .mockRejectedValueOnce(new Error("Extraction unavailable"))
      .mockResolvedValueOnce({
        deal,
        metadata: { ...metadata, factSheetLink: "https://example.com/facts" },
        extraction: { keyQuestions: ["Updated question?"] },
        files: [
          {
            filename: "retry-sow.pdf",
            path: "retry-sow.pdf",
            relativePath: "retry-sow.pdf",
            sizeBytes: 6,
          },
        ],
      });
    renderDealRoutes(`/hub/deals/${deal.dealId}`);
    const add = await screen.findByRole("button", {
      name: "Manage resources",
    });
    expect(screen.getByRole("button", { name: "Edit deal" })).toBeTruthy();
    await user.click(add);
    expect(screen.getByLabelText("SOW link")).toHaveProperty(
      "value",
      metadata.sowLink,
    );
    await user.type(
      screen.getByLabelText("Fact sheet link"),
      "https://example.com/facts",
    );
    const file = new File(["source"], "retry-sow.pdf", {
      type: "application/pdf",
    });
    await user.upload(screen.getByLabelText("SOW file"), file);
    await user.click(screen.getByRole("button", { name: "Save resources" }));
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Extraction unavailable"),
    );
    expect(screen.getByLabelText("Fact sheet link")).toHaveProperty(
      "value",
      "https://example.com/facts",
    );
    expect(screen.getByText("retry-sow.pdf")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Save resources" }));
    expect(
      await screen.findByRole("link", { name: "Open Fact Sheet in a new tab" }),
    ).toHaveProperty("href", "https://example.com/facts");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText("retry-sow.pdf")).toBeTruthy();
    expect(api.saveDealMetadata).toHaveBeenLastCalledWith(deal.dealId, {
      expectedRevision: 1,
      files: [file],
      sowLink: metadata.sowLink,
      factSheetLink: "https://example.com/facts",
      sharepointLink: null,
      rlLink: null,
    });
    expect(api.createDeal).not.toHaveBeenCalled();
    expect(api.updateDeal).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(add);
    await user.click(add);
    expect(screen.getByLabelText("Fact sheet link")).toHaveProperty(
      "value",
      "https://example.com/facts",
    );
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(api.saveDealMetadata).toHaveBeenCalledTimes(2);
    api.getDeal.mockResolvedValue({
      ...deal,
      metadata: { ...metadata, factSheetLink: "https://example.com/facts", keyQuestionsJson: '["Updated question?"]' },
    });
    await user.click(screen.getByRole("tab", { name: "Scope" }));
    expect(await screen.findByText("Updated question?")).toBeTruthy();
  });

  it("keeps archived deal settings read-only while personal company records remain separate", async () => {
    const user = userEvent.setup();
    const deal = testDeal({ archivedAt: "2026-10-01" });
    api.getDeal.mockResolvedValue({ ...deal, metadata: testMetadata() });
    renderDealRoutes(`/hub/deals/${deal.dealId}`);
    await screen.findByRole("heading", { name: deal.dealName });
    await user.click(screen.getByRole("tab", { name: "Team" }));
    expect(screen.queryByRole("button", { name: "Edit deal" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Create team" })).toBeNull();
    expect(api.updateDeal).not.toHaveBeenCalled();
    expect(api.updateCompany).not.toHaveBeenCalled();
  });

  it("retries a failed direct lookup", async () => {
    const user = userEvent.setup();
    const deal = testDeal({ archivedAt: "2026-10-01" });
    api.getDeal
      .mockRejectedValueOnce(new Error("Lookup unavailable"))
      .mockResolvedValue({ ...deal, metadata: null });
    renderDealRoutes(`/hub/deals/${deal.dealId}`);
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      expect.stringContaining("Lookup unavailable"),
    );
    await user.click(screen.getByRole("button", { name: "Retry deal" }));
    expect(
      await screen.findByRole("heading", { name: deal.dealName }),
    ).toBeTruthy();
    expect(screen.getByLabelText("Portfolio deals").textContent).toBe("");
  });
});
