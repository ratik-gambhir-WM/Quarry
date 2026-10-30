import { UserProvider } from "@/auth/UserProvider";
import { sessionStore } from "@/auth/sessionStore";
import { testSession } from "@/test/fixtures/auth";
// @vitest-environment happy-dom

import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceProvider, useWorkspace } from "@/app/WorkspaceProvider";
import { AddDealModal } from "@/components/hub/sidebar/AddDealModal";
import { DealRoomPage } from "@/pages/DealRoomPage";
import { DealRoomOverviewPage } from "@/pages/deal-room/DealRoomOverviewPage";
import { testDeal, testMetadata, testParty } from "@/test/fixtures/deals";

const { api } = vi.hoisted(() => ({
  api: {
    listDeals: vi.fn(),
    getDeal: vi.fn(),
    createDeal: vi.fn(),
    saveDealMetadata: vi.fn(),
    listCompanies: vi.fn(),
    updateCompany: vi.fn(),
    updateDeal: vi.fn(),
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
    await user.type(screen.getByLabelText("Deal ID"), deal.dealId);
    await user.type(screen.getByLabelText("Deal name"), deal.dealName);
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
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Edit deal" }));
    await user.clear(screen.getByLabelText("Deal name"));
    await user.type(screen.getByLabelText("Deal name"), "Edited project");
    await user.click(screen.getByRole("button", { name: "Save deal" }));
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
      name: "Add deal resources",
    });
    expect(screen.queryByRole("button", { name: "Edit deal" })).toBeNull();
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
    expect(screen.getByText("Updated question?")).toBeTruthy();
    expect(screen.getByText("retry-sow.pdf")).toBeTruthy();
    expect(api.saveDealMetadata).toHaveBeenLastCalledWith(deal.dealId, {
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
  });

  it("updates an archived deal after a shared company edit even when the deal editor is cancelled", async () => {
    const user = userEvent.setup();
    const party = testParty("Original company", "target");
    const deal = testDeal({ archivedAt: "2026-10-01", parties: [party] });
    api.getDeal.mockResolvedValue({ ...deal, metadata: testMetadata() });
    api.listCompanies.mockResolvedValue({
      companies: [party.company],
      hasMore: false,
    });
    api.updateCompany.mockResolvedValue({
      ...party.company,
      name: "Renamed company",
    });
    renderDealRoutes(`/hub/deals/${deal.dealId}`);
    const overview = await screen.findByLabelText("Engagement overview");
    expect(
      within(overview).getByText("Target").nextElementSibling?.textContent,
    ).toBe("Original company");
    await user.click(screen.getByRole("tab", { name: "Settings" }));
    await user.click(screen.getByRole("button", { name: "Edit deal" }));
    await user.click(
      await screen.findByRole("button", {
        name: "Edit shared company Original company",
      }),
    );
    await user.clear(screen.getByLabelText("Company name"));
    await user.type(screen.getByLabelText("Company name"), "Renamed company");
    await user.click(screen.getByRole("button", { name: "Save company" }));
    await screen.findByText(/was updated for every linked deal/);
    await user.click(
      within(screen.getByRole("dialog")).getByRole("button", {
        name: "Cancel",
      }),
    );
    await user.click(screen.getByRole("tab", { name: "Overview" }));
    expect(
      within(screen.getByLabelText("Engagement overview")).getByText("Target")
        .nextElementSibling?.textContent,
    ).toBe("Renamed company");
    expect(screen.getByLabelText("Portfolio deals").textContent).toBe("");
    expect(api.updateDeal).not.toHaveBeenCalled();
    expect(api.getDeal).toHaveBeenCalledOnce();
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
