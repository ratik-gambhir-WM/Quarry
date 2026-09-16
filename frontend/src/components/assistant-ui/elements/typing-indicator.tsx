import type { ComponentProps } from "react";
import { cn } from "../../../lib/utils";

const DOT_DELAYS = ["-0.32s", "-0.16s", "0s"];

// Adapted from assistant-ui's elements-typing-indicator registry item (2026-09-15).
export function TypingIndicator({
  className,
  ...props
}: Omit<ComponentProps<"div">, "children" | "role" | "aria-label">) {
  return (
    <div
      aria-label="Assistant is typing"
      className={cn("flex w-fit gap-1 py-2", className)}
      data-slot="typing-indicator"
      role="status"
      {...props}
    >
      {DOT_DELAYS.map((delay) => (
        <span
          aria-hidden
          className="size-1.5 animate-bounce rounded-full bg-muted-foreground/55 motion-reduce:animate-none"
          key={delay}
          style={{ animationDelay: delay, animationDuration: "1.1s" }}
        />
      ))}
    </div>
  );
}
