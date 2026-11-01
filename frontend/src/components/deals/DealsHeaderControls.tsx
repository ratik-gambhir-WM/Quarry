import { Input } from "../ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "../ui/popover";
import { useRef, useState } from "react";
import { Columns3Icon, ListIcon, XIcon } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import {
  AdjustmentsHorizontalIcon,
  type AdjustmentsHorizontalIconHandle,
} from "../ui/adjustments-horizontal";
import { Icon } from "../ui/Icon";
import { DealsToolbarButton } from "./DealsToolbarButton";

export type DealsView = "kanban" | "table";

type DealsSearchProps = {
  onQueryChange: (query: string) => void;
  query: string;
};

export function DealsSearch({ onQueryChange, query }: DealsSearchProps) {
  const [open, setOpen] = useState(false);

  function changeOpen(nextOpen: boolean) {
    setOpen(nextOpen);
    if (!nextOpen) onQueryChange("");
  }

  return (
    <Popover open={open} onOpenChange={changeOpen}>
      <PopoverTrigger asChild>
        <DealsToolbarButton aria-label="Search deals" title="Search deals">
          <Icon className="size-4" name="search" />
        </DealsToolbarButton>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        sideOffset={8}
        aria-label="Search portfolio"
        className="w-[min(20rem,calc(100vw-3rem))] rounded-xl p-2 data-open:animate-none data-closed:animate-none"
      >
        <div className="flex items-center gap-1">
          <Input
            aria-label="Search deals"
            placeholder="Name, company, sponsor…"
            type="search"
            value={query}
            onChange={(event) => onQueryChange(event.currentTarget.value)}
            className="h-10 text-[16px] sm:text-[14px]"
          />
          <button
            type="button"
            aria-label="Close deal search"
            title="Close deal search (Esc)"
            onClick={() => changeOpen(false)}
            className="grid size-10 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-surface-container-high"
          >
            <XIcon aria-hidden="true" className="size-4" />
          </button>
        </div>
        <p className="px-1 text-[12px] text-muted">Filter your portfolio · Esc to close</p>
      </PopoverContent>
    </Popover>
  );
}

type DealsViewMenuProps = {
  onPreloadKanban: () => void;
  onViewChange: (view: DealsView) => void;
  view: DealsView;
};

export function DealsViewMenu({
  onPreloadKanban,
  onViewChange,
  view,
}: DealsViewMenuProps) {
  const iconRef = useRef<AdjustmentsHorizontalIconHandle>(null);

  return (
    <DropdownMenu
      onOpenChange={(open) => {
        if (open) onPreloadKanban();
      }}
    >
      <DropdownMenuTrigger asChild>
        <DealsToolbarButton
          aria-label="Change deals view"
          onMouseEnter={() => iconRef.current?.startAnimation()}
          onMouseLeave={() => iconRef.current?.stopAnimation()}
        >
          <AdjustmentsHorizontalIcon
            className="h-4 w-4"
            ref={iconRef}
            size={16}
          />
        </DealsToolbarButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        className="w-72 rounded-2xl border border-outline-variant bg-surface-container-lowest p-4 text-text-main shadow-lg"
        sideOffset={12}
      >
        <DropdownMenuRadioGroup
          className="grid grid-cols-2 gap-2"
          onValueChange={(value) => {
            if (value === "table" || value === "kanban") onViewChange(value);
          }}
          value={view}
        >
          <DropdownMenuRadioItem
            className="justify-center rounded-full border border-outline-variant px-4 py-2.5 text-[14px] font-medium data-[state=checked]:bg-surface-container-high [&_[data-slot=dropdown-menu-radio-item-indicator]]:hidden"
            value="table"
          >
            <ListIcon aria-hidden="true" className="size-4" />
            List
          </DropdownMenuRadioItem>
          <DropdownMenuRadioItem
            className="justify-center rounded-full border border-outline-variant px-4 py-2.5 text-[14px] font-medium data-[state=checked]:bg-surface-container-high [&_[data-slot=dropdown-menu-radio-item-indicator]]:hidden"
            value="kanban"
          >
            <Columns3Icon aria-hidden="true" className="size-4" />
            Kanban
          </DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
