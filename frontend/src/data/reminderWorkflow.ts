import type { DealTask } from "./workspace";

export type ReminderStatus = "done" | "in-progress" | "to-do";

export type WorkflowReminder = DealTask & {
  status: ReminderStatus;
};

export function toWorkflowReminder(task: DealTask): WorkflowReminder {
  return {
    ...task,
    status: task.done ? "done" : task.priority ? "in-progress" : "to-do",
  };
}

export function moveReminder(
  reminders: WorkflowReminder[],
  reminderId: string,
  status: ReminderStatus,
): WorkflowReminder[] {
  return reminders.map((reminder) =>
    reminder.id === reminderId
      ? { ...reminder, done: status === "done", status }
      : reminder,
  );
}
