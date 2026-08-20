import { RefObject } from "react";
import { Icon } from "../../ui/Icon";

type DealSectionMenuProps = {
  containerRef: RefObject<HTMLDivElement | null>;
  menuOpen: boolean;
  onAddDeal: () => void;
  onToggleMenu: () => void;
};

export function DealSectionMenu({ containerRef, menuOpen, onAddDeal, onToggleMenu }: DealSectionMenuProps) {
  return (
    <div className="relative -mr-1" ref={containerRef}>
      <button
        aria-expanded={menuOpen}
        aria-haspopup="menu"
        aria-label="Active deals actions"
        className="flex h-7 w-7 items-center justify-center rounded-full text-muted transition hover:bg-white/60 hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={onToggleMenu}
        type="button"
      >
        <Icon className="h-4 w-4" name="plus" />
      </button>

      {menuOpen ? (
        <div
          className="absolute right-0 top-full z-20 mt-2 w-36 rounded-2xl border border-outline-variant bg-white p-1.5 shadow-[0_18px_44px_rgba(7,1,84,0.12)]"
          role="menu"
        >
          <button
            className="flex w-full items-center gap-2 rounded-xl px-3 py-2 text-left text-[12px] font-semibold text-text-main transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            onClick={onAddDeal}
            role="menuitem"
            type="button"
          >
            <Icon className="h-4 w-4 text-muted" name="plus" />
            <span>Add deal</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
