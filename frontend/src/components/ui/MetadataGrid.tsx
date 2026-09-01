import type { ComponentPropsWithoutRef, ReactNode } from "react";
import { cn } from "../../lib/utils";

type MetadataGridProps = ComponentPropsWithoutRef<"dl">;

type MetadataItemProps = {
  className?: string;
  label: ReactNode;
  truncate?: boolean;
  value: ReactNode;
  valueClassName?: string;
};

export function MetadataGrid({ className, ...props }: MetadataGridProps) {
  return (
    <dl
      className={cn(
        "grid grid-cols-2 gap-x-3 gap-y-3 border-t border-outline-variant/60 pt-3",
        className,
      )}
      {...props}
    />
  );
}

export function MetadataItem({
  className,
  label,
  truncate = false,
  value,
  valueClassName,
}: MetadataItemProps) {
  return (
    <div className={cn("min-w-0", className)}>
      <dt className="text-[10px] font-normal uppercase tracking-[0.08em] text-muted">{label}</dt>
      <dd
        className={cn(
          "mt-1 text-[11px] font-normal text-on-surface",
          truncate && "truncate",
          valueClassName,
        )}
      >
        {value}
      </dd>
    </div>
  );
}
