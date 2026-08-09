import { ArrowEndOnRectangleIcon } from "../../ui/icons/ArrowEndOnRectangleIcon";
import { ViewColumnsIcon } from "../../ui/icons/ViewColumnsIcon";

type ChipBankHeaderProps = {
  onCollapse: () => void;
};

export function ChipBankHeader({ onCollapse }: ChipBankHeaderProps) {
  return (
    <div className="flex w-full items-center justify-between">
      <div className="flex min-w-0 items-center gap-3">
        <button
          aria-label="Collapse document search"
          className="shrink-0 rounded-full p-1 text-accent transition hover:bg-surface-container-high hover:text-text-main"
          onClick={onCollapse}
          title="Collapse document search"
          type="button"
        >
          <ArrowEndOnRectangleIcon className="h-7 w-7" direction="right" />
        </button>
        <div className="min-w-0">
          <h3 className="truncate text-[1rem] font-bold leading-tight text-text-main [font-family:var(--font-heading)]">
            Document Search
          </h3>
          <p className="mt-0.5 text-[11px] font-semibold uppercase tracking-[0.14em] text-muted">
            Search repository
          </p>
        </div>
      </div>
      <button
        aria-label="Document search view options"
        className="group shrink-0 rounded-full p-2 text-muted transition hover:bg-surface-container-high hover:text-text-main"
        title="Document search view options"
        type="button"
      >
        <ViewColumnsIcon className="h-6 w-6" />
      </button>
    </div>
  );
}
