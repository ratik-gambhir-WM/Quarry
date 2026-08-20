import type { CSSProperties, HTMLAttributes, ReactNode } from "react";

export type WorkspaceCardRadius = "compact" | "default" | "none" | "small";

type WorkspaceCardProps = Omit<HTMLAttributes<HTMLElement>, "children"> & {
  children: ReactNode;
  radius?: WorkspaceCardRadius;
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
  radius = "default",
  style,
  ...props
}: WorkspaceCardProps) {
  return (
    <section
      className={`workspace-card ${className}`}
      style={{ borderRadius: radiusValues[radius], ...style }}
      {...props}
    >
      {children}
    </section>
  );
}
