// @vitest-environment happy-dom

import type { ReactNode } from "react";
import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { VaultPage } from "@/pages/VaultPage";

vi.mock("@/components/hub/WorkspaceHomeShell", () => ({
  WorkspaceHomeShell: ({
    children,
    header,
  }: {
    children: ReactNode;
    header: ReactNode;
  }) => (
    <main>
      {header}
      {children}
    </main>
  ),
}));

afterEach(cleanup);

describe("VaultPage", () => {
  it("renders the visual-only Vault category rail above the ReUI table", () => {
    render(<VaultPage />);

    const categoryRail = screen.getByRole("navigation", {
      name: "Vault sections",
    });

    expect(
      within(categoryRail)
        .getAllByText(
          /^(Documents|Links|Prompts|Notes|Tasks|Templates|Global variables)$/u,
        )
        .map((item) => item.textContent),
    ).toEqual([
      "Documents",
      "Links",
      "Prompts",
      "Notes",
      "Tasks",
      "Templates",
      "Global variables",
    ]);
    expect(categoryRail.querySelectorAll("svg")).toHaveLength(7);
    expect(categoryRail.querySelector("a, button")).toBeNull();
    expect(
      screen.getByRole("heading", { level: 2, name: "Vault table" }),
    ).not.toBeNull();
    expect(
      screen
        .getAllByRole("columnheader")
        .map((header) => header.textContent?.trim()),
    ).toEqual(["Favorite", "Name", "Type", "Practice area", "Creator"]);
    expect(document.querySelector('[data-slot="data-grid"]')).not.toBeNull();
    expect(screen.getByRole("table").querySelector("button")).toBeNull();
    expect(
      screen.getByText("Finalize Q3 Financial Extract for Project Alpha"),
    ).not.toBeNull();
  });
});
