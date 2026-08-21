import { NavLink } from "react-router-dom";
import { getDataRoomPath, getDealRoomPath } from "../../../data/workspace";
import { Icon } from "../../ui/Icon";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarStaticItem } from "./SidebarStaticItem";
import type { DealRoomSidebarProps, DealRoomTabSection } from "./sidebarTypes";

const dealRoomSidebarLinks = [
  { icon: "dashboard" as const, key: "deal-room" as const, label: "Deal Room" },
  { icon: "timeline" as const, key: "timeline" as const, label: "Deal Activity" },
  { icon: "person" as const, key: "site-visits" as const, label: "Site Visits" },
  { icon: "folderOpen" as const, key: "data-room" as const, label: "Data Room Vault" },
  { icon: "listAlt" as const, key: "deliverables" as const, label: "Deliverables" },
];

export function DealRoomWorkspaceSidebar({
  activeDealId,
  activeSection,
  deals,
  email,
  navigationState,
  onDealRoomSectionChange,
}: DealRoomSidebarProps) {
  const activeDeal = deals.find((deal) => deal.room.id === activeDealId) ?? deals[0];

  return (
    <SidebarFrame
      alignedHeader
      centeredLogo
      email={email}
      headerBackLabel="Back to main homepage"
      headerBackTo="/hub"
      navigationState={navigationState}
      profileDeal={activeDeal}
    >
      <nav className="space-y-1">
        {dealRoomSidebarLinks.map((link) => {
          if (activeDeal && "key" in link && (link.key === "deal-room" || link.key === "data-room")) {
            const destination = link.key === "deal-room" ? getDealRoomPath(activeDeal.room.id) : getDataRoomPath(activeDeal.room.id);

            return (
              <NavLink
                aria-label={link.label}
                className={() =>
                  [
                    "flex items-center gap-3 rounded-lg px-3 py-2 transition",
                    activeSection === link.key
                      ? "bg-sidebar-selected text-sidebar-active"
                      : "text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active",
                  ].join(" ")
                }
                key={link.label}
                onClick={() => {
                  if (link.key === "deal-room") {
                    onDealRoomSectionChange?.("deal-room");
                  }
                }}
                state={navigationState}
                to={destination}
              >
                <Icon className="h-5 w-5" name={link.icon} />
                <span className="text-[13px] font-medium leading-5">{link.label}</span>
              </NavLink>
            );
          }

          if (
            "key" in link &&
            (link.key === "timeline" || link.key === "site-visits" || link.key === "deliverables")
          ) {
            return (
              <SidebarStaticItem
                active={activeSection === link.key}
                icon={link.icon}
                key={link.label}
                label={link.label}
                onClick={() => onDealRoomSectionChange?.(link.key as DealRoomTabSection)}
              />
            );
          }

          return <SidebarStaticItem icon={link.icon} key={link.label} label={link.label} />;
        })}
      </nav>
    </SidebarFrame>
  );
}
