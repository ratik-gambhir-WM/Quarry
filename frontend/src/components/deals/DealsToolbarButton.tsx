import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import { cn } from "../../lib/utils";

type DealsToolbarButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  children: ReactNode;
};

export const DealsToolbarButton = forwardRef<HTMLButtonElement, DealsToolbarButtonProps>(
  function DealsToolbarButton(
    { children, className, type = "button", ...props },
    ref,
  ) {
    return (
      <button
        className={cn(
          "inline-flex h-8 w-8 items-center justify-center rounded-md border-0 bg-transparent text-text-main shadow-none transition-colors hover:!bg-transparent hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed",
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
