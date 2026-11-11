// @vitest-environment happy-dom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { BrowserRouter, HashRouter } from "react-router-dom";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import App from "@/App";
import { sessionStore } from "@/auth/sessionStore";

vi.mock("@quarry/runtime", () => ({ runtime: { api: {} } }));
vi.mock("@/pages/LoginPage", () => {
  throw new Error("Simulated authentication module download failure");
});

beforeEach(() => sessionStore.set(null));
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  window.history.replaceState(null, "", "/");
});

it.each([
  { path: "/login", title: "Sign in couldn’t load", hash: false },
  { path: "/register", title: "Registration couldn’t load", hash: false },
  { path: "/login", title: "Sign in couldn’t load", hash: true },
  { path: "/register", title: "Registration couldn’t load", hash: true },
])("recovers from failed $path loading with hash routing=$hash", async ({ path, title, hash }) => {
  const url = `${hash ? "/#" : ""}${path}?from=%2Fhub%2Fnotebook`;
  window.history.replaceState({ from: "/hub/notebook" }, "", url);
  const originalUrl = window.location.href;
  const reload = vi.spyOn(window.location, "reload").mockImplementation(() => {});
  const caught = vi.fn();
  const user = userEvent.setup();
  const Router = hash ? HashRouter : BrowserRouter;
  render(<Router><App /></Router>, { onCaughtError: caught });

  const heading = await screen.findByRole("heading", { name: title });
  expect(caught).toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).toContain("Check your connection");
  expect(screen.getByRole("main").contains(heading)).toBe(true);
  expect(screen.getByRole("link", { name: "Skip to main content" })).toBeTruthy();
  expect(screen.queryByRole("status")).toBeNull();
  expect(document.activeElement).toBe(heading);
  expect(window.history.state).toMatchObject({ from: "/hub/notebook" });
  const originalHistoryState: unknown = window.history.state;

  await user.keyboard("{Tab}");
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Reload page" }));
  await user.keyboard("{Enter}");
  expect(reload).toHaveBeenCalledOnce();
  expect(window.location.href).toBe(originalUrl);
  expect(window.history.state).toEqual(originalHistoryState);
});
