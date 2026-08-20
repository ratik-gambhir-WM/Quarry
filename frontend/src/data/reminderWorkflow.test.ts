import { describe, expect, it } from "vitest";
import { moveReminder, toWorkflowReminder } from "./reminderWorkflow";

describe("reminder workflow", () => {
  it("maps existing reminder flags to their initial workflow status", () => {
    expect(toWorkflowReminder({ id: "todo", label: "Open" }).status).toBe("to-do");
    expect(
      toWorkflowReminder({ id: "active", label: "Active", priority: true }).status,
    ).toBe("in-progress");
    expect(toWorkflowReminder({ done: true, id: "done", label: "Closed" }).status).toBe(
      "done",
    );
  });

  it("moves reminders in either direction and keeps urgency independent", () => {
    const reminders = [
      toWorkflowReminder({ id: "task-1", label: "Review contracts", priority: true }),
    ];

    const completed = moveReminder(reminders, "task-1", "done");
    expect(completed[0]).toMatchObject({ done: true, priority: true, status: "done" });

    const reopened = moveReminder(completed, "task-1", "to-do");
    expect(reopened[0]).toMatchObject({ done: false, priority: true, status: "to-do" });
  });
});
