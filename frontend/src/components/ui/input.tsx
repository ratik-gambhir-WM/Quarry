import * as React from "react";
import { Input as BaseInput } from "@base-ui/react/input";
import { cn } from "@/lib/utils";

// Adapted from Stealth Input, keeping Quarry's native input props and theme tokens.
function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <BaseInput
      type={type}
      data-slot="input"
      className={cn(
        "h-8 w-full min-w-0 rounded-lg border border-outline-variant bg-surface-container-lowest px-2.5 py-1 text-base text-on-surface shadow-sm outline-none transition-[border-color,box-shadow,background-color] duration-150 ease-out placeholder:text-muted hover:border-outline focus-visible:border-primary focus-visible:ring-3 focus-visible:ring-primary/10 disabled:cursor-not-allowed disabled:opacity-50 read-only:bg-surface-container-low read-only:shadow-none aria-invalid:border-error aria-invalid:focus-visible:ring-error/15 data-invalid:border-error md:text-sm motion-reduce:transition-none",
        className,
      )}
      {...props}
    />
  );
}

export { Input };
