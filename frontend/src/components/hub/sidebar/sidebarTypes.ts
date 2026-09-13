import type { WorkspaceDeal, WorkspaceLocationState, WorkspaceSidebarTool } from "../../../data/workspace";

export type ActiveHomeSection = "account" | "deals" | "hub" | "logs" | "summarize" | "tauri-playground" | "vault";

export type ActiveDealSection =
  | "activity"
  | "analysis"
  | "data-room"
  | "deal-room"
  | "deliverables";

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
};
