import { useState } from "react";
import { cn } from "../../lib/utils";
import { ArcMenu, ArcMenuAction } from "../ui/arc-menu";
import { Icon } from "../ui/Icon";
import {
  DataRoomDocumentSearch,
  type DataRoomDocumentSearchProps,
} from "./document-search/DataRoomDocumentSearch";

type DataRoomArcMenuProps = {
  documentSearch: Omit<DataRoomDocumentSearchProps, "trigger">;
  documentSearchOpen?: boolean;
};

export function DataRoomArcMenu({
  documentSearch,
  documentSearchOpen = false,
}: DataRoomArcMenuProps) {
  const [hidden, setHidden] = useState(false);
  const [open, setOpen] = useState(false);

  const handleOpenChange = (nextOpen: boolean) => {
    if (documentSearchOpen && !nextOpen) {
      return;
    }
    if (hidden && nextOpen) {
      setHidden(false);
    }
    setOpen(nextOpen);
  };

  const toggleHidden = () => {
    if (hidden) {
      setHidden(false);
      setOpen(true);
      return;
    }

    setOpen(false);
    setHidden(true);
  };

  return (
    <div
      className={cn(
        "pointer-events-none absolute left-1/2 z-20 flex -translate-x-1/2 flex-col items-center transition-[bottom] duration-200 ease-out motion-reduce:transition-none [&>*]:pointer-events-auto",
        hidden ? "-bottom-4" : "bottom-0",
      )}
    >
      {hidden ? null : (
        <ArcMenu
          classNames={{
            closeIcon: "text-on-action",
            triggerFace: "text-on-action",
            triggerSurface:
              "bg-action group-hover/button:bg-action-hover group-active/button:bg-action-hover",
          }}
          closeOnAction={false}
          menuLabel="Data room views"
          onOpenChange={handleOpenChange}
          open={open}
          triggerCaption="Views"
          triggerLabel="Open data room views"
        >
          <ArcMenuAction disabled icon={<Icon name="dataset" />} label="Data Room" />
          <ArcMenuAction disabled icon={<Icon name="graph" />} label="Diligence Graph" />
          <ArcMenuAction disabled icon={<Icon name="grid" />} label="Synthesis Canvas" />
          <ArcMenuAction icon={<Icon name="doc" />} label="Notes" />
          <DataRoomDocumentSearch
            {...documentSearch}
            trigger={<ArcMenuAction icon={<Icon name="search" />} label="Search document" />}
          />
        </ArcMenu>
      )}
      <button
        aria-label={hidden ? "Show data room views" : "Hide data room views"}
        className={cn(
          "grid place-items-center border border-outline-variant bg-background/95 text-muted-foreground shadow-sm backdrop-blur-md transition-[width,height,border-radius,background-color,color] duration-200 hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-colors",
          hidden
            ? "h-14 w-28 rounded-t-[2rem] rounded-b-none border-b-0 pb-3"
            : "mt-1 h-6 w-16 rounded-full",
        )}
        onClick={toggleHidden}
        title={hidden ? "Show data room views" : "Hide data room views"}
        type="button"
      >
        <Icon className={hidden ? "h-4 w-4" : "h-5 w-5"} name={hidden ? "grid" : "chevronDown"} />
      </button>
    </div>
  );
}
