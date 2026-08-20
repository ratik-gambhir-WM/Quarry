import { useState } from "react";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";
import { getTeamLabel, WorkspaceDeal, WorkspaceLocationState } from "../../../data/workspace";
import { WestMonroeMark } from "../../brand/WestMonroeMark";
import { Icon } from "../../ui/Icon";
import { ProfilePreferences } from "./ProfilePreferences";

type SidebarFrameProps = {
  alignedHeader?: boolean;
  children: ReactNode | ((state: { collapsed: boolean }) => ReactNode);
  email?: string;
  navigationState?: WorkspaceLocationState;
  profileDeal?: WorkspaceDeal;
  profileSubtitle?: string;
};

export function SidebarFrame({
  alignedHeader = false,
  children,
  email,
  navigationState,
  profileDeal,
  profileSubtitle,
}: SidebarFrameProps) {
  const [collapsed, setCollapsed] = useState(false);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const teamLabel = getTeamLabel(email);
  const subtitle = profileDeal ? profileDeal.room.name : profileSubtitle ?? "Focus: Project Alpha";

  return (
    <aside
      className={`hidden h-full shrink-0 overflow-hidden border-r border-outline-variant/70 bg-[#f7f7f7] text-sidebar-text backdrop-blur-md transition-[width] duration-300 ease-out lg:flex [font-family:var(--font-sidebar)] [html[data-theme=dark]_&]:bg-[#141414] ${
        collapsed ? "w-20" : "w-72"
      }`}
    >
      <div className="flex h-full min-h-0 w-full flex-col">
        <div
          className={
            alignedHeader
              ? `flex h-16 shrink-0 items-center justify-between gap-1 border-b border-outline-variant/70 ${
                  collapsed ? "px-2" : "px-4"
                }`
              : `flex shrink-0 items-center justify-between gap-1 pt-4 ${collapsed ? "px-2" : "px-4"}`
          }
        >
          <NavLink
            aria-label="Back to home page"
            className={`flex min-w-0 items-center rounded-lg py-2 transition hover:bg-sidebar-hover ${
              collapsed ? "gap-0 px-0" : alignedHeader ? "gap-3 px-0" : "gap-3 px-2"
            }`}
            state={navigationState}
            to="/hub"
          >
            <WestMonroeMark className={collapsed ? "h-[1.4rem] w-[1.4rem]" : "h-8 w-8"} framed />
            {collapsed ? null : (
              <h1 className="text-[13px] font-semibold leading-5 text-sidebar-active">
                Quarry
              </h1>
            )}
          </NavLink>

          <button
            aria-controls="workspace-sidebar-navigation"
            aria-expanded={!collapsed}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            className="flex h-[1.6rem] w-[1.6rem] shrink-0 items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
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
            } ${collapsed ? "[&_h2]:hidden [&_span]:hidden" : ""}`}
          >
            {typeof children === "function" ? children({ collapsed }) : children}
          </div>

          <div className="shrink-0 border-t border-outline-variant/70 pt-2">
            <div className="relative">
              {!collapsed && profileMenuOpen ? <ProfilePreferences email={email} navigationState={navigationState} /> : null}
              <button
                aria-expanded={!collapsed && profileMenuOpen}
                aria-haspopup={collapsed ? undefined : "menu"}
                aria-label={collapsed ? `Expand sidebar for ${teamLabel} profile` : undefined}
                className={`flex w-full items-center rounded-lg py-2 text-left transition hover:bg-sidebar-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
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
                    <p className="truncate text-[13px] font-medium leading-5 text-sidebar-active">{teamLabel}</p>
                    <p className="truncate text-[11px] font-normal leading-4 text-sidebar-muted">{subtitle}</p>
                  </div>
                )}
              </button>
            </div>
          </div>
        </div>
      </div>
    </aside>
  );
}
