// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { testCompany, testDeal, testMetadata, testParty } from "@/test/fixtures/deals";
import { AddDealModal } from "@/components/hub/sidebar/AddDealModal";

const { createDeal, createCompany, updateDeal, saveDealMetadata, listCompanies } = vi.hoisted(() => ({
  createDeal: vi.fn(),
  createCompany: vi.fn(),
  updateDeal: vi.fn(),
  listCompanies: vi.fn(),
  saveDealMetadata: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { createDeal, createCompany, updateDeal, saveDealMetadata, listCompanies },
    platform: {},
    target: "web",
  },
}));

vi.mock("@/app/WorkspaceProvider", () => ({ useWorkspace: () => ({ saveDealResult: vi.fn() }) }));

afterEach(cleanup);

describe("AddDealModal", () => {
  beforeEach(() => {
    listCompanies.mockResolvedValue({ companies: [], hasMore: false });
    createDeal.mockReset();
    createCompany.mockReset();
    updateDeal.mockReset();
    saveDealMetadata.mockReset();
    createDeal.mockResolvedValue({ deal: savedDeal, metadata: emptyMetadata });
    saveDealMetadata.mockResolvedValue({
      deal: savedDeal,
      extraction: { keyQuestions: [] },
      files: [],
      metadata: emptyMetadata,
    });
  });

  it("edits a persisted party without sending read-only company or relationship fields", async () => {
    const user = userEvent.setup();
    const initialDeal = testDeal({ parties: [testParty("Reusable", "target")] });
    const onClose = vi.fn();
    updateDeal.mockResolvedValue({ deal: initialDeal, metadata: emptyMetadata });
    render(<MemoryRouter><AddDealModal email="analyst@example.com" initialDeal={initialDeal} onClose={onClose} /></MemoryRouter>);
    fireEvent.change(screen.getByLabelText("Engagement completed (optional)"), { target: { value: "2026-10-15" } });
    await user.click(screen.getByRole("button", { name: "Save deal" }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(updateDeal).toHaveBeenCalledWith(initialDeal.dealId, {
      dealName: initialDeal.dealName, dealCode: null, engagementType: "buy_side",
      transactionType: "unknown", status: "pipeline", startDate: null,
      expectedCloseDate: null, actualCloseDate: null, engagementCompletedDate: "2026-10-15",
      parties: [{ companyId: initialDeal.parties[0]?.companyId, role: "target", notes: null }], scopes: [],
    });
    expect(createDeal).not.toHaveBeenCalled();
  });

  it("collects three steps before creating the deal and saving metadata on Finish deal", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <AddDealModal email="analyst@example.com" onClose={vi.fn()} />
      </MemoryRouter>,
    );

    await completeDealDetails(user);

    expect(createDeal).not.toHaveBeenCalled();
    expect(saveDealMetadata).not.toHaveBeenCalled();

    await user.type(
      screen.getByLabelText("SharePoint link"),
      "https://northwind.sharepoint.com/sites/acme",
    );
    await user.type(screen.getByLabelText("SOW link"), "https://example.com/sow");
    await user.type(
      screen.getByLabelText("Fact sheet link"),
      "https://example.com/fact-sheet",
    );
    await user.type(screen.getByLabelText("Request list link"), "https://example.com/request-list");
    await user.click(screen.getByRole("button", { name: "Finish deal" }));

    await waitFor(() => {
      expect(saveDealMetadata).toHaveBeenCalledWith(savedDeal.dealId, {
        factSheetLink: "https://example.com/fact-sheet",
        files: [],
        rlLink: "https://example.com/request-list",
        sharepointLink: "https://northwind.sharepoint.com/sites/acme",
        sowLink: "https://example.com/sow",
      });
    });
    expect(createDeal).toHaveBeenCalledWith(expect.objectContaining({
      dealId: savedDeal.dealId,
      sharepointLink: null,
    }));
  });

  it("skips metadata without saving populated links or selected files", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <AddDealModal email="analyst@example.com" onClose={vi.fn()} />
        <LocationProbe />
      </MemoryRouter>,
    );

    await completeDealDetails(user);
    await user.type(screen.getByLabelText("SOW link"), "https://example.com/sow");
    const fileInput = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!fileInput) throw new Error("expected the source file input to be rendered");
    fireEvent.change(fileInput, {
      target: { files: [new File(["source"], "scope.pdf", { type: "application/pdf" })] },
    });

    await user.click(screen.getByRole("button", { name: "Skip metadata" }));

    expect(saveDealMetadata).not.toHaveBeenCalled();
    expect(createDeal).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("location").textContent).toBe(`/hub/deals/${savedDeal.dealId}`);
  });

  it("keeps details, company roles, company drafts, and sources when moving back and forth", async () => {
    const user = userEvent.setup();
    const company = testCompany("Reusable");
    createCompany.mockResolvedValue(company);
    render(<MemoryRouter><AddDealModal email="analyst@example.com" onClose={vi.fn()} /></MemoryRouter>);
    expect(screen.queryByRole("button", { name: "Create company" })).toBeNull();
    expect(screen.queryByRole("textbox", { name: "SOW link" })).toBeNull();
    fireEvent.change(screen.getByLabelText("Deal ID"), { target: { value: savedDeal.dealId } });
    fireEvent.change(screen.getByLabelText("Deal name"), { target: { value: savedDeal.dealName } });
    await user.click(screen.getByRole("button", { name: "Next" }));
    await screen.findByRole("heading", { name: "Assign companies and roles" });
    expect(createDeal).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Create company" }));
    await user.type(screen.getByLabelText("Company name"), "Reusable");
    expect(screen.getByRole("button", { name: "Next" }).hasAttribute("disabled")).toBe(true);
    await user.click(screen.getByRole("button", { name: "Save company" }));
    await user.selectOptions(await screen.findByRole("combobox", { name: "Role for company 1" }), "buyer");
    await user.type(screen.getByLabelText("Deal-specific notes for company 1"), "Lead buyer");
    await user.click(screen.getByRole("button", { name: "Back" }));
    expect((screen.getByLabelText("Deal name") as HTMLInputElement).value).toBe(savedDeal.dealName);
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByText("Reusable")).toBeTruthy();
    expect((screen.getByLabelText("Role for company 1") as HTMLSelectElement).value).toBe("buyer");
    await user.click(screen.getByRole("button", { name: "Next" }));
    await user.type(screen.getByLabelText("SOW link"), "https://example.com/sow");
    const file = new File(["source"], "scope.pdf", { type: "application/pdf" });
    const fileInput = document.querySelector<HTMLInputElement>('input[type="file"]');
    if (!fileInput) throw new Error("expected source file input");
    fireEvent.change(fileInput, { target: { files: [file] } });
    await user.click(screen.getByRole("button", { name: "Back" }));
    await user.click(screen.getByRole("button", { name: "Next" }));
    expect((screen.getByLabelText("SOW link") as HTMLInputElement).value).toBe("https://example.com/sow");
    expect(screen.getByText("scope.pdf")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    await waitFor(() => expect(saveDealMetadata).toHaveBeenCalledWith(savedDeal.dealId, expect.objectContaining({ files: [file] })));
    expect(createDeal).toHaveBeenCalledWith(expect.objectContaining({
      parties: [{ companyId: company.companyId, role: "buyer", notes: "Lead buyer" }],
    }));
  });

  it("does not persist a deal when cancelled after assigning companies", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<MemoryRouter><AddDealModal email="analyst@example.com" onClose={onClose} /></MemoryRouter>);
    await completeDealDetails(user);
    await user.click(screen.getByRole("button", { name: "Close add deal dialog" }));
    expect(onClose).toHaveBeenCalled();
    expect(createDeal).not.toHaveBeenCalled();
    expect(saveDealMetadata).not.toHaveBeenCalled();
  });

  it("retries failed metadata without creating the deal again", async () => {
    const user = userEvent.setup();
    saveDealMetadata.mockRejectedValueOnce(new Error("Extraction unavailable"));
    render(<MemoryRouter><AddDealModal email="analyst@example.com" onClose={vi.fn()} /></MemoryRouter>);
    await completeDealDetails(user);
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    expect((await screen.findByRole("alert")).textContent).toContain("Deal created, but metadata could not be saved");
    expect(screen.getByRole("button", { name: "Back" }).hasAttribute("disabled")).toBe(true);
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    await waitFor(() => expect(saveDealMetadata).toHaveBeenCalledTimes(2));
    expect(createDeal).toHaveBeenCalledTimes(1);
  });

  it("returns to company assignments if the core deal is rejected and allows retry", async () => {
    const user = userEvent.setup();
    createDeal.mockRejectedValueOnce(new Error("An active acquisition requires exactly one target"));
    render(<MemoryRouter><AddDealModal email="analyst@example.com" onClose={vi.fn()} /></MemoryRouter>);
    await completeDealDetails(user);
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    await screen.findByRole("heading", { name: "Assign companies and roles" });
    expect(screen.getByRole("alert").textContent).toContain("exactly one target");
    expect(saveDealMetadata).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Next" }));
    await user.click(screen.getByRole("button", { name: "Finish deal" }));
    await waitFor(() => expect(saveDealMetadata).toHaveBeenCalledTimes(1));
    expect(createDeal).toHaveBeenCalledTimes(2);
  });
});

async function completeDealDetails(user: ReturnType<typeof userEvent.setup>) {
  fireEvent.change(screen.getByLabelText("Deal ID"), { target: { value: savedDeal.dealId } });
  fireEvent.change(screen.getByLabelText("Deal name"), { target: { value: savedDeal.dealName } });
  await user.click(screen.getByRole("button", { name: "Next" }));
  await screen.findByRole("heading", { name: "Assign companies and roles" });
  await user.click(screen.getByRole("button", { name: "Next" }));
  await screen.findByRole("heading", { name: "Add deal metadata" });
}

function LocationProbe() {
  return <output data-testid="location">{useLocation().pathname}</output>;
}

const savedDeal = testDeal();
const emptyMetadata = testMetadata();
