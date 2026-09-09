export const quarryDataGridHeaderClassName =
  "text-[13px] font-normal tracking-normal text-text-main";

export const quarryDataGridTableClassNames = {
  base:
    "bg-surface-container-lowest text-[13px] text-on-surface [&_th]:border-outline-variant/70 [&_td]:border-outline-variant/70 [&_tbody_td]:px-4 [&_tbody_td]:py-3",
  bodyRow:
    "transition-colors hover:bg-[var(--theme-workspace-chrome)] focus-within:bg-[var(--theme-workspace-chrome)]",
  edgeCell: "px-5 data-pinned:bg-surface-container-lowest",
  header: "bg-surface-container-lowest",
  headerRow: "border-outline-variant/70 bg-surface-container-lowest",
  headerSticky: "sticky top-0 z-40 bg-surface-container-lowest/95 backdrop-blur-xs",
} as const;
