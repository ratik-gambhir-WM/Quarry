import { useEffect, useRef, useState } from "react";
import { Icon } from "../ui/Icon";

const newAnalysisOptions = [
  { action: "upload", icon: "upload", label: "Upload New File" },
  { action: "connect-sharepoint", icon: "sharepoint", label: "Connect to SharePoint" },
  { action: "create-note", icon: "doc", label: "Create Note" },
  { action: "create-graph", icon: "graph", label: "Create Graph" },
] as const;

type NewAnalysisMenuProps = {
  onConnectToSharePoint: () => void;
  onUploadNewFile: () => void;
};

export function NewAnalysisMenu({ onConnectToSharePoint, onUploadNewFile }: NewAnalysisMenuProps) {
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
        className="flex h-9 w-full items-center justify-center gap-3 rounded-lg bg-[#0c006b] px-3 text-white shadow-sm transition enabled:hover:bg-[#211781] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-not-allowed disabled:opacity-50 [html[data-theme=dark]_&]:bg-white [html[data-theme=dark]_&]:text-[#0c006b] [html[data-theme=dark]_&]:enabled:hover:bg-[#f1eff8]"
        onClick={() => setOpen((current) => !current)}
        title="New analysis"
        type="button"
      >
        <Icon className="h-4 w-4" name="plus" />
        <span className="text-[13px] font-medium leading-5">New analysis</span>
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
                if (option.action === "upload") {
                  onUploadNewFile();
                } else if (option.action === "connect-sharepoint") {
                  onConnectToSharePoint();
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
