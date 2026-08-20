import type { SVGAttributes } from "react";

type ViewColumnsIconProps = SVGAttributes<SVGSVGElement> & {
  size?: number;
};

export function ViewColumnsIcon({ className, size = 24, ...props }: ViewColumnsIconProps) {
  return (
    <svg
      aria-hidden="true"
      className={className}
      fill="none"
      height={size}
      stroke="currentColor"
      strokeLinecap="round"
      strokeLinejoin="round"
      strokeWidth="1.5"
      viewBox="0 0 24 24"
      width={size}
      {...props}
    >
      <path d="M4.125 19.5h15.75c.621 0 1.125-.504 1.125-1.125V5.625c0-.621-.504-1.125-1.125-1.125H4.125C3.504 4.5 3 5.004 3 5.625v12.75c0 .621.504 1.125 1.125 1.125Z" />
      <path
        className="[stroke-dasharray:15] [stroke-dashoffset:0] group-hover:animate-[view-columns-line-draw_.3s_linear_both]"
        d="M9 4.5v15"
      />
      <path
        className="[stroke-dasharray:15] [stroke-dashoffset:0] group-hover:animate-[view-columns-line-draw_.3s_linear_both] group-hover:[animation-delay:.15s]"
        d="M15 4.5v15"
      />
    </svg>
  );
}
