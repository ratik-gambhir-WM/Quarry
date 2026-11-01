import { ButtonHTMLAttributes, ReactNode } from "react";

type QuarryButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  children: ReactNode;
  icon?: ReactNode;
};

export function QuarryButton({
  children,
  className = "",
  icon,
  type = "button",
  ...props
}: QuarryButtonProps) {
  return (
    <button
      className={`group inline-flex min-h-12 w-full items-center justify-center gap-sm rounded-lg bg-action px-lg py-sm text-center text-[14px] font-medium text-on-action active:translate-y-px shadow-sm transition-[background-color,box-shadow,transform] enabled:hover:bg-action-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary focus-visible:ring-offset-2 focus-visible:ring-offset-surface-container-lowest disabled:cursor-not-allowed disabled:opacity-60 ${className}`}
      type={type}
      {...props}
    >
      <span>{children}</span>
      {icon ? (
        <span className="transition-transform group-hover:translate-x-1">
          {icon}
        </span>
      ) : null}
    </button>
  );
}
