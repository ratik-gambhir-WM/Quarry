import { InputHTMLAttributes, ReactNode } from "react";

type FormFieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "onChange"> & {
  action?: ReactNode;
  icon: ReactNode;
  label: string;
  onChange: (value: string) => void;
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
}: FormFieldProps) {
  return (
    <div className="space-y-xs">
      <div className="flex items-center justify-between gap-sm px-xs">
        <label className="type-label text-on-surface-variant" htmlFor={id}>
          {label}
        </label>
        {action}
      </div>

      <div className="relative">
        <div className="pointer-events-none absolute left-md top-1/2 -translate-y-1/2 text-outline">{icon}</div>
        <input
          autoComplete={autoComplete}
          className="w-full rounded-full border border-outline-variant bg-surface-container-lowest py-md pl-[52px] pr-md text-[16px] leading-[1.6] text-on-surface shadow-[inset_0_1px_0_rgba(255,255,255,0.5)] outline-none transition placeholder:text-outline-variant focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40"
          id={id}
          onChange={(event) => onChange(event.currentTarget.value)}
          placeholder={placeholder}
          required
          type={type}
          value={value}
        />
      </div>
    </div>
  );
}
