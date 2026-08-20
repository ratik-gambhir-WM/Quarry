import { ButtonHTMLAttributes, ReactNode } from "react";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  children: ReactNode;
  icon?: ReactNode;
};

export function Button({ children, className = "", icon, type = "button", ...props }: ButtonProps) {
  return (
    <button
      className={`group inline-flex w-full items-center justify-center gap-sm rounded-full bg-primary-container px-lg py-md text-center text-[14px] font-semibold text-on-primary-container shadow-[0_10px_30px_rgba(7,1,84,0.18)] transition hover:bg-primary hover:shadow-[0_14px_38px_rgba(80,101,142,0.24)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed focus-visible:ring-offset-2 focus-visible:ring-offset-surface-container-lowest disabled:cursor-not-allowed disabled:opacity-60 ${className}`}
      type={type}
      {...props}
    >
      <span>{children}</span>
      {icon ? <span className="transition-transform group-hover:translate-x-1">{icon}</span> : null}
    </button>
  );
}
