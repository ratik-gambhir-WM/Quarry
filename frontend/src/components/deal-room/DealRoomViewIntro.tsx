import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

type DealRoomViewIntroProps = {
  action?: ReactNode;
  className?: string;
  description: ReactNode;
  eyebrow: ReactNode;
  title: ReactNode;
};

export function DealRoomViewIntro({ action, className, description, eyebrow, title }: DealRoomViewIntroProps) {
  return (
    <div className={cn("flex items-start justify-between gap-6", className)}>
      <div>
        <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">{eyebrow}</p>
        <h1 className="mt-3 type-display text-text-main">{title}</h1>
        <p className="mt-4 max-w-2xl text-[15px] leading-7 text-text-main/78">{description}</p>
      </div>
      {action}
    </div>
  );
}
