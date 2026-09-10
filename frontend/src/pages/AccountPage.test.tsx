// @vitest-environment happy-dom

import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceAccountUser } from "../data/workspace";
import { AccountPage } from "./AccountPage";

const { getUserByEmail, listDeals } = vi.hoisted(() => ({
  getUserByEmail: vi.fn(),
  listDeals: vi.fn(),
}));

vi.mock("@quarry/runtime", () => ({
  runtime: {
    api: { getUserByEmail, listDeals },
    platform: {},
    target: "web",
  },
}));

const accountUser: WorkspaceAccountUser = {
  apiKey: "secret-api-key",
  createdAt: "2026-09-01T12:00:00Z",
  email: "analyst@example.com",
  firstName: "Alex",
  id: 1,
  lastName: "Morgan",
  role: "Analyst",
  updatedAt: "2026-09-02T12:00:00Z",
};

describe("AccountPage", () => {
  afterEach(cleanup);

  beforeEach(() => {
    getUserByEmail.mockReset();
    listDeals.mockReset();
    listDeals.mockResolvedValue([]);
    window.sessionStorage.clear();
  });

  it("renders the destination skeleton while the account request is pending", async () => {
    const request = deferred<WorkspaceAccountUser | null>();
    getUserByEmail.mockReturnValue(request.promise);

    renderAccountPage();

    expect(screen.getByText("Loading account information")).toBeTruthy();
    expect(document.querySelectorAll('[data-slot="skeleton"]').length).toBeGreaterThan(0);
    expect(getUserByEmail).toHaveBeenCalledWith("analyst@example.com");

    request.resolve(accountUser);

    expect(await screen.findByText("Alex Morgan")).toBeTruthy();
    expect(screen.queryByText("Loading account information")).toBeNull();
  });

  it("shows a request error on the destination page", async () => {
    getUserByEmail.mockRejectedValue(new Error("Account service unavailable"));

    renderAccountPage();

    expect((await screen.findByRole("alert")).textContent).toBe("Account service unavailable");
  });
});

function renderAccountPage() {
  render(
    <MemoryRouter initialEntries={[{ pathname: "/hub/account", state: { email: "analyst@example.com" } }]}>
      <AccountPage />
    </MemoryRouter>,
  );
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });

  return { promise, resolve };
}
