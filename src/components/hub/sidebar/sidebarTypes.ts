import type { WorkspaceDeal, WorkspaceLocationState, WorkspaceSidebarTool } from "../../../data/workspace";

export type ActiveHomeSection = "account" | "hub" | "logs" | "summarize" | "tauri-playground" | "vault";

export type ActiveDealSection = "data-room" | "deal-room" | "diligence-graph" | "site-visits" | "synthesis-canvas" | "timeline";

export type DealRoomTabSection = Exclude<ActiveDealSection, "data-room">;

export type SidebarMode = "deal-room" | "home";

export type SidebarBaseProps = {
  deals: WorkspaceDeal[];
  email?: string;
  navigationState?: WorkspaceLocationState;
};

export type HomeSidebarProps = SidebarBaseProps & {
  activeHomeSection: ActiveHomeSection;
  initiatives: WorkspaceSidebarTool[];
  tools: WorkspaceSidebarTool[];
};

export type DealRoomSidebarProps = SidebarBaseProps & {
  activeDealId?: string;
  activeSection: ActiveDealSection;
  onDealRoomSectionChange?: (section: DealRoomTabSection) => void;
};
