export type DataRoomSidebarTabId = "data-room" | "diligence-graph" | "synthesis-canvas" | "notes";

type DataRoomSidebarTabsProps = {
  activeTab: DataRoomSidebarTabId;
  compact?: boolean;
};

const sidebarTabs = [
  { id: "data-room" as const, label: "Data Room" },
  { id: "diligence-graph" as const, label: "Diligence Graph" },
  { id: "synthesis-canvas" as const, label: "Synthesis Canvas" },
  { id: "notes" as const, label: "Notes" },
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
            <span className="whitespace-normal break-words px-0.5 text-center text-[10px] font-semibold leading-tight">
              {tab.label}
            </span>
          </button>
        );
      })}
    </div>
  );
}
