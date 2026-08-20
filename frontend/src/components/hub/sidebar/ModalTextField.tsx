type ModalTextFieldProps = {
  autoComplete?: string;
  label: string;
  onChange: (value: string) => void;
  placeholder: string;
  required?: boolean;
  type?: "date" | "text" | "url";
  value: string;
};

export function ModalTextField({ autoComplete, label, onChange, placeholder, required = true, type = "text", value }: ModalTextFieldProps) {
  const id = `add-deal-${label.toLowerCase().replace(/\s+/g, "-")}`;

  return (
    <div className="space-y-2">
      <label className="flex items-center justify-between px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted" htmlFor={id}>
        <span>{label}</span>
        {!required ? <span className="font-semibold normal-case tracking-normal">Optional</span> : null}
      </label>
      <input
        autoComplete={autoComplete}
        className="w-full rounded-2xl border border-outline-variant bg-surface-container-lowest px-4 py-3 text-[14px] text-text-main outline-none transition placeholder:text-muted/60 focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40"
        id={id}
        onChange={(event) => onChange(event.currentTarget.value)}
        placeholder={placeholder}
        required={required}
        type={type}
        value={value}
      />
    </div>
  );
}
