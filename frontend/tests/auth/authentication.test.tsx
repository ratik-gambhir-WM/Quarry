// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { UserProvider } from "@/auth/UserProvider";
import { credentials, sessionStore } from "@/auth/sessionStore";
import { LoginPage } from "@/pages/LoginPage";
import { testSession } from "@/test/fixtures/auth";
import { authFetch } from "@/api/authFetch";
import { httpQuarryApi } from "@/api/httpQuarryApi";
import { getActivityLogEntries } from "@/lib/activityLog";
const { login, register, logout } = vi.hoisted(() => ({ login: vi.fn(), register: vi.fn(), logout: vi.fn() }));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { login, register, logout }, platform: {}, target: "web" } }));
afterEach(() => { cleanup(); sessionStore.set(null); vi.unstubAllGlobals(); vi.useRealTimers(); });
beforeEach(() => { sessionStore.set(null); login.mockReset(); register.mockReset(); logout.mockReset(); });
function form(isRegister = false) {
  render(<MemoryRouter initialEntries={[isRegister ? "/register" : "/login"]}><UserProvider><Routes><Route path="/login" element={<LoginPage />} /><Route path="/register" element={<LoginPage register />} /><Route path="/hub" element={<p>Signed in workspace</p>} /></Routes></UserProvider></MemoryRouter>);
}
function fill(label: string, value: string) { fireEvent.change(screen.getByLabelText(label), { target: { value } }); }
function autofill(label: string, value: string) {
  const input = screen.getByLabelText(label);
  if (!(input instanceof HTMLInputElement)) throw new Error(`Expected input: ${label}`);
  // Password managers can fill native values without dispatching React change events.
  input.value = value;
  return input;
}
it("submits autofilled credentials, preserves them through visibility and retry, and disables only while pending", async () => {
  login.mockRejectedValueOnce(new Error("Try again")).mockResolvedValue(testSession);
  form();
  const username = autofill("Username", "analyst");
  const password = autofill("Password", "  a sufficiently long password  ");
  const button = screen.getByRole("button", { name: "Sign in" });
  expect(button).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "Show password" }));
  expect(password.value).toBe("  a sufficiently long password  ");
  fireEvent.click(button);
  expect(screen.getByRole("button", { name: "Please wait…" })).toHaveProperty("disabled", true);
  fireEvent.click(screen.getByRole("button", { name: "Please wait…" }));
  expect(login).toHaveBeenCalledExactlyOnceWith({ username: "analyst", password: "  a sufficiently long password  " });
  expect((await screen.findByRole("alert")).textContent).toBe("Try again");
  expect(username.value).toBe("analyst");
  expect(password.value).toBe("  a sufficiently long password  ");
  expect(screen.getByRole("button", { name: "Sign in" })).toHaveProperty("disabled", false);
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  expect(await screen.findByText("Signed in workspace")).toBeTruthy();
  expect(login).toHaveBeenCalledTimes(2);
});
it("keeps empty required fields from submitting without a misleading disabled button", () => {
  form();
  const button = screen.getByRole("button", { name: "Sign in" });
  expect(button).toHaveProperty("disabled", false);
  fireEvent.click(button);
  expect(login).not.toHaveBeenCalled();
});
it("registers autofilled fields using the fixed email domain and excludes confirmation", async () => {
  register.mockResolvedValue(testSession);
  form(true);
  autofill("Username", "analyst");
  autofill("Password", "a sufficiently long password");
  autofill("Confirm password", "a sufficiently long password");
  autofill("West Monroe email", "analyst@westmonroe.com");
  autofill("First name", "Alex");
  autofill("Last name", "Morgan");
  autofill("Job title (optional)", "Analyst");
  fireEvent.click(screen.getByRole("button", { name: "Create account" }));
  expect(await screen.findByText("Signed in workspace")).toBeTruthy();
  expect(register).toHaveBeenCalledExactlyOnceWith({ username: "analyst", password: "a sufficiently long password", email: "analyst@westmonroe.com", firstName: "Alex", lastName: "Morgan", jobTitle: "Analyst" });
});
it("logs in with username/password and exposes a memory-only session", async () => {
  login.mockResolvedValue(testSession); form();
  fill("Username", "analyst"); fill("Password", "a sufficiently long password");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  expect(await screen.findByText("Signed in workspace")).toBeTruthy();
  expect(login).toHaveBeenCalledWith({ username: "analyst", password: "a sufficiently long password" });
  expect(credentials().auth?.userId).toBe(1);
  expect(window.localStorage.getItem("accessToken")).toBeNull();
  expect(window.sessionStorage.getItem("quarry.workspace.email")).toBeNull();
});
it("validates confirmation and registers without sending confirmation or a provider key", async () => {
  register.mockResolvedValue(testSession); form(true);
  expect(screen.getByText("@westmonroe.com")).toBeTruthy();
  fill("Username", "analyst"); fill("Password", "a sufficiently long password"); fill("Confirm password", "different");
  fill("West Monroe email", "analyst"); fill("First name", "Alex"); fill("Last name", "Morgan");
  fireEvent.click(screen.getByRole("button", { name: "Create account" }));
  expect(screen.getByRole("alert").textContent).toBe("Passwords do not match."); expect(register).not.toHaveBeenCalled();
  fill("Confirm password", "a sufficiently long password"); fireEvent.click(screen.getByRole("button", { name: "Create account" }));
  expect(await screen.findByText("Signed in workspace")).toBeTruthy();
  expect(register).toHaveBeenCalledWith({ username: "analyst", password: "a sufficiently long password", email: "analyst@westmonroe.com", firstName: "Alex", lastName: "Morgan" });
});
it("shows login failure and allows a retry", async () => {
  login.mockRejectedValue(new Error("Invalid credentials")); form(); fill("Username", "analyst"); fill("Password", "incorrect long password");
  fireEvent.click(screen.getByRole("button", { name: "Sign in" }));
  expect((await screen.findByRole("alert")).textContent).toBe("Invalid credentials");
  expect((screen.getByRole("button", { name: "Sign in" }) as HTMLButtonElement).disabled).toBe(false);
});
it("attaches a captured credential pair, preserves content type, and clears a current 401", async () => {
  sessionStore.set(testSession);
  const fetch = vi.fn().mockResolvedValue(new Response(null, { status: 401 })); vi.stubGlobal("fetch", fetch);
  await authFetch("/api/v1/deals", { headers: { "Content-Type": "application/json" } });
  const headers = new Headers(fetch.mock.calls[0][1].headers);
  expect(headers.get("authorization")).toBe(`Bearer ${testSession.accessToken}`); expect(headers.get("x-user-id")).toBe("1"); expect(headers.get("content-type")).toBe("application/json");
  expect(sessionStore.getSnapshot().session).toBeNull();
});
it("does not send stale credentials to public login or clear a new session for an old 401", async () => {
  sessionStore.set(testSession);
  let complete: ((response: Response) => void) | undefined;
  const fetch = vi.fn().mockImplementation(() => new Promise<Response>((resolve) => { complete = resolve; })); vi.stubGlobal("fetch", fetch);
  const oldRequest = authFetch("/api/v1/deals");
  sessionStore.set({ ...testSession, accessToken: "b".repeat(43) });
  complete?.(new Response(null, { status: 401 })); await oldRequest;
  expect(credentials().auth?.token).toBe("b".repeat(43));
  fetch.mockResolvedValue(new Response(null, { status: 200 }));
  await authFetch("/api/v1/auth/login", { method: "POST" });
  expect(new Headers(fetch.mock.calls[1][1].headers).has("authorization")).toBe(false);
});
it("expires sessions and aborts tracked work", () => {
  vi.useFakeTimers(); sessionStore.set({ ...testSession, expiresAt: new Date(Date.now() + 1000).toISOString() });
  const controller = new AbortController(); sessionStore.track(controller); vi.advanceTimersByTime(1001);
  expect(sessionStore.getSnapshot().session).toBeNull(); expect(controller.signal.aborted).toBe(true);
});
it("never records auth request or response credentials in the activity log", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(Response.json(testSession)));
  await httpQuarryApi.login({ username: "analyst", password: "a sufficiently long password" });
  const log = JSON.stringify(getActivityLogEntries());
  expect(log).not.toContain(testSession.accessToken); expect(log).not.toContain("a sufficiently long password"); expect(log).not.toContain("analyst@example.com");
});

it("authenticates document GET-SSE and handles fragmented UTF-8 terminal events", async () => {
  sessionStore.set(testSession);
  const encoder = new TextEncoder();
  const wire = encoder.encode('event: processing\ndata: {"jobId":"job-1","filename":"résumé.pdf","status":"processing"}\n\nevent: completed\ndata: {"jobId":"job-1","filename":"résumé.pdf","status":"completed"}\n\n');
  let offset = 0;
  const body = new ReadableStream<Uint8Array>({ pull(controller) {
    if (offset >= wire.length) { controller.close(); return; }
    controller.enqueue(wire.slice(offset, offset + 3)); offset += 3;
  } });
  const fetch = vi.fn().mockResolvedValue(new Response(body, { headers: { "content-type": "text/event-stream" } }));
  vi.stubGlobal("fetch", fetch);
  const onEvent = vi.fn(); const onConnectionError = vi.fn();
  const stop = httpQuarryApi.subscribeToProcessFileJob("job-1", { onEvent, onConnectionError });
  await vi.waitFor(() => expect(onEvent).toHaveBeenCalledTimes(2));
  expect(onEvent.mock.calls[1][0].status).toBe("completed");
  expect(onEvent.mock.calls[1][0].filename).toBe("résumé.pdf");
  expect(new Headers(fetch.mock.calls[0][1].headers).get("authorization")).toBe(`Bearer ${testSession.accessToken}`);
  expect(onConnectionError).not.toHaveBeenCalled(); stop();
});
