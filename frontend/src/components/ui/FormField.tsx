import { Input } from "./input";
import { PasswordField } from "./password-field";
import { cn } from "../../lib/utils";
import { InputHTMLAttributes, ReactNode } from "react";

type FormFieldProps = Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "onChange"
> & {
  action?: ReactNode;
  icon?: ReactNode;
  label: string;
  onChange?: (value: string) => void;
};

export function FormField({
  action,
  autoComplete,
  icon,
  id,
  label,
  onChange,
  placeholder,
  type = "text",
  value,
  required = true,
  className,
  ...inputProps
}: FormFieldProps) {
  const isPassword =
    autoComplete === "current-password" || autoComplete === "new-password";
  const Control = isPassword ? PasswordField : Input;
  return (
    <div className="space-y-xs">
      <div className="flex items-center justify-between gap-sm px-xs">
        <label className="type-label text-on-surface-variant" htmlFor={id}>
          {label}
        </label>
        {action}
      </div>

      <div className="relative">
        {!isPassword && icon && (
          <div aria-hidden="true" className="pointer-events-none absolute left-md top-1/2 z-10 -translate-y-1/2 text-muted">
            {icon}
          </div>
        )}
        <Control
          {...inputProps}
          autoComplete={autoComplete}
          className={cn(
            "h-12 w-full rounded-lg border border-outline-variant bg-surface-container-lowest px-md text-[16px] leading-6 text-on-surface shadow-none outline-none transition-[border-color,box-shadow] placeholder:text-muted focus:border-primary focus:ring-2 focus:ring-primary/15",
            className,
            !isPassword && icon && "pl-12",
          )}
          id={id}
          onChange={
            onChange
              ? (event) => onChange(event.currentTarget.value)
              : undefined
          }
          placeholder={placeholder}
          required={required}
          {...(isPassword ? { visibilityLabel: label.toLowerCase() } : { type })}
          value={value}
        />
      </div>
    </div>
  );
}
