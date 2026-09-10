import { useRef, useState } from "react";
import { XIcon } from "lucide-react";
import { cn } from "../../lib/utils";
import { ArcMenu, ArcMenuAction } from "../ui/arc-menu";
import { Button } from "../ui/button";
import {
  FloatingPanel,
  FloatingPanelBody,
  FloatingPanelCloseTrigger,
  FloatingPanelContent,
  FloatingPanelControl,
  FloatingPanelHeader,
  FloatingPanelTitle,
  FloatingPanelTrigger,
} from "../ui/floating-panel";
import { Icon } from "../ui/Icon";
import { PaperAirplaneIcon } from "../ui/paper-airplane";
import { Textarea } from "../ui/textarea";
import {
  DataRoomDocumentSearch,
  type DataRoomDocumentSearchProps,
} from "./document-search/DataRoomDocumentSearch";

type DataRoomArcMenuProps = {
  documentSearch: Omit<DataRoomDocumentSearchProps, "finalFocusEl" | "trigger">;
  documentSearchOpen?: boolean;
};

const SYNTHESIS_PANEL_DEFAULT_SIZE = { width: 360, height: 450 };
const SYNTHESIS_PANEL_DEFAULT_INSET = { right: 72, bottom: 48 };
const SYNTHESIS_PANEL_MIN_SIZE = { width: 280, height: 220 };
const CONTROL_SCALE = 0.7;
const ARC_MENU_TRIGGER_SIZE = 56 * CONTROL_SCALE;
const ARC_MENU_ICON_ONLY_TRIGGER_SIZE = 48 * CONTROL_SCALE;
const ARC_MENU_OPEN_TRIGGER_SIZE = 40 * CONTROL_SCALE;
const ARC_MENU_TRIGGER_ICON_SIZE = 20 * CONTROL_SCALE;
const ARC_MENU_CLOSE_ICON_SIZE = 28 * CONTROL_SCALE;

function getInitialSynthesisPanelSize() {
  if (typeof window === "undefined") {
    return SYNTHESIS_PANEL_DEFAULT_SIZE;
  }

  return {
    width: Math.min(
      SYNTHESIS_PANEL_DEFAULT_SIZE.width,
      Math.max(window.innerWidth - 32, 1),
    ),
    height: Math.min(
      SYNTHESIS_PANEL_DEFAULT_SIZE.height,
      Math.max(window.innerHeight - 32, 1),
    ),
  };
}

export function DataRoomArcMenu({
  documentSearch,
  documentSearchOpen = false,
}: DataRoomArcMenuProps) {
  const [hidden, setHidden] = useState(false);
  const [open, setOpen] = useState(false);
  const [synthesisPanelOpen, setSynthesisPanelOpen] = useState(false);
  const [synthesisDraft, setSynthesisDraft] = useState("");
  const [synthesisPanelSize] = useState(getInitialSynthesisPanelSize);
  const arcMenuRef = useRef<HTMLDivElement | null>(null);
  const synthesisTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const overlayOpen = documentSearchOpen || synthesisPanelOpen;
  const getArcMenuTrigger = () =>
    arcMenuRef.current?.querySelector<HTMLButtonElement>(
      '[data-slot="arc-menu-trigger"]',
    ) ?? null;

  const handleOpenChange = (nextOpen: boolean) => {
    if (hidden && nextOpen) {
      setHidden(false);
    }
    setOpen(nextOpen);
  };

  const toggleHidden = () => {
    if (overlayOpen) {
      return;
    }

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
        hidden ? "-bottom-[0.7rem]" : "bottom-0",
      )}
    >
      {hidden ? null : (
        <ArcMenu
          ref={arcMenuRef}
          classNames={{
            caption: "text-[8.4px]",
            closeIcon: "text-on-action",
            triggerFace: "text-on-action",
            triggerSurface:
              "bg-action group-hover/button:bg-action-hover group-active/button:bg-action-hover",
          }}
          closeIconSize={ARC_MENU_CLOSE_ICON_SIZE}
          iconOnlyTriggerSize={ARC_MENU_ICON_ONLY_TRIGGER_SIZE}
          menuLabel="Data room views"
          onOpenChange={handleOpenChange}
          openTriggerSize={ARC_MENU_OPEN_TRIGGER_SIZE}
          open={open}
          triggerCaption="Views"
          triggerIconSize={ARC_MENU_TRIGGER_ICON_SIZE}
          triggerLabel="Open data room views"
          triggerSize={ARC_MENU_TRIGGER_SIZE}
        >
          <ArcMenuAction disabled icon={<Icon name="dataset" />} label="Data Room" />
          <ArcMenuAction disabled icon={<Icon name="graph" />} label="Diligence Graph" />
          <FloatingPanel
            allowOverflow={false}
            closeOnEscape
            defaultSize={synthesisPanelSize}
            getAnchorPosition={({ boundaryRect }) => {
              const boundaryX = boundaryRect?.x ?? 0;
              const boundaryY = boundaryRect?.y ?? 0;
              const boundaryWidth = boundaryRect?.width ?? window.innerWidth;
              const boundaryHeight = boundaryRect?.height ?? window.innerHeight;

              return {
                x: Math.max(
                  boundaryX,
                  boundaryX +
                    boundaryWidth -
                    synthesisPanelSize.width -
                    SYNTHESIS_PANEL_DEFAULT_INSET.right,
                ),
                y: Math.max(
                  boundaryY,
                  boundaryY +
                    boundaryHeight -
                    synthesisPanelSize.height -
                    SYNTHESIS_PANEL_DEFAULT_INSET.bottom,
                ),
              };
            }}
            id="data-room-synthesis-canvas"
            finalFocusEl={getArcMenuTrigger}
            initialFocusEl={() => synthesisTextareaRef.current}
            minSize={{
              width: Math.min(SYNTHESIS_PANEL_MIN_SIZE.width, synthesisPanelSize.width),
              height: Math.min(SYNTHESIS_PANEL_MIN_SIZE.height, synthesisPanelSize.height),
            }}
            onOpenChange={({ open: nextOpen }) => setSynthesisPanelOpen(nextOpen)}
            open={synthesisPanelOpen}
          >
            <FloatingPanelTrigger asChild>
              <ArcMenuAction
                icon={<PaperAirplaneIcon className="size-full" size={16} />}
                label="Synthesis Canvas"
              />
            </FloatingPanelTrigger>
            <FloatingPanelContent>
              <FloatingPanelHeader>
                <FloatingPanelTitle>Synthesis Canvas</FloatingPanelTitle>
                <FloatingPanelControl>
                  <FloatingPanelCloseTrigger asChild>
                    <Button
                      aria-label="Close Synthesis Canvas"
                      size="icon-xs"
                      type="button"
                      variant="ghost"
                    >
                      <XIcon aria-hidden="true" />
                    </Button>
                  </FloatingPanelCloseTrigger>
                </FloatingPanelControl>
              </FloatingPanelHeader>
              <FloatingPanelBody>
                <label
                  className="text-sm font-medium text-text-main"
                  htmlFor="synthesis-canvas-notes"
                >
                  Notes
                </label>
                <Textarea
                  ref={synthesisTextareaRef}
                  className="min-h-32 flex-1 resize-none"
                  id="synthesis-canvas-notes"
                  onChange={(event) => setSynthesisDraft(event.target.value)}
                  placeholder="Capture ideas for the synthesis canvas"
                  value={synthesisDraft}
                />
              </FloatingPanelBody>
            </FloatingPanelContent>
          </FloatingPanel>
          <ArcMenuAction icon={<Icon name="doc" />} label="Notes" />
          <DataRoomDocumentSearch
            {...documentSearch}
            finalFocusEl={getArcMenuTrigger}
            trigger={<ArcMenuAction icon={<Icon name="search" />} label="Search document" />}
          />
        </ArcMenu>
      )}
      <button
        aria-label={hidden ? "Show data room views" : "Hide data room views"}
        className={cn(
          "grid place-items-center border border-outline-variant bg-background/95 text-muted-foreground shadow-sm backdrop-blur-md transition-[width,height,border-radius,background-color,color] duration-200 hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring motion-reduce:transition-colors",
          hidden
            ? "h-[2.45rem] w-[4.9rem] rounded-t-[1.4rem] rounded-b-none border-b-0 pb-[0.525rem]"
            : "mt-[0.175rem] h-[1.05rem] w-[2.8rem] rounded-full",
        )}
        onClick={toggleHidden}
        title={hidden ? "Show data room views" : "Hide data room views"}
        type="button"
      >
        <Icon
          className={hidden ? "size-[0.7rem]" : "size-3.5"}
          name={hidden ? "grid" : "chevronDown"}
        />
      </button>
    </div>
  );
}
