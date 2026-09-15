// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, useLocation } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AddDealModal } from "@/components/hub/sidebar/AddDealModal";

const { createDeal, saveDealMetadata } = vi.hoisted(() => ({
  createDeal: vi.fn(),
  saveDealMetadata: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { createDeal, saveDealMetadata },
    platform: {},
    target: "web",
  },
}));

afterEach(cleanup);

describe("AddDealModal", () => {
  beforeEach(() => {
    createDeal.mockReset();
    saveDealMetadata.mockReset();
    createDeal.mockResolvedValue({ deal: savedDeal, metadata: emptyMetadata });
    saveDealMetadata.mockResolvedValue({
      deal: savedDeal,
      extraction: { keyQuestions: [] },
      files: [],
      metadata: emptyMetadata,
    });
  });

  it("collects metadata links after the core deal has been created", async () => {
    const user = userEvent.setup();
    render(
      <MemoryRouter>
        <AddDealModal email="analyst@example.com" onClose={vi.fn()} />
      </MemoryRouter>,
    );

    await completeDealDetails(user);

    expect(createDeal).toHaveBeenCalledWith(expect.objectContaining({
      dealId: savedDeal.dealId,
      sharepointLink: null,
    }));

    await user.type(
      screen.getByLabelText("SharePoint link"),
      "https://northwind.sharepoint.com/sites/acme",
    );
    await user.type(screen.getByLabelText("SOW link"), "https://example.com/sow");
    await user.type(
      screen.getByLabelText("Fact sheet link"),
      "https://example.com/fact-sheet",
    );
    await user.type(screen.getByLabelText("RL link"), "https://example.com/request-list");
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
    expect(screen.getByTestId("location").textContent).toBe(`/hub/deals/${savedDeal.dealId}`);
  });
});

async function completeDealDetails(user: ReturnType<typeof userEvent.setup>) {
  fireEvent.change(screen.getByLabelText("Deal ID"), { target: { value: savedDeal.dealId } });
  fireEvent.change(screen.getByLabelText("Deal name"), { target: { value: savedDeal.dealName } });
  fireEvent.change(screen.getByLabelText("Start date"), { target: { value: savedDeal.startDate } });
  fireEvent.change(screen.getByLabelText("Close date"), { target: { value: savedDeal.closeDate } });
  fireEvent.change(screen.getByLabelText("Target company"), { target: { value: savedDeal.targetCompany } });
  fireEvent.change(screen.getByLabelText("Primary buyer"), { target: { value: savedDeal.primaryBuyer } });
  fireEvent.change(screen.getByLabelText("Deal sponsor"), { target: { value: savedDeal.dealSponsor } });
  await user.click(screen.getByRole("combobox", { name: "Transaction type" }));
  await user.click(screen.getByRole("option", { name: savedDeal.transactionType }));
  await user.click(screen.getByRole("button", { name: "Next" }));
  await screen.findByRole("heading", { name: "Add deal metadata" });
}

function LocationProbe() {
  return <output data-testid="location">{useLocation().pathname}</output>;
}

const savedDeal = {
  closeDate: "2026-12-31",
  dealId: "DEAL-000184",
  dealName: "Project Alpha",
  dealSponsor: "Sponsor",
  primaryBuyer: "Buyer",
  startDate: "2026-09-01",
  status: "Active",
  targetCompany: "Target",
  transactionType: "Acquisition",
  userId: 1,
};

const emptyMetadata = {
  dealId: savedDeal.dealId,
  factSheetLink: null,
  keyQuestionsJson: "[]",
  localPath: null,
  rlLink: null,
  sharepointLink: null,
  sowLink: null,
  userId: savedDeal.userId,
};
