import { ArrowEndOnRectangleIcon } from "../ui/icons/ArrowEndOnRectangleIcon";

type EdgePanelOpenButtonProps = {
  label: string;
  onClick: () => void;
  side: "left" | "right";
};

export function EdgePanelOpenButton({ label, onClick, side }: EdgePanelOpenButtonProps) {
  const sideClasses = side === "left" ? "left-1" : "right-1";
  const direction = side === "left" ? "right" : "left";

  return (
    <button
      aria-label={label}
      className={`absolute top-1/2 z-30 flex h-12 w-11 -translate-y-1/2 items-center justify-center bg-transparent text-primary transition hover:text-text-main ${sideClasses}`}
      onClick={onClick}
      title={label}
      type="button"
    >
      <ArrowEndOnRectangleIcon className="h-7 w-7" direction={direction} />
    </button>
  );
}
