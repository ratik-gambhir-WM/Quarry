import { useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon, PlusIcon, XIcon } from "lucide-react";
import { useAgentRuntimeContext, useThreadRuntimeState } from "../chat/AgentRuntime";
import { QueryChatRuntimeProvider } from "../chat/QueryChatRuntimeProvider";
import { QueryChatThread } from "../chat/QueryChatThread";
import { Button } from "../ui/button";
import {
  FloatingPanel,
  FloatingPanelCloseTrigger,
  FloatingPanelContent,
  FloatingPanelControl,
  FloatingPanelHeader,
  FloatingPanelTitle,
  FloatingPanelTrigger,
} from "../ui/floating-panel";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { Icon } from "../ui/Icon";
import { DocumentMagnifyingGlassIcon } from "../ui/document-magnifying-glass";
import { PencilSquareIcon } from "../ui/pencil-square";
import { UploadIcon } from "../ui/upload";
import {
  DataRoomDocumentSearch,
  type DataRoomDocumentSearchProps,
} from "./document-search/DataRoomDocumentSearch";

type DataRoomQuickActionsProps = {
  documentSearch: Omit<DataRoomDocumentSearchProps, "finalFocusEl" | "trigger">;
  email?: string;
  onUploadNewFile: () => void;
};

const SYNTHESIS_PANEL_DEFAULT_SIZE = { width: 520, height: 512 };
const SYNTHESIS_PANEL_DEFAULT_INSET = { right: 72, bottom: 48 };
const SYNTHESIS_PANEL_MIN_SIZE = { width: 360, height: 360 };
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
  email,
  onUploadNewFile,
}: DataRoomQuickActionsProps) {
  const [contextTruncated, setContextTruncated] = useState(false);
  const [synthesisPanelOpen, setSynthesisPanelOpen] = useState(false);
  const [synthesisPanelSize] = useState(getInitialSynthesisPanelSize);
  const synthesisChatRef = useRef<HTMLDivElement | null>(null);
  const synthesisTriggerRef = useRef<HTMLButtonElement | null>(null);
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
        initialFocusEl={() => synthesisChatRef.current?.querySelector("textarea") ?? null}
        minSize={{
          width: Math.min(SYNTHESIS_PANEL_MIN_SIZE.width, synthesisPanelSize.width),
          height: Math.min(SYNTHESIS_PANEL_MIN_SIZE.height, synthesisPanelSize.height),
        }}
        onOpenChange={({ open }) => setSynthesisPanelOpen(open)}
        open={synthesisPanelOpen}
        unmountOnExit={false}
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
          <QueryChatRuntimeProvider key={email ?? ""} onContextTruncated={setContextTruncated} userEmail={email ?? ""}>
            <FloatingPanelHeader>
              <FloatingPanelTitle>Synthesis Canvas</FloatingPanelTitle>
              <SynthesisChatPicker />
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
            <div className="min-h-0 flex-1" ref={synthesisChatRef}>
              <QueryChatThread contextTruncated={contextTruncated} floatingPanel />
            </div>
          </QueryChatRuntimeProvider>
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

function SynthesisChatPicker() {
  const runtime = useAgentRuntimeContext();
  const {
    isPreparingThread,
    isRunning,
    isThreadListLoading,
    selectedThreadId,
    threadListError,
    threads,
  } = useThreadRuntimeState((state) => state);
  const selectedThread = threads.find((thread) => thread.threadId === selectedThreadId);
  const actionsDisabled = isPreparingThread || isRunning;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={(
          <Button
            aria-label="Select previous chat"
            className="max-w-44 gap-1.5 px-2 text-xs font-medium"
            size="sm"
            title="Select previous chat"
            type="button"
            variant="ghost"
          >
            <span className="truncate">{selectedThread?.title || "New chat"}</span>
            <ChevronDownIcon aria-hidden="true" className="size-3.5 shrink-0" />
          </Button>
        )}
      />
      <DropdownMenuContent align="end" className="z-[60] w-64">
        <DropdownMenuItem disabled={actionsDisabled} onSelect={() => runtime.newChat()}>
          <PlusIcon />
          New chat
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>Previous chats</DropdownMenuLabel>
        {isThreadListLoading ? <DropdownMenuLabel>Loading chats…</DropdownMenuLabel> : null}
        {threadListError ? <DropdownMenuLabel className="text-error">Couldn’t load previous chats.</DropdownMenuLabel> : null}
        {!isThreadListLoading && !threadListError && threads.length === 0 ? (
          <DropdownMenuLabel>No previous chats yet.</DropdownMenuLabel>
        ) : null}
        {threads.map((thread) => (
          <DropdownMenuItem
            disabled={actionsDisabled || selectedThreadId === thread.threadId}
            key={thread.threadId}
            onSelect={() => void runtime.selectThread(thread.threadId)}
          >
            {selectedThreadId === thread.threadId ? <CheckIcon aria-hidden="true" /> : <span className="size-4" />}
            <span className="truncate">{thread.title || "New chat"}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
