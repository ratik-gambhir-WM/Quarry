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
  const inputRef = useRef<HTMLInputElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  function openSearch() {
    setOpen(true);
    window.requestAnimationFrame(() => inputRef.current?.focus());
  }

  function closeSearch() {
    onQueryChange("");
    setOpen(false);
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  }

  return (
    <div
      className={`relative flex h-8 items-center overflow-hidden rounded-full transition-[width,background-color,border-color] duration-300 ease-out motion-reduce:transition-none ${
        open
          ? "w-[min(19rem,calc(100vw-15rem))] border border-outline-variant bg-surface-container-lowest"
          : "w-8 border border-transparent bg-transparent"
      }`}
    >
      <button
        aria-expanded={open}
        aria-label="Search deals"
        className="absolute left-0 z-10 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={openSearch}
        ref={triggerRef}
        type="button"
      >
        <Icon className="h-4 w-4" name="search" />
      </button>
      <input
        aria-hidden={!open}
        aria-label="Search deals"
        className={`h-full min-w-0 flex-1 appearance-none bg-transparent pl-8 pr-8 text-[12px] text-text-main outline-none transition-opacity duration-200 placeholder:text-muted motion-reduce:transition-none [&::-webkit-search-cancel-button]:hidden ${
          open ? "opacity-100" : "pointer-events-none opacity-0"
        }`}
        onChange={(event) => onQueryChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key !== "Escape") return;
          event.preventDefault();
          closeSearch();
        }}
        placeholder="Search deals"
        ref={inputRef}
        tabIndex={open ? 0 : -1}
        type="search"
        value={query}
      />
      {open ? (
        <button
          aria-label="Close deal search"
          className="absolute right-0 z-10 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          onClick={closeSearch}
          type="button"
        >
          <XIcon aria-hidden="true" className="h-4 w-4" />
        </button>
      ) : null}
    </div>
  );
}

type DealsViewMenuProps = {
  onPreloadKanban: () => void;
  onViewChange: (view: DealsView) => void;
  view: DealsView;
};

export function DealsViewMenu({ onPreloadKanban, onViewChange, view }: DealsViewMenuProps) {
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
          onBlur={() => iconRef.current?.stopAnimation()}
          onFocus={() => iconRef.current?.startAnimation()}
          onMouseEnter={() => iconRef.current?.startAnimation()}
          onMouseLeave={() => iconRef.current?.stopAnimation()}
        >
          <AdjustmentsHorizontalIcon className="h-4 w-4" ref={iconRef} size={16} />
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
