import type { ReactNode } from "react";
import { cn } from "../../lib/utils";
import { Icon } from "./Icon";

type DialogHeaderProps = {
  className?: string;
  closeLabel: string;
  description?: ReactNode;
  disabled?: boolean;
  eyebrow: ReactNode;
  onClose: () => void;
  title: ReactNode;
  titleId: string;
};

export function DialogHeader({
  className,
  closeLabel,
  description,
  disabled = false,
  eyebrow,
  onClose,
  title,
  titleId,
}: DialogHeaderProps) {
  return (
    <header className={cn("flex items-start justify-between gap-5", className)}>
      <div>
        <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">{eyebrow}</p>
        <h2
          className="mt-2 text-[2rem] font-bold leading-none text-text-main [font-family:var(--font-heading)]"
          id={titleId}
        >
          {title}
        </h2>
        {description ? <p className="mt-2 text-[13px] leading-5 text-muted">{description}</p> : null}
      </div>
      <button
        aria-label={closeLabel}
        className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-40"
        disabled={disabled}
        onClick={onClose}
        type="button"
      >
        <Icon className="h-5 w-5 rotate-45" name="plus" />
      </button>
    </header>
  );
}
