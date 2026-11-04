import { hubActivityTasks, hubRecentFiles } from "../hub/activity";
import { workspaceInsights } from "../workspace/portfolio";
import { taskRow, insightRow, fileRow, type VaultTableRow } from "../../data/vaultTable";

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

