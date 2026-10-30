import { Search, X } from "lucide-react";
import { useRef, type ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { Input } from "./input";

type SearchFieldProps = Omit<ComponentProps<"input">, "type" | "value" | "onChange"> & {
  value: string;
  onValueChange: (value: string) => void;
  clearLabel: string;
  containerClassName?: string;
};

// Adapted from Stealth Search Field without installing a global keyboard shortcut.
export function SearchField({ value, onValueChange, clearLabel, containerClassName, className, disabled, ref, ...props }: SearchFieldProps) {
  const inputRef = useRef<HTMLInputElement | null>(null);
  return (
    <div className={cn("group/search relative min-w-0", containerClassName)}>
      <Search aria-hidden="true" className="pointer-events-none absolute left-3 top-1/2 z-10 size-4 -translate-y-1/2 text-muted transition-colors group-focus-within/search:text-primary motion-reduce:transition-none" />
      <Input
        {...props}
        className={cn("w-full pl-9 pr-9 [&::-webkit-search-cancel-button]:appearance-none", className)}
        disabled={disabled}
        ref={(node) => {
          inputRef.current = node;
          if (typeof ref === "function") ref(node);
          else if (ref) ref.current = node;
        }}
        type="search"
        value={value}
        onChange={(event) => onValueChange(event.currentTarget.value)}
      />
      <button
        type="button"
        aria-label={clearLabel}
        disabled={disabled || !value}
        tabIndex={value ? 0 : -1}
        onClick={() => { onValueChange(""); inputRef.current?.focus(); }}
        className={cn("absolute right-1.5 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-md text-muted outline-none transition-[opacity,scale,color] duration-150 hover:text-on-surface focus-visible:ring-2 focus-visible:ring-primary motion-reduce:transition-none", value ? "opacity-100" : "pointer-events-none scale-75 opacity-0")}
      >
        <X aria-hidden="true" className="size-4" />
      </button>
    </div>
  );
}
