import { useState } from "react";
import { DealTask } from "../../data/workspace";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";

type PendingTasksTimelineCardProps = {
  tasks: DealTask[];
};

export function PendingTasksTimelineCard({ tasks }: PendingTasksTimelineCardProps) {
  const [checkedState, setCheckedState] = useState(() => tasks.map((task) => Boolean(task.done)));

  return (
    <WorkspaceCard className="col-span-12 flex min-h-[540px] flex-col p-6 xl:col-span-4" radius="compact">
      <div className="mb-6 flex items-center justify-between">
        <h2 className="type-h2 text-text-main">Pending Tasks</h2>
        <button className="inline-flex h-10 w-10 items-center justify-center rounded-full transition hover:bg-white/50">
          <Icon className="h-5 w-5 text-muted" name="more" />
        </button>
      </div>

      <div className="workspace-scrollbar-hidden relative flex-1 overflow-y-auto pr-2">
        <div className="absolute bottom-6 left-4 top-4 w-px bg-muted/30" />

        <div className="relative z-10 space-y-7">
          {tasks.map((task, index) => {
            const isChecked = checkedState[index] ?? false;
            const statusLabel = isChecked ? "Completed" : task.priority ? "High Priority" : "Open Task";
            const detailClassName = isChecked
              ? "border-primary/20 bg-primary/6 text-primary"
              : task.priority
                ? "border-error/20 bg-error/5 text-error"
                : "border-white/70 bg-white/62 text-text-main/78";

            return (
              <article className="flex gap-4" key={task.id}>
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

                <div className="space-y-2 pb-1">
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
                </div>
              </article>
            );
          })}
        </div>
      </div>

      <button className="mt-5 rounded-2xl border border-white/85 bg-white/55 px-4 py-3 text-[15px] font-semibold text-text-main transition hover:bg-white/75">
        View Full List
      </button>
    </WorkspaceCard>
  );
}
