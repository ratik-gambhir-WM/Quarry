// @vitest-environment happy-dom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { DealActivityEvent } from "@/contracts/dealMemberships";
import { DealActivity } from "@/components/deal-room/DealActivity";
const { list } = vi.hoisted(() => ({ list: vi.fn() }));
vi.mock("@quarry/runtime", () => ({
  runtime: { api: { listDealActivity: list } },
}));
vi.mock("motion/react", async (importOriginal) => ({
  ...await importOriginal<typeof import("motion/react")>(),
  useReducedMotion: () => true,
}));
afterEach(() => {
  cleanup();
  list.mockReset();
});
it("loads and paginates activity through a subscription, retaining removed-member attribution", async () => {
  const event = {
    eventId: "event-1",
    actorName: "Former member",
    actorUserId: 2,
    kind: "document_uploaded",
    createdAt: "2026-10-01T00:00:00Z",
    targetId: "version",
  };
  list
    .mockResolvedValueOnce({ items: [event], nextCursor: "event-1" })
    .mockResolvedValueOnce({
      items: [
        {
          ...event,
          eventId: "event-2",
          actorName: null,
          kind: "membership_backfilled",
        },
      ],
      nextCursor: null,
    });
  const user = userEvent.setup();
  render(<DealActivity dealId="DEAL-1" />);
  await screen.findByText((_, element) =>
    element?.tagName === "P" && element.textContent === "Former member · document uploaded",
  );
  await user.click(screen.getByRole("button", { name: "Load more activity" }));
  await screen.findByText((_, element) =>
    element?.tagName === "P" && element.textContent === "System · membership backfilled",
  );
  expect(list).toHaveBeenLastCalledWith("DEAL-1", { cursor: "event-1" });
  expect(
    screen.queryByRole("button", { name: "Load more activity" }),
  ).toBeNull();
});
it("shows empty and retry states without an effect-managed request", async () => {
  list
    .mockRejectedValueOnce(new Error("Activity unavailable"))
    .mockResolvedValueOnce({ items: [], nextCursor: null });
  const user = userEvent.setup();
  render(<DealActivity dealId="DEAL-1" />);
  await screen.findByRole("alert");
  await user.click(screen.getByRole("button", { name: "Retry activity" }));
  await screen.findByText("No activity yet.");
});

it("retains loaded events and retries the same cursor after pagination fails", async () => {
  const event = {
    eventId: "event-1", actorName: "Former member", actorUserId: 2,
    kind: "document_uploaded", createdAt: "2026-10-01T12:00:00Z", targetId: "version",
  };
  list.mockResolvedValueOnce({ items: [event], nextCursor: "next-page" })
    .mockRejectedValueOnce(new Error("Page unavailable"))
    .mockResolvedValueOnce({ items: [event], nextCursor: null });
  const user = userEvent.setup();
  render(<DealActivity dealId="DEAL-1" />);
  await user.click(await screen.findByRole("button", { name: "Load more activity" }));
  await screen.findByRole("alert");
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
  await user.click(screen.getByRole("button", { name: "Retry activity" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  expect(list).toHaveBeenLastCalledWith("DEAL-1", { cursor: "next-page" });
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
});

it("shows a busy skeleton and ignores a stale response after changing deals", async () => {
  let resolveFirst: (value: { items: DealActivityEvent[]; nextCursor: null }) => void = () => {};
  list.mockReturnValueOnce(new Promise((resolve) => { resolveFirst = resolve; }))
    .mockResolvedValueOnce({ items: [], nextCursor: null });
  const view = render(<DealActivity dealId="DEAL-1" />);
  expect(screen.getByRole("status").textContent).toBe("Loading activity…");
  expect(screen.getByRole("region", { name: "Activity feed" }).getAttribute("aria-busy")).toBe("true");
  view.rerender(<DealActivity dealId="DEAL-2" />);
  await screen.findByText("No activity yet.");
  await act(async () => resolveFirst({
    items: [{ eventId: "stale", actorUserId: null, actorName: "Old deal actor",
      kind: "document_uploaded", createdAt: "2026-10-01T12:00:00Z", targetId: null }],
    nextCursor: null,
  }));
  expect(screen.queryByText("Old deal actor")).toBeNull();
  expect(list).toHaveBeenLastCalledWith("DEAL-2");
});
