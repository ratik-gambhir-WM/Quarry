import { Eye, EyeOff } from "lucide-react";
import { useLayoutEffect, useRef, useState, type ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { Input } from "./input";

type PasswordFieldProps = Omit<ComponentProps<"input">, "type"> & {
  revealed?: boolean;
  onRevealedChange?: (value: boolean) => void;
  showToggle?: boolean;
  visibilityLabel?: string;
};

// Adapted from Stealth Password Field. The native input remains uncontrolled when
// callers use browser/password-manager autofill rather than React state.
export function PasswordField({ revealed, onRevealedChange, showToggle = true, visibilityLabel = "password", className, onKeyDown, onKeyUp, onBlur, ref, ...props }: PasswordFieldProps) {
  const [localRevealed, setLocalRevealed] = useState(false);
  const [capsLock, setCapsLock] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const selection = useRef<[number, number, "forward" | "backward" | "none"] | null>(null);
  const visible = revealed ?? localRevealed;

  useLayoutEffect(() => {
    const input = inputRef.current;
    const saved = selection.current;
    if (input && saved && document.activeElement === input) {
      input.setSelectionRange(saved[0], saved[1], saved[2]);
    }
    selection.current = null;
  }, [visible]);

  function toggle() {
    const input = inputRef.current;
    if (input && document.activeElement === input && input.selectionStart !== null) {
      selection.current = [input.selectionStart, input.selectionEnd ?? input.selectionStart, input.selectionDirection ?? "none"];
    }
    if (revealed === undefined) setLocalRevealed(!visible);
    onRevealedChange?.(!visible);
  }

  return (
    <div className="min-w-0">
      <div className="relative">
        <Input
          {...props}
          className={cn(className, showToggle && "pr-10")}
          ref={(node) => {
            inputRef.current = node;
            if (typeof ref === "function") ref(node);
            else if (ref) ref.current = node;
          }}
          type={visible ? "text" : "password"}
          onKeyDown={(event) => { setCapsLock(event.getModifierState?.("CapsLock") ?? false); onKeyDown?.(event); }}
          onKeyUp={(event) => { setCapsLock(event.getModifierState?.("CapsLock") ?? false); onKeyUp?.(event); }}
          onBlur={(event) => { setCapsLock(false); onBlur?.(event); }}
        />
        {showToggle && (
          <button
            type="button"
            aria-label={`${visible ? "Hide" : "Show"} ${visibilityLabel}`}
            disabled={props.disabled}
            onMouseDown={(event) => event.preventDefault()}
            onClick={toggle}
            className="absolute right-1 top-1/2 grid size-7 -translate-y-1/2 place-items-center rounded-md text-muted transition-[color,transform] duration-150 hover:text-on-surface active:scale-90 focus-visible:outline-2 focus-visible:outline-primary disabled:opacity-50 motion-reduce:transition-none"
          >
            {visible ? <EyeOff aria-hidden="true" className="size-4" /> : <Eye aria-hidden="true" className="size-4" />}
          </button>
        )}
      </div>
      {capsLock && !visible && <p role="status" className="mt-1 text-xs text-muted">Caps Lock is on</p>}
    </div>
  );
}
