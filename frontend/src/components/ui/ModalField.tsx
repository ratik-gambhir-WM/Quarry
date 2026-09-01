import { forwardRef, type ComponentPropsWithoutRef, type ReactNode } from "react";
import { cn } from "../../lib/utils";

type ModalFieldProps = {
  children: ReactNode;
  className?: string;
  error?: string;
  htmlFor?: string;
  label: ReactNode;
  labelId?: string;
  optional?: boolean;
};

type ModalInputProps = Omit<ComponentPropsWithoutRef<"input">, "onChange"> & {
  onValueChange?: (value: string) => void;
};

type ModalTextFieldProps = Omit<ModalInputProps, "id"> & {
  error?: string;
  id: string;
  label: ReactNode;
  optional?: boolean;
};

export const modalControlClassName =
  "w-full rounded-2xl border border-outline-variant bg-surface-container-lowest px-4 py-3 text-[14px] text-text-main outline-none transition placeholder:text-muted/60 focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40";

export function ModalField({
  children,
  className,
  error,
  htmlFor,
  label,
  labelId,
  optional = false,
}: ModalFieldProps) {
  return (
    <div className={cn("space-y-2", className)}>
      <label
        className="flex items-center justify-between px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted"
        htmlFor={htmlFor}
        id={labelId}
      >
        <span>{label}</span>
        {optional ? <span className="font-semibold normal-case tracking-normal">Optional</span> : null}
      </label>
      {children}
      {error ? <p className="px-1 text-[12px] font-medium text-error">{error}</p> : null}
    </div>
  );
}

export const ModalInput = forwardRef<HTMLInputElement, ModalInputProps>(function ModalInput(
  { className, onValueChange, ...props },
  ref,
) {
  return (
    <input
      className={cn(modalControlClassName, className)}
      onChange={onValueChange ? (event) => onValueChange(event.currentTarget.value) : undefined}
      ref={ref}
      {...props}
    />
  );
});

export const ModalTextField = forwardRef<HTMLInputElement, ModalTextFieldProps>(function ModalTextField(
  { className, error, id, label, optional = false, required = !optional, ...props },
  ref,
) {
  return (
    <ModalField error={error} htmlFor={id} label={label} optional={optional}>
      <ModalInput className={className} id={id} ref={ref} required={required} {...props} />
    </ModalField>
  );
});
