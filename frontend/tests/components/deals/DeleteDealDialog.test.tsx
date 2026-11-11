// @vitest-environment happy-dom
import { useCallback, useRef, useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { DeleteDealDialog } from "@/components/deals/DeleteDealDialog";
import { sessionStore } from "@/auth/sessionStore";
import type { DealDeletion } from "@/contracts/quarryApi";
import { testSession } from "@/test/fixtures/auth";

const { deleteDeal, getDealDeletion } = vi.hoisted(() => ({
  deleteDeal: vi.fn(),
  getDealDeletion: vi.fn(),
}));
vi.mock("@quarry/runtime", () => ({ runtime: { api: { deleteDeal, getDealDeletion } } }));

// Use Motion's JS fallback: happy-dom rejects canceled Web Animations promises.
const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => {
  if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor);
});

function receipt(status: DealDeletion["status"] = "pending"): DealDeletion {
  return {
    deletionId: "operation-1", dealId: "DEAL-1", status,
    phase: status === "completed" ? "completed" : "draining",
    retryable: false, statusUrl: "/api/v1/deal-deletions/operation-1",
    errorCode: null, createdAt: "2026-11-10T00:00:00Z", updatedAt: "2026-11-10T00:00:00Z",
    completedAt: status === "completed" ? "2026-11-10T00:00:01Z" : null,
  };
}

function Harness() {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const fallback = useRef<HTMLDivElement>(null);
  const onClose = useCallback(() => setOpen(false), []);
  return <div ref={fallback} tabIndex={-1}>
    <button ref={trigger} onClick={() => setOpen(true)}>Open deletion</button>
    {open ? <DeleteDealDialog deal={{ id: "DEAL-1", name: "Project Alpha", trigger: trigger.current }} fallbackFocus={fallback} onClose={onClose} /> : null}
  </div>;
}

async function openDialog() {
  const user = userEvent.setup();
  render(<Harness />);
  await user.click(screen.getByRole("button", { name: "Open deletion" }));
  return user;
}

async function confirmDeletion(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByRole("textbox", { name: "Deletion confirmation" }), "delete Project Alpha");
  await user.click(screen.getByRole("button", { name: "Delete deal" }));
}

beforeEach(() => {
  sessionStore.set(testSession);
  vi.resetAllMocks();
  deleteDeal.mockResolvedValue(null);
  getDealDeletion.mockResolvedValue(receipt("completed"));
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("DeleteDealDialog", () => {
  it("requires the exact case-sensitive phrase and cancelling restores focus without deleting", async () => {
    const user = await openDialog();
    const submit = screen.getByRole("button", { name: "Delete deal" }) as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    const input = screen.getByRole("textbox", { name: "Deletion confirmation" });
    await user.type(input, "delete project alpha");
    expect(submit.disabled).toBe(true);
    fireEvent.submit(input.closest("form") as HTMLFormElement);
    expect(deleteDeal).not.toHaveBeenCalled();
    await user.clear(input);
    await user.type(input, "delete Project Alpha ");
    expect(submit.disabled).toBe(true);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("button", { name: "Open deletion" })));
    expect(deleteDeal).not.toHaveBeenCalled();
  });

  it("submits once, prevents dismissal while acceptance is unknown, and handles completed 204", async () => {
    let finish: (value: null) => void = () => {};
    deleteDeal.mockReturnValue(new Promise<null>((resolve) => { finish = resolve; }));
    const user = await openDialog();
    await confirmDeletion(user);
    fireEvent.submit(screen.getByRole("textbox").closest("form") as HTMLFormElement);
    await user.keyboard("{Escape}");
    expect(screen.getByRole("alertdialog")).toBeTruthy();
    expect((screen.getByRole("button", { name: "Cancel" }) as HTMLButtonElement).disabled).toBe(true);
    expect(deleteDeal).toHaveBeenCalledExactlyOnceWith("DEAL-1");
    await act(async () => finish(null));
    expect(await screen.findByText("Deal deleted")).toBeTruthy();
    expect(getDealDeletion).not.toHaveBeenCalled();
  });

  it("preserves the phrase on a failed acceptance and allows a deliberate retry", async () => {
    deleteDeal.mockRejectedValueOnce(new Error("Unavailable"));
    const user = await openDialog();
    await confirmDeletion(user);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect((screen.getByRole("textbox") as HTMLInputElement).value).toBe("delete Project Alpha");
    await user.click(screen.getByRole("button", { name: "Delete deal" }));
    expect(await screen.findByText("Deal deleted")).toBeTruthy();
    expect(deleteDeal).toHaveBeenCalledTimes(2);
  });

  it("keeps acceptance distinct from completion and polls the receipt", async () => {
    deleteDeal.mockResolvedValue(receipt());
    const user = await openDialog();
    await confirmDeletion(user);
    expect(screen.getByText("Deletion is in progress…")).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Continue in background" }));
    expect(screen.queryByText("Deal deleted")).toBeNull();
    await waitFor(() => expect(screen.getByText("Deal deleted")).toBeTruthy(), { timeout: 2500 });
    expect(getDealDeletion).toHaveBeenCalledExactlyOnceWith("operation-1");
    expect(deleteDeal).toHaveBeenCalledTimes(1);
  });

  it("rechecks status after a polling error without issuing another deletion", async () => {
    deleteDeal.mockResolvedValue(receipt());
    getDealDeletion.mockRejectedValueOnce(new Error("Offline"));
    const user = await openDialog();
    await confirmDeletion(user);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Deletion has already been accepted"), { timeout: 2500 });
    await user.click(screen.getByRole("button", { name: "Check status" }));
    expect(await screen.findByText("Deal deleted")).toBeTruthy();
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close" }));
    expect(getDealDeletion).toHaveBeenCalledTimes(2);
    expect(deleteDeal).toHaveBeenCalledTimes(1);
  });

  it("reports blocked cleanup honestly with an operation reference", async () => {
    deleteDeal.mockResolvedValue(receipt("blocked"));
    const user = await openDialog();
    await confirmDeletion(user);
    expect(await screen.findByRole("alert")).toBeTruthy();
    expect(screen.getByText("Operation: operation-1")).toBeTruthy();
    expect(screen.queryByText("Deal deleted")).toBeNull();
    expect(getDealDeletion).not.toHaveBeenCalled();
  });

  it("stops polling when closed while durable cleanup continues", async () => {
    deleteDeal.mockResolvedValue(receipt());
    const user = await openDialog();
    await confirmDeletion(user);
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Continue in background" }));
    await act(() => vi.advanceTimersByTimeAsync(1500));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(getDealDeletion).not.toHaveBeenCalled();
  });

  it("closes and ignores a late response when the session changes", async () => {
    let finish: (value: null) => void = () => {};
    deleteDeal.mockReturnValue(new Promise<null>((resolve) => { finish = resolve; }));
    const user = await openDialog();
    await confirmDeletion(user);
    act(() => sessionStore.set(null));
    await act(async () => finish(null));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.queryByText("Deal deleted")).toBeNull();
  });
});
