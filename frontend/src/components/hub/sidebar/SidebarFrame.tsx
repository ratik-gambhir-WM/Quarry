import { useState } from "react";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { getTeamLabel, WorkspaceDeal, WorkspaceLocationState } from "../../../data/workspace";
import { WestMonroeMark } from "../../brand/WestMonroeMark";
import { Icon } from "../../ui/Icon";
import { ProfilePreferences } from "./ProfilePreferences";
import { MockSidebarNavigation, SidebarSpaceId, SidebarSwitcher } from "./SidebarSwitcher";

type SidebarFrameProps = {
  alignedHeader?: boolean;
  children: ReactNode | ((state: { collapsed: boolean }) => ReactNode);
  email?: string;
  headerBackLabel?: string;
  headerBackTo?: string;
  showHeaderBackButton?: boolean;
  navigationState?: WorkspaceLocationState;
  profileDeal?: WorkspaceDeal;
  profileSubtitle?: string;
  sidebarIcon?: "dashboard" | "folderOpen" | "home";
  sidebarLabel?: string;
};

export function SidebarFrame({
  alignedHeader = false,
  children,
  email,
  headerBackLabel = "Back to home page",
  headerBackTo = "/hub",
  navigationState,
  profileDeal,
  profileSubtitle,
  sidebarIcon = "home",
  sidebarLabel = "Workspace",
  showHeaderBackButton = true,
}: SidebarFrameProps) {
  const [activeSidebarSpaceId, setActiveSidebarSpaceId] = useState<SidebarSpaceId>("current");
  const [collapsed, setCollapsed] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const teamLabel = getTeamLabel(email);
  const subtitle = profileDeal ? profileDeal.room.name : profileSubtitle ?? "Focus: Project Alpha";

  return (
    <aside
      className={`workspace-sidebar hidden h-full shrink-0 overflow-hidden text-sidebar-text transition-[width] duration-300 ease-out lg:flex [font-family:var(--font-sidebar)] ${
        collapsed ? "w-20" : "w-72"
      }`}
    >
      <div className="flex h-full min-h-0 w-full flex-col">
        <div
          className={
            alignedHeader
              ? `relative flex h-12 shrink-0 items-center gap-2 ${
                  collapsed ? "px-2" : "px-4"
                }`
              : `flex shrink-0 items-center gap-2 pt-4 ${collapsed ? "px-2" : "px-4"}`
          }
        >
          {showHeaderBackButton ? (
            <NavLink
              aria-label={headerBackLabel}
              className="group flex h-8 w-8 items-center justify-center rounded-lg text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
              state={navigationState}
              title={headerBackLabel}
              to={headerBackTo}
            >
              <Icon
                className="h-4 w-4 rotate-180 transition-transform group-hover:-translate-x-0.5"
                name="arrowRight"
              />
            </NavLink>
          ) : null}

          {collapsed ? null : (
            <SidebarSwitcher
              activeSpaceId={activeSidebarSpaceId}
              currentIcon={sidebarIcon}
              currentLabel={sidebarLabel}
              onOpenChange={(open) => {
                if (open) {
                  setProfileMenuOpen(false);
                }
              }}
              onSpaceChange={setActiveSidebarSpaceId}
            />
          )}

          <button
            aria-controls="workspace-sidebar-navigation"
            aria-expanded={!collapsed}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className={`${collapsed && !showHeaderBackButton ? "mx-auto" : "ml-auto"} flex h-[1.6rem] w-[1.6rem] shrink-0 items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed`}
            onClick={() => {
              setCollapsed((isCollapsed) => !isCollapsed);
              setProfileMenuOpen(false);
            }}
            type="button"
          >
            <Icon className={`h-3.5 w-3.5 transition-transform ${collapsed ? "rotate-180" : ""}`} name="sidebar" />
          </button>
        </div>

        <div className={`flex min-h-0 flex-1 flex-col ${collapsed ? "p-2" : "p-4"}`}>
          <div
            id="workspace-sidebar-navigation"
            className={`min-h-0 flex-1 overflow-y-auto pb-4 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden ${
              alignedHeader ? "pt-2" : "pt-6"
            } ${
              collapsed
                ? "[&_h2]:hidden [&_span]:hidden [&_nav>a]:flex [&_nav>a]:justify-center [&_nav>a]:px-0 [&_nav>button]:flex [&_nav>button]:justify-center [&_nav>button]:px-0"
                : ""
            }`}
          >
            {activeSidebarSpaceId === "current" ? (
              typeof children === "function" ? children({ collapsed }) : children
            ) : (
              <MockSidebarNavigation
                key={activeSidebarSpaceId}
                spaceId={activeSidebarSpaceId}
              />
            )}
          </div>

          <div className="shrink-0 border-t border-outline-variant/70 pt-2">
            <div className="relative">
              {!collapsed && profileMenuOpen ? <ProfilePreferences email={email} navigationState={navigationState} /> : null}
              <div className="flex items-center gap-2">
                <button
                  aria-expanded={!collapsed && profileMenuOpen}
                  aria-haspopup={collapsed ? undefined : "menu"}
                  aria-label={collapsed ? `Expand sidebar for ${teamLabel} profile` : undefined}
                  className={`flex min-w-0 flex-1 items-center rounded-lg py-2 text-left transition hover:bg-sidebar-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
                    collapsed ? "justify-center px-0" : "gap-3 px-3"
                  }`}
                  onClick={() => {
                    if (collapsed) {
                      setCollapsed(false);
                      return;
                    }
                    setProfileMenuOpen((isOpen) => !isOpen);
                  }}
                  type="button"
                >
                  <div
                    className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-secondary-fixed-dim text-white"
                  >
                    <span className="text-[12px] font-semibold">
                      {teamLabel.slice(0, 1)}
                    </span>
                  </div>
                  {collapsed ? null : (
                    <div className="min-w-0">
                      <p className="truncate text-[13px] font-medium leading-5 text-sidebar-active">
                        {teamLabel}
                      </p>
                      <p className="truncate text-[11px] font-normal leading-4 text-sidebar-muted">{subtitle}</p>
                    </div>
                  )}
                </button>
                {collapsed ? null : (
                  <div
                    aria-label="Quarry"
                    className="flex h-8 w-8 shrink-0 items-center justify-center"
                    role="img"
                    title="Quarry"
                  >
                    <WestMonroeMark className="h-7 w-7" framed />
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}
