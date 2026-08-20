import { ArrowEndOnRectangleIcon } from "../../ui/icons/ArrowEndOnRectangleIcon";
import { ViewColumnsIcon } from "../../ui/icons/ViewColumnsIcon";
import { Icon } from "../../ui/Icon";

type ChipBankHeaderProps = {
  onCollapse: () => void;
};

export function ChipBankHeader({ onCollapse }: ChipBankHeaderProps) {
  return (
    <div className="flex w-full items-center justify-between">
      <div className="flex min-w-0 items-center gap-3">
        <Icon className="h-5 w-5 shrink-0 text-sidebar-muted" name="search" />
        <h3 className="truncate text-[13px] font-semibold leading-5 text-sidebar-active">Document Search</h3>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <button
          aria-label="Document search view options"
          className="group flex h-[1.6rem] w-[1.6rem] items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          title="Document search view options"
          type="button"
        >
          <ViewColumnsIcon className="h-4 w-4" />
        </button>
        <button
          aria-label="Collapse document search"
          className="flex h-[1.6rem] w-[1.6rem] items-center justify-center rounded-md text-sidebar-muted transition hover:bg-sidebar-hover hover:text-sidebar-active focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          onClick={onCollapse}
          title="Collapse document search"
          type="button"
        >
          <ArrowEndOnRectangleIcon className="h-4 w-4" direction="right" />
        </button>
      </div>
    </div>
  );
}
