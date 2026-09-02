import type { WorkspaceSidebarTool } from "../../data/workspace";

export const workspaceInitiatives: WorkspaceSidebarTool[] = [
  { href: "/hub/initiatives/vault", icon: "personSearch", name: "Vault" },
  { icon: "terminal", name: "Software Migration" },
];

export const workspaceTools: WorkspaceSidebarTool[] = [
  { href: "/hub/logs", icon: "terminal", name: "Logs" },
];

export const defaultWorkspaceProfileSubtitle = "Focus: Project Alpha";
