// Adapted from https://www.stealth.pm/ui/field; Quarry theme tokens and existing state owners.
import { Field as BaseField } from "@base-ui/react/field";
import { cn } from "@/lib/utils";
export type FieldProps = Omit<BaseField.Root.Props, "className"> & {
  className?: string;
};
export function Field({
  validationMode = "onBlur",
  className,
  ...props
}: FieldProps) {
  return (
    <BaseField.Root
      validationMode={validationMode}
      className={cn(
        "group/field flex w-full min-w-0 flex-col gap-1.5",
        className,
      )}
      {...props}
    />
  );
}
export function FieldLabel({
  className,
  ...props
}: Omit<BaseField.Label.Props, "className"> & { className?: string }) {
  return (
    <BaseField.Label
      className={cn(
        "flex w-fit select-none items-baseline gap-1 text-[12.5px] font-medium tracking-[-0.005em] text-muted transition-colors duration-150 group-data-focused/field:text-on-surface group-data-disabled/field:opacity-50 motion-reduce:transition-none",
        className,
      )}
      {...props}
    />
  );
}

export type FieldDescriptionProps = Omit<
  BaseField.Description.Props,
  "className"
> & { className?: string };

export function FieldDescription({
  className,
  ...rest
}: FieldDescriptionProps) {
  return (
    <BaseField.Description
      className={cn(
        "text-[12px] leading-[1.45] text-muted [text-wrap:pretty] group-data-disabled/field:text-muted",
        className,
      )}
      {...rest}
    />
  );
}
