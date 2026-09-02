import { SearchIcon } from "lucide-react";
import { forwardRef, type ComponentProps } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export type DocumentSearchButtonProps = ComponentProps<typeof Button> & {
  iconOnly?: boolean;
  showShortcut?: boolean;
};

export const DocumentSearchButton = forwardRef<
  HTMLButtonElement,
  DocumentSearchButtonProps
>(function DocumentSearchButton(
  { children, className, iconOnly = false, showShortcut = false, ...buttonProps },
  ref,
) {
  const baseClassName =
    "h-auto cursor-pointer justify-between border py-3 shadow-none transition-transform duration-400 translate-y-0 hover:-translate-y-0.5 hover:bg-transparent hover:shadow-md md:min-w-[200px] motion-reduce:transform-none motion-reduce:transition-none";

  return (
    <Button
      aria-label="Open search"
      className={cn(baseClassName, className)}
      ref={ref}
      type="button"
      variant="outline"
      {...buttonProps}
    >
      <span className="flex items-center justify-center gap-2 text-muted-foreground opacity-80">
        <SearchIcon aria-hidden="true" size={iconOnly ? 20 : 24} strokeWidth={1.5} />
        {iconOnly ? null : (
          <span className="hidden sm:inline">{children ?? "Search"}</span>
        )}
      </span>
      {showShortcut ? (
        <span aria-hidden="true" className="hidden gap-0.5 md:flex">
          <kbd className="grid h-5 min-w-5 place-items-center rounded bg-muted text-xs text-muted-foreground">
            ⌘
          </kbd>
          <kbd className="grid h-5 min-w-5 place-items-center rounded bg-muted text-xs text-muted-foreground">
            K
          </kbd>
        </span>
      ) : null}
    </Button>
  );
});
