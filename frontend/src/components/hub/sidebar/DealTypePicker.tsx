import { Field, FieldError, FieldLabel } from "../../ui/field";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../ui/select";

export const transactionTypeOptions = ["Acquisition", "Divestiture", "Merger", "Recapitalization", "Growth Equity", "Other"];

type TransactionTypePickerProps = {
  error?: string;
  onChange: (value: string) => void;
  value: string;
};

export function TransactionTypePicker({ error = "", onChange, value }: TransactionTypePickerProps) {
  return (
    <Field data-invalid={Boolean(error)}>
      <FieldLabel htmlFor="add-deal-transaction-type">Transaction type</FieldLabel>
      <Select onValueChange={onChange} required value={value}>
        <SelectTrigger
          aria-invalid={Boolean(error)}
          className="w-full"
          id="add-deal-transaction-type"
        >
          <SelectValue placeholder="Select transaction type" />
        </SelectTrigger>
        <SelectContent position="popper">
          {transactionTypeOptions.map((transactionType) => (
            <SelectItem key={transactionType} value={transactionType}>
              {transactionType}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <FieldError>{error}</FieldError>
    </Field>
  );
}
