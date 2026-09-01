import { useState } from "react";
import { DealTask } from "../../data/workspace";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";
import {
  TimelineEntry,
  TimelineList,
  TimelinePanelAction,
  TimelinePanelHeader,
} from "./TimelinePanel";

type PendingTasksTimelineCardProps = {
  tasks: DealTask[];
};

export function PendingTasksTimelineCard({ tasks }: PendingTasksTimelineCardProps) {
  const [checkedState, setCheckedState] = useState(() => tasks.map((task) => Boolean(task.done)));

  return (
    <WorkspaceCard className="col-span-12 flex min-h-[540px] flex-col p-6 xl:col-span-4" radius="compact">
      <TimelinePanelHeader className="mb-6" title="Pending Tasks" />

      <TimelineList>
        {tasks.map((task, index) => {
          const isChecked = checkedState[index] ?? false;
          const statusLabel = isChecked ? "Completed" : task.priority ? "High Priority" : "Open Task";
          const detailClassName = isChecked
            ? "border-primary/20 bg-primary/6 text-primary"
            : task.priority
              ? "border-error/20 bg-error/5 text-error"
              : "border-white/70 bg-white/62 text-text-main/78";

          return (
            <TimelineEntry
              key={task.id}
              marker={
                <button
                  aria-label={`${isChecked ? "Mark incomplete" : "Mark complete"}: ${task.label}`}
                  aria-pressed={isChecked}
                  className="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-white/90 bg-white/85 shadow-[0_8px_18px_rgba(7,1,84,0.06)] transition hover:scale-[1.03]"
                  onClick={() =>
                    setCheckedState((current) => current.map((value, valueIndex) => (valueIndex === index ? !value : value)))
                  }
                  type="button"
                >
                  {isChecked ? (
                    <div className="flex h-6 w-6 items-center justify-center rounded-full bg-primary text-white">
                      <Icon className="h-3.5 w-3.5" name="check" />
                    </div>
                  ) : (
                    <span className={`h-3.5 w-3.5 rounded-full ${task.priority ? "bg-error" : "bg-accent"}`} />
                  )}
                </button>
              }
            >
              <p className="text-[12px] font-medium text-muted">{statusLabel}</p>
              <h3 className={`text-[1.02rem] font-semibold ${isChecked ? "text-text-main/55 line-through" : "text-text-main"}`}>
                {task.label}
              </h3>
              <div className={`rounded-[11px] border px-4 py-3 text-[1rem] leading-7 ${detailClassName}`}>
                {isChecked
                  ? "Task completed."
                  : task.priority
                    ? "Needs immediate follow-up and should be addressed before the next review cycle."
                    : "Click the circle to mark this diligence task complete."}
              </div>
            </TimelineEntry>
          );
        })}
      </TimelineList>

      <TimelinePanelAction>View Full List</TimelinePanelAction>
    </WorkspaceCard>
  );
}
