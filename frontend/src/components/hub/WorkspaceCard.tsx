import type { CSSProperties, HTMLAttributes, ReactNode } from "react";
import { cn } from "../../lib/utils";

export type WorkspaceCardRadius = "compact" | "default" | "none" | "small";

type WorkspaceCardProps = Omit<HTMLAttributes<HTMLElement>, "children"> & {
  children: ReactNode;
  interactive?: boolean;
  radius?: WorkspaceCardRadius;
  surface?: "chrome" | "default";
};

const radiusValues: Record<WorkspaceCardRadius, CSSProperties["borderRadius"]> = {
  compact: "19px",
  default: "21px",
  none: 0,
  small: "16px",
};

export function WorkspaceCard({
  children,
  className = "",
  interactive = true,
  radius = "default",
  surface = "default",
  style,
  ...props
}: WorkspaceCardProps) {
  return (
    <section
      className={cn(
        "workspace-card",
        surface === "chrome" && "workspace-card--chrome",
        !interactive && "workspace-card--static",
        className,
      )}
      style={{ borderRadius: radiusValues[radius], ...style }}
      {...props}
    >
      {children}
    </section>
  );
}
