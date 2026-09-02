import { useEffect, useRef, useState } from "react";
import {
  suggestedContentByTab,
  type SuggestedContentTab,
} from "../../../fixtures/hub/suggestedContent";
import { Icon } from "../../ui/Icon";
import { useWorkspaceHomeDeals } from "../WorkspaceHomeShell";

const tabs: SuggestedContentTab[] = ["Recent", "Analyzed", "Files to review"];

export function SuggestedContentCard() {
  const [activeTab, setActiveTab] = useState<SuggestedContentTab>("Recent");
  const groups = suggestedContentByTab[activeTab];

  return (
    <section className="relative px-1 pb-4 pt-1 sm:px-3 sm:pb-5">
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div aria-label="Content views" className="flex items-center gap-1" role="tablist">
          {tabs.map((tab) => (
            <button
              aria-selected={activeTab === tab}
              className={
                "rounded-lg px-3 py-2 text-[14px] font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed " +
                (activeTab === tab
                  ? "bg-surface-container-low text-text-main"
                  : "text-muted hover:bg-surface-container-low hover:text-text-main")
              }
              key={tab}
              onClick={() => setActiveTab(tab)}
              role="tab"
              type="button"
            >
              {tab}
            </button>
          ))}
        </div>

        <div className="flex items-center gap-2 self-end sm:self-auto">
          <DealFilterMenu />
          <button
            aria-label="About suggested content"
            className="flex h-9 w-9 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-low hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            type="button"
          >
            <Icon className="h-4 w-4" name="info" />
          </button>
        </div>
      </div>

      <div className="mt-2" role="tabpanel">
        {groups.map((group) => (
          <div className="mt-4" key={group.label}>
            <h3 className="text-[15px] font-semibold text-text-main">{group.label}</h3>
            <div className="mt-1.5 flex flex-col">
              {group.items.map((item) => (
                <button
                  className="group flex min-h-11 w-full items-center gap-3 rounded-xl px-1.5 py-2 text-left text-[14px] font-medium text-on-surface transition hover:bg-surface-container-low focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
                  key={item.label}
                  type="button"
                >
                  <Icon
                    className={
                      "h-4 w-4 shrink-0 " +
                      (item.tone === "error"
                        ? "text-error"
                        : item.tone === "primary"
                          ? "text-primary"
                          : "text-text-main")
                    }
                    name={item.icon}
                  />
                  <span className="min-w-0 truncate group-hover:text-text-main">{item.label}</span>
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function DealFilterMenu() {
  const deals = useWorkspaceHomeDeals();
  const [open, setOpen] = useState(false);
  const [selectedDealId, setSelectedDealId] = useState("all");
  const menuRef = useRef<HTMLDivElement | null>(null);
  const selectedDeal = deals.find((deal) => deal.room.id === selectedDealId);
  const selectedLabel = selectedDeal?.room.name ?? "All deals";

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!menuRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }

    if (open) {
      document.addEventListener("pointerdown", handlePointerDown);
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  return (
    <div className="relative" ref={menuRef}>
      <button
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-label={`Filter content by deal: ${selectedLabel}`}
        className="flex h-10 items-center gap-2 rounded-xl border border-outline-variant bg-surface-container-lowest px-3.5 text-[13px] font-semibold text-text-main transition hover:bg-surface-container-low focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
        onClick={() => setOpen((isOpen) => !isOpen)}
        type="button"
      >
        <Icon className="h-4 w-4" name="check" />
        <span>{selectedLabel}</span>
        <Icon className={`h-3.5 w-3.5 transition ${open ? "rotate-180" : ""}`} name="chevronDown" />
      </button>

      {open ? (
        <div
          aria-label="Filter content by deal"
          className="absolute right-0 top-[calc(100%+0.5rem)] z-30 min-w-52 overflow-hidden rounded-2xl border border-outline-variant bg-surface-container-lowest p-1.5 shadow-[0_18px_44px_rgba(7,1,84,0.14)]"
          role="listbox"
        >
          <DealFilterOption
            label="All deals"
            onSelect={() => {
              setSelectedDealId("all");
              setOpen(false);
            }}
            selected={selectedDealId === "all"}
          />
          {deals.map((deal) => (
            <DealFilterOption
              key={deal.room.id}
              label={deal.room.name}
              onSelect={() => {
                setSelectedDealId(deal.room.id);
                setOpen(false);
              }}
              selected={selectedDealId === deal.room.id}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

type DealFilterOptionProps = {
  label: string;
  onSelect: () => void;
  selected: boolean;
};

function DealFilterOption({ label, onSelect, selected }: DealFilterOptionProps) {
  return (
    <button
      aria-selected={selected}
      className={`flex w-full items-center justify-between gap-4 rounded-xl px-3 py-2.5 text-left text-[13px] font-medium transition ${
        selected ? "bg-primary/10 text-text-main" : "text-text-main/82 hover:bg-surface-container-high"
      }`}
      onClick={onSelect}
      role="option"
      type="button"
    >
      <span className="whitespace-nowrap">{label}</span>
      {selected ? <Icon className="h-4 w-4 shrink-0 text-primary" name="check" /> : null}
    </button>
  );
}
