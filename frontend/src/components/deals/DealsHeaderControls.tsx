import { useRef, useState } from "react";
import type { DealScope } from "../../data/dealsView";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import {
  AdjustmentsHorizontalIcon,
  type AdjustmentsHorizontalIconHandle,
} from "../ui/adjustments-horizontal";
import {
  MagnifyingGlassIcon,
  type MagnifyingGlassIconHandle,
} from "../ui/magnifying-glass";
import { TableCellsIcon, type TableCellsIconHandle } from "../ui/table-cells";
import { ViewColumnsIcon, type ViewColumnsIconHandle } from "../ui/view-columns";
import { DealsToolbarButton } from "./DealsToolbarButton";

export type DealsView = "kanban" | "table";

type DealsSearchProps = {
  onQueryChange: (query: string) => void;
  query: string;
};

export function DealsSearch({ onQueryChange, query }: DealsSearchProps) {
  const [open, setOpen] = useState(false);
  const iconRef = useRef<MagnifyingGlassIconHandle>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  function openSearch() {
    setOpen(true);
    iconRef.current?.startAnimation();
    window.requestAnimationFrame(() => inputRef.current?.focus());
  }

  function closeSearch() {
    setOpen(false);
    iconRef.current?.stopAnimation();
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
        onFocus={() => iconRef.current?.startAnimation()}
        onMouseEnter={() => iconRef.current?.startAnimation()}
        onMouseLeave={() => {
          if (!open) iconRef.current?.stopAnimation();
        }}
        type="button"
      >
        <MagnifyingGlassIcon className="h-4 w-4" ref={iconRef} size={16} />
      </button>
      <input
        aria-hidden={!open}
        aria-label="Search deals"
        className={`h-full min-w-0 flex-1 bg-transparent pl-8 pr-3 text-[12px] text-text-main outline-none transition-opacity duration-200 placeholder:text-muted motion-reduce:transition-none ${
          open ? "opacity-100" : "pointer-events-none opacity-0"
        }`}
        onChange={(event) => onQueryChange(event.currentTarget.value)}
        onKeyDown={(event) => {
          if (event.key !== "Escape") return;
          event.preventDefault();
          onQueryChange("");
          closeSearch();
        }}
        placeholder="Search deals"
        ref={inputRef}
        tabIndex={open ? 0 : -1}
        type="search"
        value={query}
      />
    </div>
  );
}

type DealLifecycleFilterProps = {
  onScopeChange: (scope: DealScope) => void;
  scope: DealScope;
};

const scopeOptions: Array<{ label: string; value: DealScope }> = [
  { label: "All deals", value: "all" },
  { label: "Current", value: "current" },
  { label: "Historic", value: "historic" },
];

export function DealLifecycleFilter({ onScopeChange, scope }: DealLifecycleFilterProps) {
  const iconRef = useRef<AdjustmentsHorizontalIconHandle>(null);

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <DealsToolbarButton
          aria-label="Filter deals by lifecycle"
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
        className="w-44 rounded-xl border border-outline-variant bg-surface-container-lowest p-1.5 text-text-main"
        sideOffset={8}
      >
        <DropdownMenuLabel className="px-2 py-1.5 text-[11px] font-medium uppercase tracking-[0.08em] text-muted">
          Deal lifecycle
        </DropdownMenuLabel>
        <DropdownMenuRadioGroup
          onValueChange={(value) => onScopeChange(value as DealScope)}
          value={scope}
        >
          {scopeOptions.map((option) => (
            <DropdownMenuRadioItem
              className="rounded-lg px-2.5 py-2 text-[13px] font-normal"
              key={option.value}
              value={option.value}
            >
              {option.label}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

type DealsViewToggleProps = {
  onPreloadKanban: () => void;
  onViewChange: (view: DealsView) => void;
  view: DealsView;
};

export function DealsViewToggle({ onPreloadKanban, onViewChange, view }: DealsViewToggleProps) {
  const tableIconRef = useRef<TableCellsIconHandle>(null);
  const kanbanIconRef = useRef<ViewColumnsIconHandle>(null);

  return (
    <div aria-label="Deals view" className="flex items-center gap-1" role="group">
      <DealsToolbarButton
        active={view === "table"}
        aria-label="Table view"
        aria-pressed={view === "table"}
        onClick={() => onViewChange("table")}
        onMouseEnter={() => tableIconRef.current?.startAnimation()}
        onMouseLeave={() => tableIconRef.current?.stopAnimation()}
        title="Table view"
      >
        <TableCellsIcon className="h-4 w-4" ref={tableIconRef} size={16} />
      </DealsToolbarButton>
      <DealsToolbarButton
        active={view === "kanban"}
        aria-label="Kanban view"
        aria-pressed={view === "kanban"}
        onClick={() => onViewChange("kanban")}
        onFocus={onPreloadKanban}
        onMouseEnter={() => {
          onPreloadKanban();
          kanbanIconRef.current?.startAnimation();
        }}
        onMouseLeave={() => kanbanIconRef.current?.stopAnimation()}
        title="Kanban view"
      >
        <ViewColumnsIcon className="h-4 w-4" ref={kanbanIconRef} size={16} />
      </DealsToolbarButton>
    </div>
  );
}
