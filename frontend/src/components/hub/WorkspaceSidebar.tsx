import type { WorkspaceSidebarTool } from "../../data/workspace";
import { DealRoomWorkspaceSidebar } from "./sidebar/DealRoomWorkspaceSidebar";
import { HomeWorkspaceSidebar } from "./sidebar/HomeWorkspaceSidebar";
import type { ActiveDealSection, ActiveHomeSection, SidebarBaseProps, SidebarMode } from "./sidebar/sidebarTypes";

type WorkspaceSidebarProps = SidebarBaseProps & {
  activeDealId?: string;
  activeHomeSection?: ActiveHomeSection;
  activeSection?: ActiveDealSection;
  initiatives?: WorkspaceSidebarTool[];
  mode?: SidebarMode;
  tools?: WorkspaceSidebarTool[];
};

export function WorkspaceSidebar({
  activeDealId,
  activeHomeSection = "hub",
  activeSection = "deal-room",
  deals,
  email,
  initiatives = [],
  mode = "home",
  navigationState,
  tools = [],
}: WorkspaceSidebarProps) {
  if (mode === "deal-room") {
    return (
      <DealRoomWorkspaceSidebar
        activeDealId={activeDealId}
        activeSection={activeSection}
        deals={deals}
        email={email}
        navigationState={navigationState}
      />
    );
  }

  return (
    <HomeWorkspaceSidebar
      activeHomeSection={activeHomeSection}
      deals={deals}
      email={email}
      initiatives={initiatives}
      navigationState={navigationState}
      tools={tools}
    />
  );
}
