// Adapted from https://www.stealth.pm/ui/alert-dialog; Quarry theme tokens and existing state owners.
import { AlertDialog as BaseAlertDialog } from "@base-ui/react/alert-dialog";
import { cn } from "@/lib/utils";

export const AlertDialog = BaseAlertDialog.Root;
export function AlertDialogContent({
  className,
  children,
  ...props
}: Omit<BaseAlertDialog.Popup.Props, "className"> & { className?: string }) {
  return (
    <BaseAlertDialog.Portal>
      <BaseAlertDialog.Backdrop className="fixed inset-0 z-50 bg-black/30 transition-opacity duration-200 ease-out data-starting-style:opacity-0 data-ending-style:opacity-0 motion-reduce:transition-none" />
      <BaseAlertDialog.Viewport className="fixed inset-0 z-50 flex items-center justify-center p-4 max-sm:items-end max-sm:p-2 max-sm:pb-[max(8px,env(safe-area-inset-bottom))]">
        <BaseAlertDialog.Popup
          className={cn(
            "relative flex w-full max-w-[400px] flex-col rounded-2xl border border-outline-variant bg-surface-container-lowest p-5 text-on-surface shadow-xl outline-none origin-center transition-[opacity,scale,translate] duration-[260ms] ease-out data-starting-style:translate-y-1.5 data-starting-style:scale-[0.97] data-starting-style:opacity-0 data-ending-style:scale-[0.98] data-ending-style:opacity-0 data-ending-style:duration-[160ms] motion-reduce:transition-none",
            className,
          )}
          {...props}
        >
          {children}
        </BaseAlertDialog.Popup>
      </BaseAlertDialog.Viewport>
    </BaseAlertDialog.Portal>
  );
}
export function AlertDialogTitle({
  className,
  ...props
}: Omit<BaseAlertDialog.Title.Props, "className"> & { className?: string }) {
  return (
    <BaseAlertDialog.Title
      className={cn(
        "text-[15px] font-medium leading-[1.3] tracking-[-0.015em] text-on-surface text-balance",
        className,
      )}
      {...props}
    />
  );
}
export function AlertDialogDescription({
  className,
  ...props
}: Omit<BaseAlertDialog.Description.Props, "className"> & {
  className?: string;
}) {
  return (
    <BaseAlertDialog.Description
      className={cn(
        "mt-1.5 text-[13px] leading-[1.5] text-muted text-pretty",
        className,
      )}
      {...props}
    />
  );
}
