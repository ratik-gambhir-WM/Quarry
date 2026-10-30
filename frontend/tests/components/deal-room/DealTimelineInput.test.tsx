// @vitest-environment happy-dom
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import { DealTimelineView } from "@/components/deal-room/DealTimelineView";
import type { DealRoomData } from "@/data/workspace";

const animationDescriptor = Object.getOwnPropertyDescriptor(Element.prototype, "animate");
beforeAll(() => { Reflect.deleteProperty(Element.prototype, "animate"); });
afterAll(() => { if (animationDescriptor) Object.defineProperty(Element.prototype, "animate", animationDescriptor); });
afterEach(cleanup);

it("keeps the activity dialog open when the calendar closes and saves a typed date", async () => {
  const user = userEvent.setup();
  const onEventsChange = vi.fn();
  const deal: DealRoomData = {
    id: "test", name: "Test", overviewSubtitle: "", phaseLabel: "", sectorLabel: "",
    stageLabel: "", summary: "", keyQuestions: [], metrics: [], pendingTasks: [], resources: [], timeline: [],
  };
  render(<DealTimelineView deal={deal} events={[]} onEventsChange={onEventsChange} />);
  await user.click(screen.getByRole("button", { name: "Log Activity" }));
  const dialog = screen.getByRole("dialog", { name: "New Activity" });
  await user.type(within(dialog).getByLabelText("Activity Title"), "Review");
  await user.click(within(dialog).getByRole("button", { name: "Choose activity date" }));
  await user.keyboard("{Escape}");
  expect(screen.getByRole("dialog", { name: "New Activity" })).toBeDefined();
  const date = within(dialog).getByLabelText("Date");
  await user.type(date, "not a date");
  await user.keyboard("{Escape}");
  expect(screen.getByRole("dialog", { name: "New Activity" })).toBeDefined();
  await user.type(date, "2026-10-15");
  await user.tab();
  await user.click(within(dialog).getByRole("button", { name: "Log Activity" }));
  await waitFor(() => expect(onEventsChange).toHaveBeenCalledWith([expect.objectContaining({ date: "2026-10-15", title: "Review" })]));
});
