// @vitest-environment happy-dom

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { WorkspaceHomeShell } from "@/components/hub/WorkspaceHomeShell";

const { retryDeals, useWorkspaceMock } = vi.hoisted(() => ({
  retryDeals: vi.fn(),
  useWorkspaceMock: vi.fn(),
}));

vi.mock("@/app/WorkspaceProvider", () => ({ useWorkspace: useWorkspaceMock }));
vi.mock("@/components/hub/WorkspaceSidebar", () => ({ WorkspaceSidebar: () => <aside /> }));

afterEach(cleanup);

describe("WorkspaceHomeShell", () => {
  beforeEach(() => {
    retryDeals.mockReset();
    useWorkspaceMock.mockReturnValue({
      deals: [],
      dealsResource: { deals: [], source: "demo", status: "success" },
      initiatives: [],
      retryDeals,
      tools: [],
    });
  });

  it("labels explicitly selected demo data", () => {
    render(<WorkspaceHomeShell header={null}>Workspace content</WorkspaceHomeShell>);

    expect(screen.getByRole("status").textContent).toContain("Demo data");
    expect(screen.getByText("Workspace content")).toBeTruthy();
  });

  it("keeps independent content available beside a retryable API warning", () => {
    useWorkspaceMock.mockReturnValue({
      deals: [],
      dealsResource: { message: "Service unavailable", status: "error" },
      initiatives: [],
      retryDeals,
      tools: [],
    });
    render(<WorkspaceHomeShell header={null}>Account content</WorkspaceHomeShell>);

    expect(screen.getByRole("alert").textContent).toContain("Service unavailable");
    expect(screen.getByText("Account content")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(retryDeals).toHaveBeenCalledTimes(1);
  });

  it("keeps notices above a shrinking-safe fill child", () => {
    const { container } = render(
      <WorkspaceHomeShell contentMode="fill" header={null}>
        <div>Assistant thread</div>
      </WorkspaceHomeShell>,
    );

    expect(screen.getByRole("status").textContent).toContain("Demo data");
    expect(screen.getByText("Assistant thread").parentElement?.className).toContain("min-h-0 flex-1");
    expect(container.querySelector(".overflow-hidden.p-0")).toBeTruthy();
  });
});
