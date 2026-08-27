import { Icon } from "../ui/Icon";
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip";

export type DataRoomSidebarTabId = "data-room" | "diligence-graph" | "synthesis-canvas" | "notes";

type DataRoomSidebarTabsProps = {
  activeTab: DataRoomSidebarTabId;
  compact?: boolean;
};

const sidebarTabs = [
  { icon: "dataset" as const, id: "data-room" as const, label: "Data Room" },
  { icon: "graph" as const, id: "diligence-graph" as const, label: "Diligence Graph" },
  { icon: "grid" as const, id: "synthesis-canvas" as const, label: "Synthesis Canvas" },
  { icon: "doc" as const, id: "notes" as const, label: "Notes" },
];

export function DataRoomSidebarTabs({
  activeTab,
  compact = false,
}: DataRoomSidebarTabsProps) {
  const visibleTabs = compact ? sidebarTabs.filter((tab) => tab.id === activeTab) : sidebarTabs;

  return (
    <TooltipProvider delayDuration={250}>
      <div
        aria-label="Data room views"
        className={`grid w-full gap-0.5 rounded-lg bg-sidebar-hover/65 p-0.5 ${
          compact ? "grid-cols-1" : "grid-cols-4"
        }`}
        role="tablist"
      >
        {visibleTabs.map((tab) => {
          const active = tab.id === activeTab;

          return (
            <Tooltip key={tab.id}>
              <TooltipTrigger asChild>
                <button
                  aria-disabled={!active}
                  aria-label={tab.label}
                  aria-selected={active}
                  className={`flex h-8 min-w-0 items-center justify-center rounded-md transition-[background-color,color,box-shadow] duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed ${
                    active
                      ? "bg-surface-container-lowest text-sidebar-active shadow-[0_1px_2px_rgba(0,0,0,0.06)] ring-1 ring-outline-variant/60"
                      : "cursor-not-allowed text-sidebar-muted opacity-45"
                  }`}
                  role="tab"
                  type="button"
                >
                  <Icon className="h-4 w-4" name={tab.icon} />
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom" sideOffset={8}>
                {active ? tab.label : `${tab.label} · Coming soon`}
              </TooltipContent>
            </Tooltip>
          );
        })}
      </div>
    </TooltipProvider>
  );
}
