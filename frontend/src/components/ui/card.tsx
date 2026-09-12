import * as React from "react";

import { cn } from "@/lib/utils";

function Card({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      className={cn(
        "overflow-hidden rounded-lg bg-surface-container text-text-main ring-1 ring-outline-variant",
        className,
      )}
      data-slot="card"
      {...props}
    />
  );
}

export { Card };
