import { ThreadListPrimitive } from "@assistant-ui/react";
import { useState } from "react";
import type { KeyboardEvent } from "react";
import type { HomeSidebarProps } from "./sidebarTypes";
import { Icon } from "../../ui/Icon";
import { HomeWorkspaceSidebarNavigation } from "./HomeWorkspaceSidebar";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarIcon } from "./SidebarIcon";

type AssistantWorkspaceSidebarProps = Pick<
  HomeSidebarProps,
  "activeHomeSection" | "email" | "navigationState" | "tools"
>;

type AssistantSidebarView = "chat" | "deal-hub";

export function AssistantWorkspaceSidebar({
  activeHomeSection,
  email,
  navigationState,
  tools,
}: AssistantWorkspaceSidebarProps) {
  const [activeView, setActiveView] = useState<AssistantSidebarView>("chat");

  const handleTabKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
      return;
    }

    const tabs = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]'));
    const currentTab = (event.target as HTMLElement).closest<HTMLButtonElement>('[role="tab"]');
    const currentIndex = currentTab ? tabs.indexOf(currentTab) : -1;
    if (currentIndex < 0) {
      return;
    }

    event.preventDefault();
    const nextIndex = event.key === "Home"
      ? 0
      : event.key === "End"
        ? tabs.length - 1
        : (currentIndex + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
    const nextTab = tabs[nextIndex];
    const nextView = nextTab.dataset.view;
    if (nextView === "chat" || nextView === "deal-hub") {
      setActiveView(nextView);
      nextTab.focus();
    }
  };

  return (
    <SidebarFrame
      alignedHeader
      email={email}
      headerContent={(
        <div
          aria-label="Assistant sidebar views"
          className="flex min-w-0 items-center gap-1"
          onKeyDown={handleTabKeyDown}
          role="tablist"
        >
          <AssistantTab
            activeView={activeView}
            icon="home"
            label="Deal Hub"
            onSelect={setActiveView}
            view="deal-hub"
          />
          <AssistantTab
            activeView={activeView}
            icon="sparkles"
            label="Chat"
            onSelect={setActiveView}
            view="chat"
          />
        </div>
      )}
      navigationState={navigationState}
      showHeaderBackButton={false}
    >
      {({ collapsed }) => (
        <div
          aria-labelledby={`assistant-${activeView}-tab`}
          id="assistant-sidebar-panel"
          role="tabpanel"
        >
          {activeView === "deal-hub" ? (
            <HomeWorkspaceSidebarNavigation
              activeHomeSection={activeHomeSection}
              navigationState={navigationState}
              tools={tools}
            />
          ) : (
            <ChatSidebarContent collapsed={collapsed} />
          )}
        </div>
      )}
    </SidebarFrame>
  );
}

function AssistantTab({
  activeView,
  icon,
  label,
  onSelect,
  view,
}: {
  activeView: AssistantSidebarView;
  icon: "home" | "sparkles";
  label: string;
  onSelect: (view: AssistantSidebarView) => void;
  view: AssistantSidebarView;
}) {
  const selected = activeView === view;

  return (
    <button
      aria-controls="assistant-sidebar-panel"
      aria-selected={selected}
      className={`flex min-w-0 items-center gap-2 rounded-xl px-2.5 py-2 text-[13px] font-medium leading-4 transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
        selected
          ? "bg-sidebar-selected text-sidebar-active"
          : "text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-active"
      }`}
      id={`assistant-${view}-tab`}
      data-view={view}
      onClick={() => onSelect(view)}
      role="tab"
      tabIndex={selected ? 0 : -1}
      type="button"
    >
      <SidebarIcon className="h-4 w-4 shrink-0" name={icon} />
      <span className="truncate">{label}</span>
    </button>
  );
}

function ChatSidebarContent({ collapsed }: { collapsed: boolean }) {
  return (
    <div className="space-y-3">
      <ThreadListPrimitive.New
        aria-label="New chat"
        className={`group flex w-full items-center rounded-xl py-2.5 text-left text-[14px] font-medium text-sidebar-active transition hover:bg-sidebar-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
          collapsed ? "justify-center px-0" : "gap-3 px-3"
        }`}
      >
        <Icon className="h-5 w-5 shrink-0 text-sidebar-muted" name="editSquare" />
        {collapsed ? null : (
          <>
            <span className="min-w-0 flex-1 truncate">New chat</span>
            <Icon
              className="h-5 w-5 shrink-0 text-sidebar-muted transition group-hover:text-sidebar-active"
              name="plusCircle"
            />
          </>
        )}
      </ThreadListPrimitive.New>
      <ul aria-label="Previous chats" className="space-y-1" />
    </div>
  );
}
