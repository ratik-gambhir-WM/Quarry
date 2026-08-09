import { useEffect, useRef, useState } from "react";
import { Icon } from "../ui/Icon";

const newAnalysisOptions = [
  { icon: "upload", label: "Upload New File" },
  { icon: "doc", label: "Create Note" },
  { icon: "graph", label: "Create Graph" },
] as const;

export function NewAnalysisMenu({ onUploadNewFile }: { onUploadNewFile: () => void }) {
  const [open, setOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    function handlePointerDown(event: PointerEvent) {
      if (!menuRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setOpen(false);
      }
    }

    if (open) {
      document.addEventListener("pointerdown", handlePointerDown);
      document.addEventListener("keydown", handleKeyDown);
    }

    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  return (
    <div className="relative z-40 flex justify-center" ref={menuRef}>
      <button
        aria-expanded={open}
        aria-haspopup="menu"
        aria-label="New analysis"
        className="flex h-14 w-full items-center justify-center rounded-full bg-[#0c006b] text-white shadow-sm transition enabled:hover:bg-[#211781] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-not-allowed disabled:opacity-50 [html[data-theme=dark]_&]:bg-white [html[data-theme=dark]_&]:text-[#0c006b] [html[data-theme=dark]_&]:enabled:hover:bg-[#f1eff8]"
        onClick={() => setOpen((current) => !current)}
        title="New analysis"
        type="button"
      >
        <Icon className="h-6 w-6" name="plus" />
      </button>

      {open ? (
        <div
          className="absolute left-1/2 top-full mt-2 w-52 -translate-x-1/2 overflow-hidden rounded-lg border border-outline-variant bg-surface-container-lowest py-1 shadow-xl"
          role="menu"
        >
          {newAnalysisOptions.map((option) => (
            <button
              className="flex w-full items-center gap-3 px-4 py-3 text-left text-sm font-medium text-text-main transition hover:bg-surface-container-high"
              key={option.label}
              onClick={() => {
                setOpen(false);
                if (option.label === "Upload New File") {
                  onUploadNewFile();
                }
              }}
              role="menuitem"
              type="button"
            >
              <Icon className="h-5 w-5 shrink-0 text-muted" name={option.icon} />
              <span>{option.label}</span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
