import { cn } from "@/lib/utils";
import { AnimatedIcon } from "../../ui/AnimatedIcon";
import type { AnimatedIconName } from "../../ui/AnimatedIcon";
import { Icon } from "../../ui/Icon";
import { AssistantIcon } from "../../ui/workspace-tool-icons";

export type SidebarIconName =
  | AnimatedIconName
  | "assistant"
  | "bookmark"
  | "briefcase"
  | "dashboard"
  | "dataset"
  | "folderOpen"
  | "graph"
  | "grid"
  | "listAlt"
  | "person"
  | "personSearch"
  | "search"
  | "sparkles"
  | "terminal"
  | "timeline";

const animatedIconNames = new Set<SidebarIconName>([
  "bookOpen",
  "boxes",
  "calendarDays",
  "fileStack",
  "folderTree",
  "home",
  "layers",
  "ship",
  "telescope",
  "twitch",
]);

const iconColors: Partial<Record<SidebarIconName, string>> = {
  assistant: "text-icon-assistant",
  sparkles: "text-icon-assistant",
  telescope: "text-icon-assistant",
  bookOpen: "text-icon-knowledge",
  fileStack: "text-icon-knowledge",
  graph: "text-icon-knowledge",
  timeline: "text-icon-knowledge",
  dataset: "text-icon-knowledge",
  folderOpen: "text-icon-files",
  folderTree: "text-icon-files",
  bookmark: "text-icon-files",
  layers: "text-icon-files",
};

export function SidebarIcon({
  className,
  name,
  inheritColor = false,
}: {
  className: string;
  name: SidebarIconName;
  inheritColor?: boolean;
}) {
  className = cn(inheritColor ? "text-current" : iconColors[name] ?? "text-icon-navigation", className);
  if (name === "assistant") return <AssistantIcon className={className} />;
  return isAnimatedIconName(name) ? (
    <AnimatedIcon className={className} name={name} />
  ) : (
    <Icon className={className} name={name} />
  );
}

function isAnimatedIconName(name: SidebarIconName): name is AnimatedIconName {
  return animatedIconNames.has(name);
}
