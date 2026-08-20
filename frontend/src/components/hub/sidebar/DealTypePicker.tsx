import { useEffect, useRef, useState } from "react";
import { Icon } from "../../ui/Icon";

export const dealTypeOptions = ["Buy-side", "Sell-side", "Carve-out", "Add-on", "Recapitalization", "Growth equity"];

type DealTypePickerProps = {
  error?: string;
  onChange: (value: string) => void;
  value: string;
};

export function DealTypePicker({ error = "", onChange, value }: DealTypePickerProps) {
  const [open, setOpen] = useState(false);
  const pickerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!pickerRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, []);

  return (
    <div className="relative" ref={pickerRef}>
      <label className="px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted" id="add-deal-type-label">
        Type of deal
      </label>
      <button
        aria-expanded={open}
        aria-haspopup="listbox"
        aria-labelledby="add-deal-type-label"
        className={`mt-2 flex w-full items-center justify-between rounded-2xl border border-outline-variant bg-surface-container-lowest px-4 py-3 text-left text-[14px] outline-none transition focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40 ${
          value ? "text-text-main" : "text-muted/60"
        }`}
        onClick={() => setOpen((isOpen) => !isOpen)}
        type="button"
      >
        <span>{value || "Select deal type"}</span>
        <Icon className={`h-4 w-4 text-muted transition ${open ? "rotate-180" : ""}`} name="chevronDown" />
      </button>

      {open ? (
        <div
          aria-labelledby="add-deal-type-label"
          className="absolute left-0 right-0 top-[calc(100%+0.5rem)] z-30 overflow-hidden rounded-2xl border border-outline-variant bg-white p-1.5 shadow-[0_18px_44px_rgba(7,1,84,0.14)]"
          role="listbox"
        >
          {dealTypeOptions.map((dealType) => {
            const selected = dealType === value;

            return (
              <button
                aria-selected={selected}
                className={`flex w-full items-center justify-between rounded-xl px-3 py-2.5 text-left text-[14px] font-medium transition ${
                  selected ? "bg-primary/10 text-text-main" : "text-text-main/82 hover:bg-surface-container-high"
                }`}
                key={dealType}
                onClick={() => {
                  onChange(dealType);
                  setOpen(false);
                }}
                role="option"
                type="button"
              >
                <span>{dealType}</span>
                {selected ? <Icon className="h-4 w-4 text-primary" name="check" /> : null}
              </button>
            );
          })}
        </div>
      ) : null}
      {error ? <p className="mt-2 px-1 text-[12px] font-medium text-error">{error}</p> : null}
    </div>
  );
}
