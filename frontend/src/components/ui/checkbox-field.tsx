import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

type CheckboxFieldProps = Omit<ComponentProps<"input">, "type">;

// Adapted from Stealth Checkbox while retaining the native input and form events.
export function CheckboxField({ className, ...props }: CheckboxFieldProps) {
  return (
    <span className={cn("relative inline-grid h-4 w-4 shrink-0 place-items-center", className)}>
      <input {...props} type="checkbox" className="peer absolute inset-0 z-10 m-0 h-full w-full cursor-pointer opacity-0 disabled:cursor-not-allowed" />
      <span aria-hidden="true" className="grid h-full w-full place-items-center rounded-[4px] border border-outline bg-surface-container-lowest text-on-action transition-[background-color,border-color,transform] duration-150 peer-checked:border-action peer-checked:bg-action peer-checked:[&_path]:[stroke-dashoffset:0] peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-primary peer-disabled:opacity-50 peer-active:scale-90 motion-reduce:transition-none">
        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="size-3">
          <path className="[stroke-dasharray:20] [stroke-dashoffset:20] transition-[stroke-dashoffset] duration-200 motion-reduce:transition-none" d="M3.75 8.25 6.75 11.25 12.25 4.75" />
        </svg>
      </span>
    </span>
  );
}
