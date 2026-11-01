"use client";
import { Menu } from "@base-ui/react/menu";
import { cn } from "@/lib/utils";
import { useControllableState } from "@/components/ui/stealth-date/use-controllable-state";

export type SpeedDialProps = {
  /** What the button does, e.g. "Create". Its accessible name, and a hint beside it on hover. */
  label: string;
  /** The actions, as SpeedDialAction elements, listed top to bottom. */
  children: React.ReactNode;
  open?: boolean;
  defaultOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
  /**
   * Where the button sits. `fixed` pins it to the viewport's bottom-right corner above the
   * safe area; `absolute` to the nearest positioned ancestor's; `inline` leaves it in the flow.
   */
  placement?: "fixed" | "absolute" | "inline";
  /** Render the actions into this element instead of document.body. */
  container?: HTMLElement | React.RefObject<HTMLElement | null> | null;
  /** The glyph on the button. The workspace uses its brand mark. */
  icon: React.ReactNode;
  /** Dim what's behind the actions so their labels stay readable. */
  scrim?: boolean;
  className?: string;
  disabled?: boolean;
  triggerRef?: React.Ref<HTMLButtonElement>;
  restoreFocus?: () => boolean;
};

export function SpeedDial({
  label,
  children,
  open: openProp,
  defaultOpen = false,
  onOpenChange,
  placement = "fixed",
  container,
  icon,
  scrim = true,
  className,
  disabled,
  triggerRef,
  restoreFocus,
}: SpeedDialProps) {
  const [open, setOpen] = useControllableState({ value: openProp, defaultValue: defaultOpen, onChange: onOpenChange });
  return (
    <Menu.Root open={open} onOpenChange={(next) => setOpen(next)} modal={false} disabled={disabled}>
      <div
        className={cn(
          "group/dial isolate",
          // Above the scrim, so the button that closes the dial is never dimmed.
          placement === "fixed" && "fixed bottom-[max(24px,env(safe-area-inset-bottom))] right-6 z-[60]",
          placement === "absolute" && "absolute bottom-4 right-4 z-[60]",
          placement === "inline" && "relative z-[60] inline-flex",
          className,
        )}
      >
        <Menu.Trigger
          ref={triggerRef}
          aria-label={label}
          className={cn(
            "relative grid size-8 place-items-center rounded-full text-primary outline-none",
            "transition-[background-color,scale] duration-150 ease-out hover:bg-sidebar-hover active:scale-[0.94] active:duration-75",
            "focus-visible:outline-solid focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-primary",
            "data-disabled:pointer-events-none data-disabled:opacity-50",
          )}
        >
          <span aria-hidden className={cn("grid place-items-center transition-transform duration-150 motion-reduce:transition-none", open && "scale-95")}>
            {icon}
          </span>
        </Menu.Trigger>
        {/* A quiet name beside the button on hover; the button already carries it as its label. */}
        <span
          aria-hidden
          className={cn(
            "pointer-events-none absolute right-full top-1/2 mr-3 -translate-y-1/2 translate-x-1 whitespace-nowrap rounded-md border border-outline-variant bg-surface-container-lowest px-2 py-1 text-[12px] font-medium text-primary opacity-0 shadow-[var(--theme-panel-shadow)]",
            "transition-[opacity,translate] duration-150 ease-out",
            !open && "group-hover/dial:translate-x-0 group-hover/dial:opacity-100 group-hover/dial:delay-300",
          )}
        >
          {label}
        </span>
      </div>

      <Menu.Portal container={container}>
        {scrim && (
          <Menu.Backdrop
            className={cn(container ? "absolute" : "fixed", "inset-0 z-50 bg-background/80 transition-opacity duration-150 data-starting-style:opacity-0 data-ending-style:opacity-0 motion-reduce:transition-none")}
          />
        )}
        <Menu.Positioner side="top" align="end" sideOffset={12} className="z-[60] outline-none">
          <Menu.Popup
            finalFocus={() => restoreFocus?.() !== false}
            className="flex flex-col items-end gap-2.5 pr-1 outline-none transition-[opacity,transform] duration-150 ease-out data-starting-style:translate-y-1 data-starting-style:opacity-0 data-ending-style:translate-y-1 data-ending-style:opacity-0 motion-reduce:transition-none"
          >
            {children}
          </Menu.Popup>
        </Menu.Positioner>
      </Menu.Portal>
    </Menu.Root>
  );
}

export type SpeedDialActionProps = {
  /** The action's name, shown in a pill beside its button. */
  children: React.ReactNode;
  icon: React.ReactNode;
  onClick?: (event: React.MouseEvent<HTMLElement>) => void;
  disabled?: boolean;
  /** Keep the dial open after this action, e.g. for toggles. */
  closeOnClick?: boolean;
  className?: string;
};

export function SpeedDialAction({ children, icon, onClick, disabled, closeOnClick = true, className }: SpeedDialActionProps) {
  return (
    <Menu.Item
      disabled={disabled}
      closeOnClick={closeOnClick}
      onClick={onClick}
      className={cn(
        "group/action flex cursor-default select-none items-center gap-3 outline-none",
        "data-disabled:pointer-events-none data-disabled:opacity-50",
        className,
      )}
    >
      <span
        className={cn(
          "whitespace-nowrap rounded-lg border border-outline-variant bg-surface-container-lowest px-2.5 py-1.5 text-[12.5px] font-medium leading-none text-on-surface shadow-[var(--theme-panel-shadow)]",
          "transition-[color,border-color] duration-150",
          "group-data-highlighted/action:border-primary/40 group-data-highlighted/action:text-primary",
        )}
      >
        {children}
      </span>
      <span
        className={cn(
          "grid size-10 shrink-0 place-items-center rounded-full border border-outline-variant bg-surface-container-lowest text-on-surface shadow-[var(--theme-panel-shadow)]",
          "transition-[background-color,color,border-color,scale] duration-150 ease-out group-active/action:scale-[0.92]",
          "group-data-highlighted/action:border-primary/40 group-data-highlighted/action:bg-sidebar-hover group-data-highlighted/action:text-primary",
        )}
      >
        {icon}
      </span>
    </Menu.Item>
  );
}
