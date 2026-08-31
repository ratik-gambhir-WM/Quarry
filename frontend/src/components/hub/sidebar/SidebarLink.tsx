import { NavLink } from "react-router-dom";
import { WorkspaceLocationState } from "../../../data/workspace";
import { Icon } from "../../ui/Icon";
import type { ActiveHomeSection } from "./sidebarTypes";

type SidebarLinkProps = {
  homeSection?: ActiveHomeSection;
  href?: string;
  icon: "bookmark" | "bookOpen" | "boxes" | "dataset" | "folderOpen" | "grid" | "personSearch" | "search" | "sparkles" | "telescope" | "terminal" | "timeline";
  label: string;
  navigationState?: WorkspaceLocationState;
};

export function SidebarLink({ homeSection, href, icon, label, navigationState }: SidebarLinkProps) {
  if (href) {
    const isVaultLink = label === "Global Vault";

    return (
      <NavLink
        aria-label={label}
        className={({ isActive }) =>
          [
            "grid w-full grid-cols-[1.25rem_minmax(0,1fr)] items-center gap-3 rounded-lg px-3 py-2 text-left transition",
            isActive && (!isVaultLink || homeSection === "vault")
              ? "bg-sidebar-selected font-medium text-sidebar-active"
              : "font-normal text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active",
          ].join(" ")
        }
        end
        state={navigationState}
        to={href}
      >
        <Icon className="h-5 w-5 text-current" name={icon} />
        <span className="min-w-0 truncate text-[13px] leading-5">{label}</span>
      </NavLink>
    );
  }

  return (
    <button
      aria-label={label}
      className="grid w-full grid-cols-[1.25rem_minmax(0,1fr)] items-center gap-3 rounded-lg px-3 py-2 text-left font-normal text-sidebar-text transition hover:bg-sidebar-hover hover:text-sidebar-active"
      type="button"
    >
      <Icon className="h-5 w-5 text-sidebar-muted" name={icon} />
      <span className="min-w-0 truncate text-[13px] leading-5">{label}</span>
    </button>
  );
}
