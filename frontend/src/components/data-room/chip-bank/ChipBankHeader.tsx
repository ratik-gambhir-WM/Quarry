import { Icon } from "../../ui/Icon";

type ChipBankHeaderProps = {
  onCollapse: () => void;
};

export function ChipBankHeader({ onCollapse }: ChipBankHeaderProps) {
  return (
    <div className="flex w-full items-center">
      <div className="flex min-w-0 items-center gap-3">
        <button
          aria-label="Collapse document search"
          className="flex h-[1.6rem] w-[1.6rem] shrink-0 items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          onClick={onCollapse}
          title="Collapse document search"
          type="button"
        >
          <Icon className="h-3.5 w-3.5 rotate-180" name="sidebar" />
        </button>
        <Icon className="h-5 w-5 shrink-0 text-sidebar-muted" name="search" />
        <h3 className="truncate text-[13px] font-semibold leading-5 text-sidebar-active">Document Search</h3>
      </div>
    </div>
  );
}
