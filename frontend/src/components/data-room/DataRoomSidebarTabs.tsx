import { Icon } from "../ui/Icon";

export type DataRoomSidebarTabId = "data-room" | "diligence-graph" | "notebook" | "synthesis-canvas";

type DataRoomSidebarTabsProps = {
  activeTab: DataRoomSidebarTabId;
  onTabChange?: (tab: DataRoomSidebarTabId) => void;
};

const sidebarTabs = [
  { icon: "folderOpen" as const, id: "data-room" as const, label: "Data Room" },
  { icon: "graph" as const, id: "diligence-graph" as const, label: "Diligence Graph" },
  { icon: "doc" as const, id: "notebook" as const, label: "Notebook" },
  { icon: "grid" as const, id: "synthesis-canvas" as const, label: "Synthesis Canvas" },
];

export function DataRoomSidebarTabs({ activeTab, onTabChange }: DataRoomSidebarTabsProps) {
  return (
    <nav aria-label="Data room views" className="grid grid-cols-2 gap-2">
      {sidebarTabs.map((tab) => {
        const active = tab.id === activeTab;
        const enabled = active || Boolean(onTabChange);

        return (
          <button
            aria-current={active ? "page" : undefined}
            className={`flex min-h-12 min-w-0 items-center gap-2 rounded-md border px-3 py-2 text-left transition ${
              active
                ? "border-primary/25 bg-primary/10 text-text-main shadow-sm"
                : "border-outline-variant bg-surface-container-lowest text-muted"
            } ${enabled ? "hover:border-primary/30 hover:text-text-main" : "cursor-default"}`}
            disabled={!enabled}
            key={tab.id}
            onClick={() => onTabChange?.(tab.id)}
            title={active ? tab.label : `${tab.label} (coming soon)`}
            type="button"
          >
            <Icon className="h-4 w-4 shrink-0" name={tab.icon} />
            <span className="min-w-0 text-[12px] font-semibold leading-4">{tab.label}</span>
          </button>
        );
      })}
    </nav>
  );
}
