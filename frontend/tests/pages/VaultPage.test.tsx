// @vitest-environment happy-dom

import type { ReactNode } from "react";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { VaultPage } from "@/pages/VaultPage";

vi.mock("@/components/hub/WorkspaceHomeShell", () => ({
  WorkspaceHomeShell: ({ children, header }: { children: ReactNode; header: ReactNode }) => (
    <main>{header}{children}</main>
  ),
}));

afterEach(cleanup);

function visibleRows() {
  return within(screen.getByRole("table")).getAllByRole("row").slice(1);
}

describe("VaultPage", () => {
  it("shows all sample items with truthful metadata and no inactive favorites", () => {
    render(<VaultPage />);
    expect(screen.getByText("Sample data")).not.toBeNull();
    expect(screen.getByRole("button", { name: "All items 9" }).getAttribute("aria-pressed")).toBe("true");
    expect(visibleRows()).toHaveLength(9);
    expect(screen.getAllByRole("columnheader").map((header) => header.textContent?.trim()))
      .toEqual(["Name", "Type", "Project", "Practice area", "Created by"]);
    const insight = visibleRows().find((row) => row.textContent?.includes("CapEx will decrease"));
    if (!insight) throw new Error("Missing insight row");
    const cells = within(insight).getAllByRole("cell");
    expect(cells.slice(2).map((cell) => cell.textContent)).toEqual(["Project Alpha", "Financials", "—"]);
    expect(screen.getByRole("table").querySelector("button")).toBeNull();
  });

  it("combines category, project, and text filters and resets them", async () => {
    const user = userEvent.setup();
    render(<VaultPage />);
    await user.click(screen.getByRole("button", { name: "Documents 3" }));
    expect(visibleRows()).toHaveLength(3);
    await user.selectOptions(screen.getByLabelText("Project"), "Project Alpha");
    expect(visibleRows()).toHaveLength(1);
    await user.type(screen.getByLabelText("Search Vault"), "  FINANCIAL  ");
    expect(screen.getByRole("status").textContent).toBe("1 item");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(visibleRows()).toHaveLength(9);
    expect((screen.getByLabelText("Search Vault") as HTMLInputElement).value).toBe("");
    expect((screen.getByLabelText("Project") as HTMLSelectElement).value).toBe("");
  });

  it("allows keyboard category selection and recovers from empty results", async () => {
    const user = userEvent.setup();
    render(<VaultPage />);
    screen.getByRole("button", { name: "Notes 0" }).focus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Notes 0" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByRole("status").textContent).toBe("0 items");
    expect(screen.getByText(/No items match your filters/)).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await user.type(screen.getByLabelText("Search Vault"), "no matching sample");
    expect(screen.getByRole("status").textContent).toBe("0 items");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    expect(visibleRows()).toHaveLength(9);
  });
});
