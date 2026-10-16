import { codeLabel, transactionTypeOptions, type TransactionType } from "../../../data/dealModel";

export function CodeSelect<T extends string>({ label, options, value, onChange }: {
  label: string; options: readonly T[]; value: T; onChange: (value: T) => void;
}) {
  return <label className="grid gap-2 text-sm font-medium">{label}
    <select className="h-9 w-full rounded-md border border-input bg-background px-3 text-sm" value={value} onChange={(event) => {
      const selected = options.find((option) => option === event.currentTarget.value);
      if (selected) onChange(selected);
    }}>{options.map((option) => <option key={option} value={option}>{codeLabel(option)}</option>)}</select>
  </label>;
}

export function TransactionTypePicker({ value, onChange }: { value: TransactionType; onChange: (value: TransactionType) => void }) {
  return <CodeSelect label="Transaction type" options={transactionTypeOptions} value={value} onChange={onChange} />;
}
