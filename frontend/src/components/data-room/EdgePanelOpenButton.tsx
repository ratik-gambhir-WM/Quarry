import { Icon } from "../ui/Icon";

type EdgePanelOpenButtonProps = {
  label: string;
  onClick: () => void;
};

export function EdgePanelOpenButton({ label, onClick }: EdgePanelOpenButtonProps) {
  return (
    <button
      aria-label={label}
      className="flex h-[1.6rem] w-[1.6rem] shrink-0 items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
      onClick={onClick}
      title={label}
      type="button"
    >
      <Icon className="h-3.5 w-3.5" name="sidebar" />
    </button>
  );
}
