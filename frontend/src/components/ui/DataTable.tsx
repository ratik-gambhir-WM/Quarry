import type { ComponentPropsWithoutRef } from "react";
import { cn } from "../../lib/utils";

type DataTableHeaderRowProps = ComponentPropsWithoutRef<"tr"> & {
  surface?: boolean;
};

type DataTableHeadingProps = ComponentPropsWithoutRef<"th"> & {
  density?: "compact" | "default";
};

export function DataTableHeaderRow({ className, surface = false, ...props }: DataTableHeaderRowProps) {
  return (
    <tr
      className={cn(
        "border-b border-outline-variant/70",
        surface && "bg-surface-container-low/70",
        className,
      )}
      {...props}
    />
  );
}

export function DataTableHeading({
  className,
  density = "default",
  scope = "col",
  ...props
}: DataTableHeadingProps) {
  return (
    <th
      className={cn(
        "px-4 py-3 uppercase text-muted",
        density === "compact"
          ? "text-[10px] font-bold tracking-[0.14em]"
          : "text-[11px] font-medium tracking-[0.08em]",
        className,
      )}
      scope={scope}
      {...props}
    />
  );
}
