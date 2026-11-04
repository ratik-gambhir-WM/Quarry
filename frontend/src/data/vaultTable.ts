import type { hubActivityTasks, hubRecentFiles } from "../fixtures/hub/activity";
import type { workspaceInsights } from "../fixtures/workspace/portfolio";

type VaultTableTypeIcon = "bookmark" | "checkCircle" | "doc" | "pdf" | "sheet";

export type VaultTableRow = {
  creator: string | null;
  description: string;
  id: string;
  name: string;
  practiceArea: string | null;
  project: string;
  type: "Document" | "Insight" | "Task";
  typeIcon: VaultTableTypeIcon;
};

export function taskRow(
  id: string,
  task: (typeof hubActivityTasks)[number],
): VaultTableRow {
  return {
    creator: task.creator,
    description: task.checked
      ? `Completed task · Assigned to: ${task.creator}`
      : `Assigned to: ${task.creator}`,
    id,
    name: task.label,
    practiceArea: null,
    project: task.practiceArea,
    type: "Task",
    typeIcon: "checkCircle",
  };
}

export function insightRow(
  id: string,
  insight: (typeof workspaceInsights)[number],
): VaultTableRow {
  return {
    creator: null,
    description: `Source: ${insight.fileName}`,
    id,
    name: insight.quote,
    practiceArea: insight.category,
    project: insight.deal,
    type: "Insight",
    typeIcon: "bookmark",
  };
}

export function fileRow(
  id: string,
  file: (typeof hubRecentFiles)[number],
): VaultTableRow {
  return {
    creator: null,
    description: `Recently opened · ${file.time}`,
    id,
    name: file.title,
    practiceArea: null,
    project: file.deal,
    type: "Document",
    typeIcon: file.icon,
  };
}
