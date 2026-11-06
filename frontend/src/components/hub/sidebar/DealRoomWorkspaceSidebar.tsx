import { Link, NavLink } from "react-router-dom";
import {
  getDataRoomPath,
  getDealAnalysisPath,
  getDealRoomPath,
  getDeliverablesPath,
} from "../../../data/workspace";
import { ChevronDown } from "lucide-react";
import { isTerminalDealStatus } from "../../../data/dealModel";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuTrigger,
} from "../../ui/dropdown-menu";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarIcon } from "./SidebarIcon";
import { SidebarSection } from "./SidebarSection";
import { SidebarStaticItem } from "./SidebarStaticItem";
import type { DealRoomSidebarProps } from "./sidebarTypes";

const primaryDealRoomLinks = [
  { icon: "dashboard" as const, key: "deal-room" as const, label: "Deal Room" },
];

const dealArtifactLinks = [
  {
    icon: "folderTree" as const,
    key: "data-room" as const,
    label: "Data Room",
  },
  { icon: "fileStack" as const, key: "analysis" as const, label: "Analysis" },
  { icon: "ship" as const, key: "deliverables" as const, label: "Deliverable" },
];

export function DealRoomWorkspaceSidebar({
  activeDealId,
  activeSection,
  deals,
  email,
  navigationState,
}: DealRoomSidebarProps) {
  const activeDeal =
    deals.find((deal) => deal.room.id === activeDealId) ?? deals[0];
  const otherActiveDeals = deals.filter(
    (deal) =>
      deal.room.id !== activeDeal?.room.id &&
      !isTerminalDealStatus(deal.portfolio.status),
  );
  const renderLink = (
    link: (typeof primaryDealRoomLinks | typeof dealArtifactLinks)[number],
  ) => {
    if (activeDeal) {
      const destination =
        link.key === "deal-room"
          ? getDealRoomPath(activeDeal.room.id)
          : link.key === "data-room"
            ? getDataRoomPath(activeDeal.room.id)
            : link.key === "analysis"
              ? getDealAnalysisPath(activeDeal.room.id)
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
          end={link.key !== "deliverables"}
          key={link.label}
          state={navigationState}
          to={destination}
        >
          <SidebarIcon className="h-5 w-5" name={link.icon} />
          <span className="text-[13px] font-medium leading-5">
            {link.label}
          </span>
        </NavLink>
      );
    }

    return (
      <SidebarStaticItem icon={link.icon} key={link.label} label={link.label} />
    );
  };

  return (
    <SidebarFrame
      alignedHeader
      email={email}
      headerBackLabel="Back to deals"
      headerBackTo="/hub/deals"
      headerIdentity={
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              aria-label="Switch deal room"
              className="flex min-w-0 flex-1 items-center gap-2 rounded-xl bg-sidebar-selected px-2 py-1.5 text-left text-sidebar-active outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed [@media(hover:hover)]:hover:bg-sidebar-hover active:brightness-95"
              title={activeDeal?.room.name}
              type="button"
            >
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-primary-container text-on-primary-container shadow-[0_4px_12px_rgba(7,1,84,0.12)]">
                <SidebarIcon inheritColor className="h-4 w-4" name="dashboard" />
              </span>
              <span className="min-w-0 flex-1 truncate text-[13px] font-semibold leading-5">
                Deal Room
              </span>
              <ChevronDown
                aria-hidden="true"
                className="size-3.5 shrink-0 text-sidebar-muted"
              />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            className="min-w-64 max-w-80 rounded-xl p-2 motion-reduce:animate-none [font-family:var(--font-sidebar)]"
          >
            <DropdownMenuLabel className="text-xs text-muted-foreground">
              Other active deals
            </DropdownMenuLabel>
            {otherActiveDeals.map((deal) => (
              <DropdownMenuItem asChild key={deal.room.id}>
                <Link
                  className="min-h-9 whitespace-normal break-words text-[13px]"
                  state={navigationState}
                  to={getDealRoomPath(deal.room.id)}
                >
                  {deal.room.name}
                </Link>
              </DropdownMenuItem>
            ))}
            {otherActiveDeals.length === 0 ? (
              <p className="px-2 py-3 text-xs text-muted-foreground">
                No other active deals.
              </p>
            ) : null}
          </DropdownMenuContent>
        </DropdownMenu>
      }
      navigationState={navigationState}
      profileDeal={activeDeal}
      sidebarIcon="dashboard"
      sidebarLabel="Deal Room"
    >
      <nav className="space-y-1">{primaryDealRoomLinks.map(renderLink)}</nav>

      <SidebarSection title="Deal Artifacts">
        {dealArtifactLinks.map(renderLink)}
      </SidebarSection>
    </SidebarFrame>
  );
}
