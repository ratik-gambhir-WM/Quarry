import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

type DialogBackdropProps = {
  children: ReactNode;
  className?: string;
  closeLabel: string;
  disabled?: boolean;
  onClose: () => void;
};

export function DialogBackdrop({
  children,
  className,
  closeLabel,
  disabled = false,
  onClose,
}: DialogBackdropProps) {
  return (
    <div
      className={cn(
        "modal-backdrop fixed inset-0 z-[100] flex items-center justify-center px-4 py-6",
        className,
      )}
      role="presentation"
    >
      <button
        aria-label={closeLabel}
        className="absolute inset-0 cursor-default disabled:cursor-wait"
        disabled={disabled}
        onClick={onClose}
        type="button"
      />
      {children}
    </div>
  );
}
