// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { UserProvider } from "@/auth/UserProvider";
import { sessionStore } from "@/auth/sessionStore";
import { WorkspaceProvider } from "@/app/WorkspaceProvider";
import { AccountPage } from "@/pages/AccountPage";
import { testSession } from "@/test/fixtures/auth";
const { logout } = vi.hoisted(() => ({ logout: vi.fn() }));
vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { logout, listDeals: vi.fn().mockResolvedValue([]) },
    platform: {},
    target: "web",
  },
}));
afterEach(() => {
  cleanup();
  sessionStore.set(null);
});
beforeEach(() => {
  logout.mockResolvedValue(undefined);
  sessionStore.set(testSession);
});
it("shows the verified profile and signs out", async () => {
  render(
    <MemoryRouter>
      <UserProvider>
        <WorkspaceProvider dataSource="api">
          <AccountPage />
        </WorkspaceProvider>
      </UserProvider>
    </MemoryRouter>,
  );
  expect(screen.getByText("Alex Morgan")).toBeTruthy();
  expect(screen.getByText("analyst@example.com")).toBeTruthy();
  expect(screen.queryByText("API key")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Sign out" }));
  await vi.waitFor(() => expect(sessionStore.getSnapshot().session).toBeNull());
  expect(logout).toHaveBeenCalledTimes(1);
});
