import type { ComponentProps } from "react";
import { Toolbar as ToolbarPrimitive } from "radix-ui";
import { Button } from "./button";
import { cn } from "@/lib/utils";

function Toolbar({ className, ...props }: ComponentProps<typeof ToolbarPrimitive.Root>) {
  return <ToolbarPrimitive.Root data-slot="toolbar" className={cn("flex items-center gap-1", className)} {...props} />;
}

function ToolbarButton({ className, disabled, ...props }: ComponentProps<typeof Button>) {
  return (
    <ToolbarPrimitive.Button asChild disabled={disabled}>
      <Button
        data-slot="toolbar-button" variant="ghost" size="icon-sm" disabled={disabled}
        className={cn("quarry-toolbar-button relative inline-flex size-8 items-center justify-center rounded-[6px] text-muted outline-none transition-[background-color,color,scale] duration-150 [@media(hover:hover)]:hover:bg-surface-container-high [@media(hover:hover)]:hover:text-on-surface active:scale-90 focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-50 motion-reduce:transition-none motion-reduce:active:scale-100 pointer-coarse:size-11", className)}
        {...props}
      />
    </ToolbarPrimitive.Button>
  );
}

export { Toolbar, ToolbarButton };
