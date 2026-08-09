import { useEffect, useRef, useState } from "react";
import { NavLink } from "react-router-dom";
import { getDealRoomPath } from "../../../data/workspace";
import { Icon } from "../../ui/Icon";
import { AddDealModal } from "./AddDealModal";
import { DealSectionMenu } from "./DealSectionMenu";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarLink } from "./SidebarLink";
import { SidebarSection } from "./SidebarSection";
import type { HomeSidebarProps } from "./sidebarTypes";

export function HomeWorkspaceSidebar({
  activeHomeSection,
  deals,
  email,
  initiatives,
  navigationState,
  tools,
}: HomeSidebarProps) {
  const [dealMenuOpen, setDealMenuOpen] = useState(false);
  const [addDealModalOpen, setAddDealModalOpen] = useState(false);
  const dealMenuRef = useRef<HTMLDivElement>(null);

  function handleAddDeal() {
    setDealMenuOpen(false);
    setAddDealModalOpen(true);
  }

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!dealMenuRef.current?.contains(event.target as Node)) {
        setDealMenuOpen(false);
      }
    }

    if (dealMenuOpen) {
      document.addEventListener("pointerdown", handlePointerDown);
    }

    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [dealMenuOpen]);

  return (
    <>
      <SidebarFrame alignedHeader email={email} navigationState={navigationState}>
        <nav className="space-y-1">
          <NavLink
            aria-label="Deal Hub"
            className={({ isActive }) =>
              [
                "flex items-center gap-3 rounded-lg px-3 py-2 transition",
                isActive && activeHomeSection === "hub"
                  ? "bg-sidebar-selected text-sidebar-active"
                  : "text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active",
              ].join(" ")
            }
            end
            state={navigationState}
            to="/hub"
          >
            <Icon className="h-5 w-5" name="home" />
            <span className="text-[13px] font-medium leading-5">Deal Hub</span>
          </NavLink>
        </nav>

        <SidebarSection
          action={
            <DealSectionMenu
              containerRef={dealMenuRef}
              menuOpen={dealMenuOpen}
              onAddDeal={handleAddDeal}
              onToggleMenu={() => setDealMenuOpen((isOpen) => !isOpen)}
            />
          }
          title="Active Deals"
        >
          {deals.map((deal) => (
            <NavLink
              aria-label={deal.room.name}
              className={({ isActive }) =>
                [
                  "flex items-center justify-between rounded-lg px-3 py-2 text-sidebar-text transition",
                  isActive
                    ? "bg-sidebar-selected text-sidebar-active"
                    : "hover:bg-sidebar-hover hover:text-sidebar-active",
                ].join(" ")
              }
              key={deal.room.id}
              state={navigationState}
              to={getDealRoomPath(deal.room.id)}
            >
              <div className="flex items-center gap-3">
                <span className={`h-2.5 w-2.5 rounded-full ${deal.colorClassName}`} />
                <span className="text-[13px] font-medium leading-5">{deal.room.name}</span>
              </div>
              {deal.complete ? <Icon className="h-4 w-4 text-sidebar-muted" name="checkCircle" /> : null}
            </NavLink>
          ))}
        </SidebarSection>

        <SidebarSection title="Internal Initiatives">
          {initiatives.map((item) => (
            <SidebarLink icon={item.icon} key={item.name} label={item.name} />
          ))}
        </SidebarSection>

        <SidebarSection title="Research">
          <SidebarLink icon="search" label="Topics" />
          <SidebarLink
            homeSection={activeHomeSection}
            href="/hub/summarize"
            icon="sparkles"
            label="Quick Chat"
            navigationState={navigationState}
          />
        </SidebarSection>

        <div className="mt-3 border-t border-outline-variant/70 pt-3">
          <nav className="space-y-1">
            {tools.map((item) => (
              <SidebarLink
                homeSection={activeHomeSection}
                href={item.href}
                icon={item.icon}
                key={item.name}
                label={item.name}
                navigationState={navigationState}
              />
            ))}
          </nav>
        </div>
      </SidebarFrame>

      {addDealModalOpen ? <AddDealModal onClose={() => setAddDealModalOpen(false)} /> : null}
    </>
  );
}
