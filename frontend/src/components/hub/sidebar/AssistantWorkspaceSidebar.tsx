import { useState } from "react";
import type { KeyboardEvent } from "react";
import { Archive, MoreHorizontal, Trash2 } from "lucide-react";
import type { HomeSidebarProps } from "./sidebarTypes";
import { Icon } from "../../ui/Icon";
import { buttonVariants } from "../../ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../../ui/dropdown-menu";
import { HomeWorkspaceSidebarNavigation } from "./HomeWorkspaceSidebar";
import { SidebarFrame } from "./SidebarFrame";
import { SidebarIcon } from "./SidebarIcon";
import { useAgentRuntimeContext, useThreadRuntimeState } from "../../chat/AgentRuntime";

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
  const runtime = useAgentRuntimeContext();
  const {
    isPreparingThread,
    isRunning,
    isThreadListLoading,
    selectedThreadId,
    deletingThreadId,
    threadDeleteError,
    threadListError,
    threads,
  } = useThreadRuntimeState((state) => state);
  return (
    <div className="space-y-3">
      <button
        aria-label="New chat"
        className={`group flex w-full items-center rounded-xl py-2.5 text-left text-[14px] font-medium text-sidebar-active transition hover:bg-sidebar-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
          collapsed ? "justify-center px-0" : "gap-3 px-3"
        }`}
        onClick={() => runtime.newChat()}
        disabled={isPreparingThread}
        type="button"
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
      </button>
      {collapsed ? null : (
        <ul aria-label="Previous chats" className="space-y-1">
          {threads.map((thread) => (
            <AssistantThreadListItem
              active={selectedThreadId === thread.threadId}
              deleting={deletingThreadId === thread.threadId}
              key={thread.threadId}
              onDelete={() => void runtime.deleteThread(thread.threadId)}
              onSelect={() => void runtime.selectThread(thread.threadId)}
              actionsDisabled={isPreparingThread || isRunning || deletingThreadId !== null}
              title={thread.title}
            />
          ))}
          {isThreadListLoading ? (
            <li className="px-3 py-2 text-xs text-sidebar-muted" role="status">Loading chats…</li>
          ) : null}
          {threadListError ? (
            <li className="px-3 py-2 text-xs text-error" role="status">Couldn’t load previous chats.</li>
          ) : null}
          {threadDeleteError ? (
            <li className="px-3 py-2 text-xs text-error" role="status">Couldn’t delete chat. Please try again.</li>
          ) : null}
        </ul>
      )}
    </div>
  );
}

function AssistantThreadListItem({
  active,
  actionsDisabled,
  deleting,
  onDelete,
  onSelect,
  title,
}: {
  active: boolean;
  actionsDisabled: boolean;
  deleting: boolean;
  onDelete: () => void;
  onSelect: () => void;
  title: string;
}) {
  return (
    <li className="group flex items-center gap-1">
      <button
        aria-current={active ? "page" : undefined}
        className={`min-w-0 flex-1 truncate rounded-xl px-3 py-2 text-left text-[13px] text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
          active ? "bg-sidebar-selected text-sidebar-active" : ""
        }`}
        onClick={onSelect}
        type="button"
      >
        {title || "New chat"}
      </button>
      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label={`Chat options for ${title || "New chat"}`}
          className={`${buttonVariants({ variant: "ghost", size: "icon-xs" })} shrink-0 text-sidebar-muted hover:text-sidebar-active`}
          type="button"
        >
          <MoreHorizontal />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="w-auto">
          <DropdownMenuItem disabled>
            <Archive />
            Archive chat
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem disabled={actionsDisabled || deleting} onSelect={onDelete} variant="destructive">
            <Trash2 />
            {deleting ? "Deleting…" : "Delete chat"}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </li>
  );
}
