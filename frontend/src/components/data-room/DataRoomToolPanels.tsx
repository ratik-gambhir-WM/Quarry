import { useRef, useState } from "react";
import { CheckIcon, ChevronDownIcon, PlusIcon, XIcon } from "lucide-react";
import {
  useAgentRuntimeContext,
  useThreadRuntimeState,
} from "../chat/AgentRuntime";
import { QueryChatRuntimeProvider } from "../chat/QueryChatRuntimeProvider";
import { QueryChatThread } from "../chat/QueryChatThread";
import { Button } from "../ui/button";
import { getFloatingPanelInitialPosition } from "../ui/floating-panel-position";
import {
  FloatingPanel,
  FloatingPanelCloseTrigger,
  FloatingPanelContent,
  FloatingPanelControl,
  FloatingPanelHeader,
  FloatingPanelTitle,
} from "../ui/floating-panel";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu";
import { DataRoomDocumentSearch } from "./document-search/DataRoomDocumentSearch";
import DocumentSearch from "./document-search/DocumentSearch";
import type { DocumentSearchContext } from "./DataRoomToolsProvider";

const SYNTHESIS_PANEL_DEFAULT_SIZE = { width: 520, height: 512 };
const SYNTHESIS_PANEL_DEFAULT_INSET = { right: 72, bottom: 48 };
const SYNTHESIS_PANEL_MIN_SIZE = { width: 360, height: 360 };

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

export default function DataRoomToolPanels({
  search,
  dealId,
  email,
  panel,
  finalFocusEl,
  onAssistantOpenChange,
  onSearchOpenChange,
}: {
  search: DocumentSearchContext | null;
  dealId: string;
  email?: string;
  panel: "assistant" | "search" | null;
  finalFocusEl: () => HTMLElement | null;
  onAssistantOpenChange: (open: boolean) => void;
  onSearchOpenChange: (open: boolean) => void;
}) {
  const [contextTruncated, setContextTruncated] = useState(false);
  const [synthesisPanelSize] = useState(getInitialSynthesisPanelSize);
  const synthesisChatRef = useRef<HTMLDivElement | null>(null);

  return (
    <>
      <FloatingPanel
        allowOverflow={false}
        closeOnEscape
        defaultSize={synthesisPanelSize}
        defaultPosition={getFloatingPanelInitialPosition(
          synthesisPanelSize, search?.boundaryElement, SYNTHESIS_PANEL_DEFAULT_INSET,
        )}
        getBoundaryEl={() =>
          search?.boundaryElement ?? document.documentElement
        }
        id="data-room-synthesis-canvas"
        finalFocusEl={finalFocusEl}
        initialFocusEl={() =>
          synthesisChatRef.current?.querySelector("textarea") ?? null
        }
        minSize={{
          width: Math.min(
            SYNTHESIS_PANEL_MIN_SIZE.width,
            synthesisPanelSize.width,
          ),
          height: Math.min(
            SYNTHESIS_PANEL_MIN_SIZE.height,
            synthesisPanelSize.height,
          ),
        }}
        onOpenChange={({ open }) => onAssistantOpenChange(open)}
        open={panel === "assistant"}
        unmountOnExit={false}
      >
        <FloatingPanelContent>
          <QueryChatRuntimeProvider
            dealId={dealId}
            scope="deal"
            key={`${email ?? ""}:${dealId}`}
            onContextTruncated={setContextTruncated}
            userEmail={email ?? ""}
          >
            <FloatingPanelHeader>
              <FloatingPanelTitle>Assistant</FloatingPanelTitle>
              <SynthesisChatPicker />
              <FloatingPanelControl>
                <FloatingPanelCloseTrigger asChild>
                  <Button
                    aria-label="Close Assistant"
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
              <QueryChatThread
                contextTruncated={contextTruncated}
                floatingPanel
              />
            </div>
          </QueryChatRuntimeProvider>
        </FloatingPanelContent>
      </FloatingPanel>

      {search ? (
        <DataRoomDocumentSearch
          {...search}
          dialogTitle="Workspace Search"
          finalFocusEl={finalFocusEl}
          open={panel === "search"}
          onOpenChange={onSearchOpenChange}
          hideTrigger
        />
      ) : (
        <DocumentSearch
          dialogTitle="Workspace Search"
          dialogDescription="Open a document in the Data Room to search its sample excerpts. Workspace-wide search is not connected yet."
          showDescription
          finalFocusEl={finalFocusEl}
          items={[]}
          onSelect={() => {}}
          open={panel === "search"}
          onOpenChange={onSearchOpenChange}
          hideTrigger
        />
      )}
    </>
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
  const selectedThread = threads.find(
    (thread) => thread.threadId === selectedThreadId,
  );
  const actionsDisabled = isPreparingThread || isRunning;

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <Button
            aria-label="Select previous chat"
            className="max-w-44 gap-1.5 px-2 text-xs font-medium"
            size="sm"
            title="Select previous chat"
            type="button"
            variant="ghost"
          >
            <span className="truncate">
              {selectedThread?.title || "New chat"}
            </span>
            <ChevronDownIcon aria-hidden="true" className="size-3.5 shrink-0" />
          </Button>
        }
      />
      <DropdownMenuContent align="end" className="z-[60] w-64">
        <DropdownMenuItem
          disabled={actionsDisabled}
          onSelect={() => runtime.newChat()}
        >
          <PlusIcon />
          New chat
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>Previous chats</DropdownMenuLabel>
        {isThreadListLoading ? (
          <DropdownMenuLabel>Loading chats…</DropdownMenuLabel>
        ) : null}
        {threadListError ? (
          <DropdownMenuLabel className="text-error">
            Couldn’t load previous chats.
          </DropdownMenuLabel>
        ) : null}
        {!isThreadListLoading && !threadListError && threads.length === 0 ? (
          <DropdownMenuLabel>No previous chats yet.</DropdownMenuLabel>
        ) : null}
        {runtime.state.nextCursor ? (
          <DropdownMenuItem
            disabled={isThreadListLoading}
            onSelect={() => void runtime.loadMoreThreads()}
          >
            Load more chats
          </DropdownMenuItem>
        ) : null}
        {threads.map((thread) => (
          <DropdownMenuItem
            disabled={actionsDisabled || selectedThreadId === thread.threadId}
            key={thread.threadId}
            onSelect={() => void runtime.selectThread(thread.threadId)}
          >
            {selectedThreadId === thread.threadId ? (
              <CheckIcon aria-hidden="true" />
            ) : (
              <span className="size-4" />
            )}
            <span className="truncate">{thread.title || "New chat"}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
