import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/utils";

type DealsToolbarButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  active?: boolean;
  children: ReactNode;
};

export const DealsToolbarButton = forwardRef<HTMLButtonElement, DealsToolbarButtonProps>(
  function DealsToolbarButton(
    { active = false, children, className, type = "button", ...props },
    ref,
  ) {
    return (
      <button
        className={cn(
          "inline-flex h-8 w-8 items-center justify-center rounded-full border shadow-sm transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed",
          active
            ? "border-primary-container bg-primary-container text-on-primary-container"
            : "border-outline-variant bg-surface-container-lowest text-text-main hover:bg-surface-container-high",
          className,
        )}
        ref={ref}
        type={type}
        {...props}
      >
        {children}
      </button>
    );
  },
);
