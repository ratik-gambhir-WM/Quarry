import * as React from "react";
import { cn } from "@/lib/utils";

// Adapted from Stealth Textarea; native field sizing keeps form and editor events intact.
function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        "field-sizing-content min-h-16 w-full rounded-lg border border-outline-variant bg-surface-container-lowest px-2.5 py-2 text-base text-on-surface shadow-sm outline-none transition-[border-color,box-shadow,background-color] duration-150 ease-out placeholder:text-muted hover:border-outline focus-visible:border-primary focus-visible:ring-3 focus-visible:ring-primary/10 disabled:cursor-not-allowed disabled:opacity-50 read-only:bg-surface-container-low read-only:shadow-none aria-invalid:border-error aria-invalid:focus-visible:ring-error/15 md:text-sm motion-reduce:transition-none",
        className,
      )}
      {...props}
    />
  );
}

export { Textarea };
