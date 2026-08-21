import { Icon } from "../../ui/Icon";

type SidebarStaticItemProps = {
  active?: boolean;
  icon: "dashboard" | "folderOpen" | "graph" | "grid" | "listAlt" | "person" | "timeline";
  label: string;
  onClick?: () => void;
};

export function SidebarStaticItem({ active = false, icon, label, onClick }: SidebarStaticItemProps) {
  return (
    <button
      aria-label={label}
      className={[
        "flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition",
        active
          ? "bg-sidebar-selected text-sidebar-active"
          : "text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active",
      ].join(" ")}
      onClick={onClick}
      type="button"
    >
      <Icon className={`h-5 w-5 ${active ? "text-current" : "text-sidebar-muted"}`} name={icon} />
      <span className="text-[13px] font-medium leading-5">{label}</span>
    </button>
  );
}
