import { hubActivityTasks, hubRecentFiles } from "../fixtures/hub/activity";
import { workspaceInsights } from "../fixtures/workspace/portfolio";

type VaultTableTypeIcon = "bookmark" | "checkCircle" | "doc" | "pdf" | "sheet";

export type VaultTableRow = {
  creator: string;
  description: string;
  id: string;
  name: string;
  practiceArea: string;
  type: "Document" | "Insight" | "Task";
  typeIcon: VaultTableTypeIcon;
};

export const vaultTableRows = [
  taskRow("task-q3-extract", hubActivityTasks[0]),
  insightRow("insight-capex", workspaceInsights[0]),
  fileRow("file-meeting-minutes", hubRecentFiles[1]),
  taskRow("task-legal-disclosures", hubActivityTasks[1]),
  insightRow("insight-distribution", workspaceInsights[1]),
  fileRow("file-q3-report", hubRecentFiles[0]),
  taskRow("task-transcript-summary", hubActivityTasks[2]),
  fileRow("file-logistics-due-diligence", hubRecentFiles[2]),
  taskRow("task-data-room", hubActivityTasks[3]),
] satisfies readonly VaultTableRow[];

function taskRow(
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
    practiceArea: task.practiceArea,
    type: "Task",
    typeIcon: "checkCircle",
  };
}

function insightRow(
  id: string,
  insight: (typeof workspaceInsights)[number],
): VaultTableRow {
  return {
    creator: insight.deal,
    description: `Source: ${insight.fileName}`,
    id,
    name: insight.quote,
    practiceArea: insight.category,
    type: "Insight",
    typeIcon: "bookmark",
  };
}

function fileRow(
  id: string,
  file: (typeof hubRecentFiles)[number],
): VaultTableRow {
  return {
    creator: file.creator,
    description: `Recently opened · ${file.time}`,
    id,
    name: file.title,
    practiceArea: file.deal,
    type: "Document",
    typeIcon: file.icon,
  };
}
