import { useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import type { SidebarSpaceIcon, SidebarSpaceId } from "../../../data/sidebar";
import { workspaceSidebarSpaces } from "../../../fixtures/sidebar/spaces";
import { Icon } from "../../ui/Icon";
import { SidebarSection } from "./SidebarSection";

export type { SidebarSpaceId } from "../../../data/sidebar";

type SidebarSwitcherProps = {
  activeSpaceId: SidebarSpaceId;
  currentIcon: SidebarSpaceIcon;
  currentLabel: string;
  onOpenChange?: (open: boolean) => void;
  onSpaceChange: (spaceId: SidebarSpaceId) => void;
};

export function SidebarSwitcher({
  activeSpaceId,
  currentIcon,
  currentLabel,
  onOpenChange,
  onSpaceChange,
}: SidebarSwitcherProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const menuId = useId();
  const openFocusIndexRef = useRef<number | null>(null);
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const options = [
    {
      description: "Current page navigation",
      icon: currentIcon,
      id: "current" as const,
      label: currentLabel,
    },
    ...workspaceSidebarSpaces,
  ];
  const activeSpace = options.find((space) => space.id === activeSpaceId) ?? options[0];

  function updateOpen(nextOpen: boolean) {
    if (!nextOpen) {
      openFocusIndexRef.current = null;
    }
    setOpen(nextOpen);
    onOpenChange?.(nextOpen);
  }

  function selectSpace(spaceId: SidebarSpaceId) {
    onSpaceChange(spaceId);
    updateOpen(false);
    window.requestAnimationFrame(() => triggerRef.current?.focus());
  }

  function handleMenuKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      updateOpen(false);
      triggerRef.current?.focus();
      return;
    }

    if (event.key === "Tab") {
      event.preventDefault();
      const focusTarget = getAdjacentFocusableElement(triggerRef.current, event.shiftKey ? -1 : 1);
      updateOpen(false);
      window.requestAnimationFrame(() => (focusTarget ?? triggerRef.current)?.focus());
      return;
    }

    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      return;
    }

    event.preventDefault();
    const items = itemRefs.current.filter((item): item is HTMLButtonElement => item !== null);
    const currentIndex = items.findIndex((item) => item === document.activeElement);
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? items.length - 1
          : event.key === "ArrowDown"
            ? (currentIndex + 1) % items.length
            : (currentIndex - 1 + items.length) % items.length;
    items[nextIndex]?.focus();
  }

  useEffect(() => {
    if (!open) {
      return;
    }

    const activeIndex =
      activeSpaceId === "current"
        ? 0
        : Math.max(1, workspaceSidebarSpaces.findIndex((space) => space.id === activeSpaceId) + 1);
    const focusIndex = openFocusIndexRef.current ?? activeIndex;
    openFocusIndexRef.current = null;
    const frame = window.requestAnimationFrame(() => itemRefs.current[focusIndex]?.focus());

    function handlePointerDown(event: PointerEvent) {
      if (event.target instanceof Node && !containerRef.current?.contains(event.target)) {
        setOpen(false);
        onOpenChange?.(false);
        if (!isFocusableTarget(event.target)) {
          window.requestAnimationFrame(() => triggerRef.current?.focus());
        }
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => {
      window.cancelAnimationFrame(frame);
      document.removeEventListener("pointerdown", handlePointerDown);
    };
  }, [activeSpaceId, onOpenChange, open]);

  return (
    <div className="contents" ref={containerRef}>
      <button
        aria-controls={menuId}
        aria-expanded={open}
        aria-haspopup="menu"
        aria-label={`Switch sidebar. Current sidebar: ${activeSpace.label}`}
        className="flex min-w-0 flex-1 items-center gap-2 rounded-xl bg-sidebar-selected px-2 py-1.5 text-left text-sidebar-active transition hover:bg-sidebar-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed data-[state=open]:bg-sidebar-hover"
        data-state={open ? "open" : "closed"}
        onClick={() => updateOpen(!open)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            openFocusIndexRef.current = event.key === "ArrowDown" ? 0 : options.length - 1;
            updateOpen(true);
          }
        }}
        ref={triggerRef}
        type="button"
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-primary-container text-on-primary-container shadow-[0_4px_12px_rgba(7,1,84,0.12)]">
          <Icon className="h-4 w-4" name={activeSpace.icon} />
        </span>
        <span className="min-w-0 flex-1 truncate text-[13px] font-semibold leading-5">
          {activeSpace.label}
        </span>
        <Icon
          className={`h-3.5 w-3.5 shrink-0 text-sidebar-muted transition-transform ${open ? "rotate-180" : ""}`}
          name="chevronDown"
        />
      </button>

      {open ? (
        <div
          className="absolute left-2 top-full z-50 mt-2 w-[272px] overflow-hidden rounded-[22px] border border-outline-variant bg-surface-container-lowest shadow-[0_14px_36px_rgba(7,1,84,0.12)]"
          onBlur={(event) => {
            if (event.relatedTarget instanceof Node && !containerRef.current?.contains(event.relatedTarget)) {
              updateOpen(false);
            }
          }}
        >
          <div
            aria-label="Switch sidebar"
            className="divide-y divide-outline-variant/60"
            id={menuId}
            onKeyDown={handleMenuKeyDown}
            role="menu"
          >
            {options.map((space) => (
              <button
                aria-checked={space.id === activeSpaceId}
                className={`relative flex w-full items-center px-4 py-1.5 text-left outline-none transition focus:bg-sidebar-hover ${
                  space.id === activeSpaceId ? "bg-sidebar-hover" : ""
                }`}
                key={space.id}
                onClick={() => {
                  if (isSidebarSpaceId(space.id)) {
                    selectSpace(space.id);
                  }
                }}
                onKeyDown={(event) => {
                  if (event.key !== "Enter" && event.key !== " ") {
                    return;
                  }

                  event.preventDefault();
                  if (isSidebarSpaceId(space.id)) {
                    selectSpace(space.id);
                  }
                }}
                ref={(element) => {
                  const index = options.findIndex((option) => option.id === space.id);
                  itemRefs.current[index] = element;
                }}
                role="menuitemradio"
                tabIndex={-1}
                type="button"
              >
                <span className="min-w-0">
                  <span className="block truncate text-[13px] font-semibold leading-5 text-sidebar-active">
                    {space.label}
                  </span>
                  <span className="block truncate text-[11px] leading-4 text-sidebar-muted">
                    {space.description}
                  </span>
                </span>
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

function isFocusableTarget(target: EventTarget | null) {
  if (!(target instanceof Element)) {
    return false;
  }

  const focusableElement = target.closest<HTMLElement>(FOCUSABLE_SELECTOR);
  return Boolean(focusableElement && focusableElement.tabIndex >= 0);
}

function getAdjacentFocusableElement(origin: HTMLElement | null, offset: -1 | 1) {
  if (!origin) {
    return null;
  }

  const focusableElements = Array.from(document.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
    (element) => element.tabIndex >= 0 && element.getClientRects().length > 0,
  );
  const originIndex = focusableElements.indexOf(origin);
  return originIndex < 0 ? null : focusableElements[originIndex + offset] ?? null;
}

const FOCUSABLE_SELECTOR =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [contenteditable="true"], [tabindex]:not([tabindex="-1"])';

export function MockSidebarNavigation({ spaceId }: { spaceId: Exclude<SidebarSpaceId, "current"> }) {
  const space = workspaceSidebarSpaces.find((candidate) => candidate.id === spaceId) ?? workspaceSidebarSpaces[0];
  const [activeItemId, setActiveItemId] = useState(space.sections[0].items[0].id);

  return (
    <div>
      <div className="px-3 pt-1">
        <span className="inline-flex rounded-full border border-outline-variant/70 bg-surface-container-lowest/70 px-2 py-1 text-[9px] font-semibold uppercase tracking-[0.12em] text-sidebar-muted">
          Mock sidebar
        </span>
      </div>
      {space.sections.map((section) => (
        <SidebarSection key={section.title} title={section.title}>
          {section.items.map((item) => {
            const active = activeItemId === item.id;

            return (
              <button
                aria-label={item.label}
                aria-pressed={active}
                className={`grid w-full grid-cols-[1.25rem_minmax(0,1fr)] items-center gap-3 rounded-lg px-3 py-2 text-left transition ${
                  active
                    ? "bg-sidebar-selected text-sidebar-active"
                    : "text-sidebar-text hover:bg-sidebar-hover hover:text-sidebar-active"
                }`}
                key={item.id}
                onClick={() => setActiveItemId(item.id)}
                type="button"
              >
                <Icon className="h-5 w-5 text-current" name={item.icon} />
                <span className="min-w-0 truncate text-[13px] font-medium leading-5">{item.label}</span>
              </button>
            );
          })}
        </SidebarSection>
      ))}
    </div>
  );
}

function isSidebarSpaceId(value: string): value is SidebarSpaceId {
  return value === "current" || workspaceSidebarSpaces.some((space) => space.id === value);
}
