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

export function SidebarIcon({
  className,
  name,
}: {
  className: string;
  name: SidebarIconName;
}) {
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
