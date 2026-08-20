import { useState } from "react";
import { DealTask } from "../../data/workspace";
import { WorkspaceCard } from "../hub/WorkspaceCard";
import { Icon } from "../ui/Icon";

type PendingTasksCardProps = {
  className?: string;
  embedded?: boolean;
  tasks: DealTask[];
};

export function PendingTasksCard({ className = "", embedded = false, tasks }: PendingTasksCardProps) {
  const [checkedState, setCheckedState] = useState(() => tasks.map((task) => Boolean(task.done)));

  const content = (
    <div className={`space-y-4 ${className}`}>
      <div className="flex items-center gap-3">
        <Icon className={`${embedded ? "h-5 w-5" : "h-6 w-6"} text-primary`} name="checkCircle" />
        <h2 className={`${embedded ? "text-[1rem]" : "type-h3"} font-semibold text-text-main`}>Pending Tasks</h2>
      </div>

      <div className={`grid gap-3 ${embedded ? "lg:grid-cols-3" : "xl:grid-cols-3"}`}>
        {tasks.map((task, index) => {
          const isChecked = checkedState[index] ?? false;

          return (
            <label
              className={`flex cursor-pointer items-start gap-3 rounded-[11px] border px-4 py-3 transition hover:bg-white/58 ${
                embedded ? "border-white/60 bg-white/28" : "border-white/70 bg-white/38"
              }`}
              key={task.id}
            >
              <input
                checked={isChecked}
                className={`${embedded ? "mt-1 h-4 w-4" : "h-5 w-5"} rounded border-white/80 bg-white/80 accent-primary`}
                onChange={() =>
                  setCheckedState((current) => current.map((value, valueIndex) => (valueIndex === index ? !value : value)))
                }
                type="checkbox"
              />
              <span
                className={`flex items-center gap-2 ${embedded ? "text-[0.92rem] leading-6" : "text-[1rem] font-medium"} ${
                  task.priority ? "text-error" : "text-text-main"
                }`}
              >
                {task.label}
                {task.priority ? <Icon className="h-4 w-4 shrink-0" name="alert" /> : null}
              </span>
            </label>
          );
        })}
      </div>
    </div>
  );

  if (embedded) {
    return content;
  }

  return (
    <WorkspaceCard className="col-span-12 p-6" radius="compact">
      {content}
    </WorkspaceCard>
  );
}
