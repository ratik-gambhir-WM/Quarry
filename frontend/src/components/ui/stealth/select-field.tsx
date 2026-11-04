import { useId } from "react";
import { Field, FieldLabel } from "./field";
import { Select, SelectTrigger, SelectPopup, SelectItem } from "./select";

export function SelectField<Value extends string>({
  label,
  value,
  onValueChange,
  options,
  disabled,
  placeholder = "Select…",
  className,
}: {
  label: string;
  value: Value | null;
  onValueChange: (value: Value) => void;
  options: ReadonlyArray<{ value: Value; label: string }>;
  disabled?: boolean;
  placeholder?: string;
  className?: string;
}) {
  const id = useId();
  return (
    <Field disabled={disabled} className={className}>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select<Value>
        items={options}
        value={value}
        disabled={disabled}
        onValueChange={(next) => {
          if (next !== null) onValueChange(next);
        }}
      >
        <SelectTrigger id={id} placeholder={placeholder} />
        <SelectPopup>
          {options.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectPopup>
      </Select>
    </Field>
  );
}
