export type SidebarSpaceIcon =
  | "bookmark"
  | "checkCircle"
  | "dashboard"
  | "dataset"
  | "doc"
  | "folderOpen"
  | "graph"
  | "grid"
  | "home"
  | "listAlt"
  | "notification"
  | "person"
  | "search"
  | "settings"
  | "shield"
  | "sparkles"
  | "timeline";

export type SidebarSpaceId = "current" | "diligence" | "operations" | "research";

export type SidebarSpace = {
  description: string;
  icon: SidebarSpaceIcon;
  id: Exclude<SidebarSpaceId, "current">;
  label: string;
  sections: Array<{
    items: Array<{
      icon: SidebarSpaceIcon;
      id: string;
      label: string;
    }>;
    title: string;
  }>;
};
