import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

const dotDelays = ["-0.32s", "-0.16s", "0s"];

export function TypingIndicator({
  className,
  ...props
}: Omit<ComponentProps<"div">, "aria-label" | "children" | "role">) {
  return (
    <div aria-label="Assistant is typing" className={cn("flex w-fit gap-1 py-2", className)} data-slot="typing-indicator" role="status" {...props}>
      {dotDelays.map((delay) => <span aria-hidden className="size-1.5 animate-bounce rounded-full bg-muted-foreground/55 motion-reduce:animate-none" key={delay} style={{ animationDelay: delay, animationDuration: "1.1s" }} />)}
    </div>
  );
}
