import { Icon } from "../ui/Icon";

type PanelTabProps = {
  active: boolean;
  icon: "send" | "sparkles";
  label: string;
  onClick: () => void;
};

export function PanelTab({ active, icon, label, onClick }: PanelTabProps) {
  return (
    <button
      aria-pressed={active}
      className={[
        "flex items-center gap-2 rounded-full px-5 py-2 text-[13px] font-semibold transition",
        active ? "bg-primary-container text-on-primary-container shadow-[0_8px_18px_rgba(7,1,84,0.18)]" : "text-primary hover:bg-primary/8",
      ].join(" ")}
      onClick={onClick}
      type="button"
    >
      <Icon className="h-4 w-4" name={icon} />
      {label}
    </button>
  );
}
