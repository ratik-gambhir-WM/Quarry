// @vitest-environment happy-dom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { DealActivity } from "@/components/deal-room/DealActivity";
const { list } = vi.hoisted(() => ({ list: vi.fn() }));
vi.mock("@quarry/runtime", () => ({
  runtime: { api: { listDealActivity: list } },
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
  await screen.findByText("Former member · document uploaded");
  await user.click(screen.getByRole("button", { name: "Load more activity" }));
  await screen.findByText("System · membership backfilled");
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
