import { Icon } from "../ui/Icon";

export type DataRoomSidebarTabId = "data-room" | "diligence-graph" | "notebook" | "synthesis-canvas";

type DataRoomSidebarTabsProps = {
  activeTab: DataRoomSidebarTabId;
  compact?: boolean;
};

const sidebarTabs = [
  { icon: "folderOpen" as const, id: "data-room" as const, label: "Data Room" },
  { icon: "graph" as const, id: "diligence-graph" as const, label: "Diligence Graph" },
  { icon: "doc" as const, id: "notebook" as const, label: "Notebook" },
  { icon: "grid" as const, id: "synthesis-canvas" as const, label: "Synthesis Canvas" },
];

export function DataRoomSidebarTabs({
  activeTab,
  compact = false,
}: DataRoomSidebarTabsProps) {
  const visibleTabs = compact ? sidebarTabs.filter((tab) => tab.id === activeTab) : sidebarTabs;

  return (
    <div
      aria-label="Data room views"
      className={`flex bg-sidebar-hover ${compact ? "rounded-lg p-0" : "rounded-lg p-1"}`}
      role="tablist"
    >
      {visibleTabs.map((tab) => {
        const active = tab.id === activeTab;

        return (
          <button
            aria-label={tab.label}
            aria-selected={active}
            className={`flex h-9 min-w-0 items-center justify-center rounded-md transition ${
              compact ? "w-full" : "flex-1"
            } ${
              active
                ? "bg-background text-sidebar-active shadow-sm"
                : "cursor-not-allowed text-sidebar-muted opacity-50"
            }`}
            disabled={!active}
            key={tab.id}
            title={active ? tab.label : `${tab.label} (coming soon)`}
            role="tab"
            type="button"
          >
            <Icon className="h-5 w-5 shrink-0" name={tab.icon} />
          </button>
        );
      })}
    </div>
  );
}
