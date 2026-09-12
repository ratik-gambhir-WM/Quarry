import { NavLink } from "react-router-dom";
import { getDataRoomPath, getDealRoomPath, getDeliverablesPath } from "../../../data/workspace";
import { Icon } from "../../ui/Icon";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarSection } from "./SidebarSection";
import { SidebarStaticItem } from "./SidebarStaticItem";
import type { DealRoomSidebarProps } from "./sidebarTypes";

const primaryDealRoomLinks = [
  { icon: "dashboard" as const, key: "deal-room" as const, label: "Deal Room" },
  { icon: "calendarDays" as const, key: "timeline" as const, label: "Deal Activity" },
];

const dealArtifactLinks = [
  { icon: "folderTree" as const, key: "data-room" as const, label: "Data Room" },
  { icon: "fileStack" as const, key: "site-visits" as const, label: "Analysis" },
  { icon: "ship" as const, key: "deliverables" as const, label: "Deliverable" },
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
  const renderLink = (link: (typeof primaryDealRoomLinks | typeof dealArtifactLinks)[number]) => {
    if (
      activeDeal
      && (link.key === "deal-room" || link.key === "data-room" || link.key === "deliverables")
    ) {
      const destination = link.key === "deal-room"
        ? getDealRoomPath(activeDeal.room.id)
        : link.key === "data-room"
          ? getDataRoomPath(activeDeal.room.id)
          : getDeliverablesPath(activeDeal.room.id);

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

    if (link.key === "timeline" || link.key === "site-visits") {
      return (
        <SidebarStaticItem
          active={activeSection === link.key}
          icon={link.icon}
          key={link.label}
          label={link.label}
          onClick={() => onDealRoomSectionChange?.(link.key)}
        />
      );
    }

    return <SidebarStaticItem icon={link.icon} key={link.label} label={link.label} />;
  };

  return (
    <SidebarFrame
      alignedHeader
      email={email}
      headerBackLabel="Back to deals"
      headerBackTo="/hub/deals"
      navigationState={navigationState}
      profileDeal={activeDeal}
      sidebarIcon="dashboard"
      sidebarLabel="Deal Room"
    >
      <nav className="space-y-1">
        {primaryDealRoomLinks.map(renderLink)}
      </nav>

      <SidebarSection title="Deal Artifacts">
        {dealArtifactLinks.map(renderLink)}
      </SidebarSection>
    </SidebarFrame>
  );
}
