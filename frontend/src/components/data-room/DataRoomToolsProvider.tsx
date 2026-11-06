import {
  lazy,
  Suspense,
  useContext,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { WestMonroeMark } from "../brand/WestMonroeMark";
import { SpeedDial, SpeedDialAction } from "../ui/speed-dial";
import { AssistantIcon, WorkspaceSearchIcon } from "../ui/workspace-tool-icons";
import { createSearchRegistry, ToolsContext, type DataRoomTool } from "./dataRoomToolsContext";

const DataRoomToolPanels = lazy(() => import("./DataRoomToolPanels"));
export function DataRoomToolsProvider({ active = true, children, dealId, email }: {
  active?: boolean;
  children: ReactNode;
  dealId: string;
  email?: string;
}) {
  // A new route session discards tool state without remounting the workspace shell.
  const session = useMemo(() => ({}), [active]);
  const registry = useMemo(createSearchRegistry, [session]);
  const search = useSyncExternalStore(
    registry.subscribe, registry.getSnapshot, registry.getSnapshot,
  );
  const [panelState, setPanelState] = useState<{
    session: object;
    panel: DataRoomTool | null;
  } | null>(null);
  const hasOpened = active && panelState?.session === session;
  const panel = hasOpened ? panelState.panel : null;
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  function launch(tool: DataRoomTool, trigger: HTMLButtonElement | null) {
    triggerRef.current = trigger;
    setPanelState({ session, panel: tool });
  }
  return (
    <ToolsContext.Provider value={active ? { launch, registry } : null}>
      {children}
      {active ? (
        <div className="fixed bottom-4 right-4 z-50 lg:hidden">
          <DataRoomToolsDial />
        </div>
      ) : null}
      {hasOpened ? (
        <Suspense fallback={<span className="sr-only" role="status">Loading Data Room tools…</span>}>
          <DataRoomToolPanels
            dealId={dealId}
            email={email}
            finalFocusEl={() => triggerRef.current?.isConnected ? triggerRef.current : null}
            onAssistantOpenChange={(open) => setPanelState({ session, panel: open ? "assistant" : null })}
            onSearchOpenChange={(open) => setPanelState({ session, panel: open ? "search" : null })}
            panel={panel}
            search={search}
          />
        </Suspense>
      ) : null}
    </ToolsContext.Provider>
  );
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
      <SpeedDialAction icon={<AssistantIcon className="size-5 text-icon-assistant" />} onClick={() => {
        actionChosen.current = true;
        tools.launch("assistant", triggerRef.current);
      }}>Assistant</SpeedDialAction>
      <SpeedDialAction icon={<WorkspaceSearchIcon className="size-5 text-icon-navigation" />} onClick={() => {
        actionChosen.current = true;
        tools.launch("search", triggerRef.current);
      }}>Workspace Search</SpeedDialAction>
    </SpeedDial>
  );
}
