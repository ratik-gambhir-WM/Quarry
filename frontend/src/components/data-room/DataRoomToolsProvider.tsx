import {
  createContext,
  lazy,
  Suspense,
  useContext,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { WestMonroeMark } from "../brand/WestMonroeMark";
import { SpeedDial, SpeedDialAction } from "../ui/speed-dial";
import { AssistantIcon, WorkspaceSearchIcon } from "../ui/workspace-tool-icons";
import type { DataRoomDocumentSearchProps } from "./document-search/DataRoomDocumentSearch";

const DataRoomToolPanels = lazy(() => import("./DataRoomToolPanels"));
export type DocumentSearchContext = Omit<
  DataRoomDocumentSearchProps,
  "trigger" | "finalFocusEl" | "open" | "onOpenChange"
>;

// The selected viewer registers its live page/focus capabilities. The shell subscribes
// without copying document state or retaining callbacks after the viewer unmounts.
function createSearchRegistry() {
  let snapshot: DocumentSearchContext | null = null;
  const listeners = new Set<() => void>();
  return {
    getSnapshot: () => snapshot,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    register(value: DocumentSearchContext) {
      snapshot = value;
      listeners.forEach((listener) => listener());
      return () => {
        if (snapshot !== value) return;
        snapshot = null;
        listeners.forEach((listener) => listener());
      };
    },
  };
}

type Tool = "assistant" | "search";
const ToolsContext = createContext<{
  launch: (tool: Tool, trigger: HTMLButtonElement | null) => void;
  registry: ReturnType<typeof createSearchRegistry>;
} | null>(null);

export function DataRoomToolsProvider({ children, dealId, email }: {
  children: ReactNode;
  dealId: string;
  email?: string;
}) {
  const [registry] = useState(createSearchRegistry);
  const search = useSyncExternalStore(
    registry.subscribe, registry.getSnapshot, registry.getSnapshot,
  );
  const [panel, setPanel] = useState<Tool | null>(null);
  const [hasOpened, setHasOpened] = useState(false);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  function launch(tool: Tool, trigger: HTMLButtonElement | null) {
    triggerRef.current = trigger;
    setHasOpened(true);
    setPanel(tool);
  }
  return (
    <ToolsContext.Provider value={{ launch, registry }}>
      {children}
      <div className="fixed bottom-4 right-4 z-50 lg:hidden">
        <DataRoomToolsDial />
      </div>
      {hasOpened ? (
        <Suspense fallback={<span className="sr-only" role="status">Loading Data Room tools…</span>}>
          <DataRoomToolPanels
            dealId={dealId}
            email={email}
            finalFocusEl={() => triggerRef.current?.isConnected ? triggerRef.current : null}
            onAssistantOpenChange={(open) => setPanel(open ? "assistant" : null)}
            onSearchOpenChange={(open) => setPanel(open ? "search" : null)}
            panel={panel}
            search={search}
          />
        </Suspense>
      ) : null}
    </ToolsContext.Provider>
  );
}

export function useDataRoomDocumentSearch(value: DocumentSearchContext | null) {
  const registry = useContext(ToolsContext)?.registry;
  useEffect(() => {
    if (value && registry) return registry.register(value);
  }, [registry, value]);
}

export function DataRoomToolsDial() {
  const tools = useContext(ToolsContext);
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const actionChosen = useRef(false);
  if (!tools) {
    return <span aria-label="Quarry" role="img"><WestMonroeMark className="h-7 w-7" framed /></span>;
  }
  return (
    <SpeedDial
      label="Data Room tools"
      icon={<WestMonroeMark className="h-7 w-7" framed />}
      placement="inline"
      scrim={false}
      triggerRef={triggerRef}
      restoreFocus={() => !actionChosen.current}
      onOpenChange={(open) => { if (open) actionChosen.current = false; }}
    >
      <SpeedDialAction icon={<AssistantIcon className="size-5" />} onClick={() => {
        actionChosen.current = true;
        tools.launch("assistant", triggerRef.current);
      }}>Assistant</SpeedDialAction>
      <SpeedDialAction icon={<WorkspaceSearchIcon className="size-5" />} onClick={() => {
        actionChosen.current = true;
        tools.launch("search", triggerRef.current);
      }}>Workspace Search</SpeedDialAction>
    </SpeedDial>
  );
}
