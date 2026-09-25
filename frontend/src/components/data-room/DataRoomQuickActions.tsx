import { useRef, useState } from "react";
import { XIcon } from "lucide-react";
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
import { DocumentMagnifyingGlassIcon } from "../ui/document-magnifying-glass";
import { PencilSquareIcon } from "../ui/pencil-square";
import { Textarea } from "../ui/textarea";
import { UploadIcon } from "../ui/upload";
import {
  DataRoomDocumentSearch,
  type DataRoomDocumentSearchProps,
} from "./document-search/DataRoomDocumentSearch";

type DataRoomQuickActionsProps = {
  documentSearch: Omit<DataRoomDocumentSearchProps, "finalFocusEl" | "trigger">;
  onUploadNewFile: () => void;
};

const SYNTHESIS_PANEL_DEFAULT_SIZE = { width: 360, height: 450 };
const SYNTHESIS_PANEL_DEFAULT_INSET = { right: 72, bottom: 48 };
const SYNTHESIS_PANEL_MIN_SIZE = { width: 280, height: 220 };
const SIDEBAR_ACTION_CLASSNAME =
  "text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-active";

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

export function DataRoomQuickActions({
  documentSearch,
  onUploadNewFile,
}: DataRoomQuickActionsProps) {
  const [synthesisPanelOpen, setSynthesisPanelOpen] = useState(false);
  const [synthesisDraft, setSynthesisDraft] = useState("");
  const [synthesisPanelSize] = useState(getInitialSynthesisPanelSize);
  const synthesisTriggerRef = useRef<HTMLButtonElement | null>(null);
  const synthesisTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  const searchTriggerRef = useRef<HTMLButtonElement | null>(null);

  return (
    <div
      aria-label="Data room tools"
      className="grid grid-cols-4 items-center px-2"
      role="toolbar"
    >
      <Button
        aria-label="Upload files"
        className={`${SIDEBAR_ACTION_CLASSNAME} justify-self-center`}
        onClick={onUploadNewFile}
        size="icon-lg"
        title="Upload files"
        type="button"
        variant="ghost"
      >
        <UploadIcon aria-hidden="true" className="size-4" size={16} />
      </Button>

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
        getBoundaryEl={() => documentSearch.boundaryElement ?? document.documentElement}
        id="data-room-synthesis-canvas"
        finalFocusEl={() => synthesisTriggerRef.current}
        initialFocusEl={() => synthesisTextareaRef.current}
        minSize={{
          width: Math.min(SYNTHESIS_PANEL_MIN_SIZE.width, synthesisPanelSize.width),
          height: Math.min(SYNTHESIS_PANEL_MIN_SIZE.height, synthesisPanelSize.height),
        }}
        onOpenChange={({ open }) => setSynthesisPanelOpen(open)}
        open={synthesisPanelOpen}
      >
        <FloatingPanelTrigger asChild>
          <Button
            aria-label="Open Synthesis Canvas"
            className={`${SIDEBAR_ACTION_CLASSNAME} justify-self-center`}
            ref={synthesisTriggerRef}
            size="icon-lg"
            title="Synthesis Canvas"
            type="button"
            variant="ghost"
          >
            <PencilSquareIcon aria-hidden="true" className="size-4" size={16} />
          </Button>
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

      <Button
        aria-label="Notes"
        className={`${SIDEBAR_ACTION_CLASSNAME} justify-self-center`}
        size="icon-lg"
        title="Notes"
        type="button"
        variant="ghost"
      >
        <Icon aria-hidden="true" className="size-4" name="doc" />
      </Button>

      <DataRoomDocumentSearch
        {...documentSearch}
        finalFocusEl={() => searchTriggerRef.current}
        trigger={
          <Button
            aria-label="Search document"
            className={`${SIDEBAR_ACTION_CLASSNAME} justify-self-center`}
            ref={searchTriggerRef}
            size="icon-lg"
            title="Search document"
            type="button"
            variant="ghost"
          >
            <DocumentMagnifyingGlassIcon aria-hidden="true" className="size-4" size={16} />
          </Button>
        }
      />
    </div>
  );
}
