import {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { runtime } from "@quarry/runtime";
import { getNotebookPersistence } from "@/data/notebook/notebookPersistence";
import { getNotebookSession } from "@/data/notebook/notebookSession";

function useNotebookState() {
  const [session] = useState(getNotebookSession);
  const notes = useSyncExternalStore(
    session.subscribe,
    session.getSnapshot,
    session.getSnapshot,
  );
  const [persistence] = useState(() => getNotebookPersistence(runtime.api));
  const saved = useSyncExternalStore(
    persistence.subscribe,
    persistence.getSnapshot,
    persistence.getSnapshot,
  );
  useEffect(() => {
    if (persistence.getSnapshot().catalog === "idle") void persistence.load();
  }, [persistence]);
  const [query, setQuery] = useState("");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const focusTitleAfterDrawer = useRef(false);
  const { noteId } = useParams();
  const { navigationState } = useWorkspace();
  const navigate = useNavigate();
  const select = (id: string, focusTitle = false) => {
    persistence.selectNotebook(persistence.getSnapshot().records[id]?.notebookId || null);
    setDrawerOpen(false);
    navigate(`/hub/notebook/${encodeURIComponent(id)}`, {
      state: { ...navigationState, focusNotebookTitle: focusTitle },
    });
  };
  const selected = noteId
    ? notes.find(
        (note) =>
          note.id === noteId || saved.records[note.id]?.noteId === noteId,
      )
    : undefined;
  const selectedSaved = selected ? saved.records[selected.id] : undefined;
  const activeNotebookId = selected
    ? selectedSaved?.notebookId || null
    : saved.activeNotebookId !== undefined
      ? saved.activeNotebookId
      : saved.notebooks[0]?.notebookId ?? null;
  const activeNotebook = saved.notebooks.find((book) =>
    book.notebookId === activeNotebookId,
  );
  const notebookNotes = notes.filter((note) =>
    (saved.records[note.id]?.notebookId || null) === activeNotebookId,
  );
  useEffect(() => {
    if (selectedSaved?.noteId && noteId !== selectedSaved.noteId)
      navigate(`/hub/notebook/${encodeURIComponent(selectedSaved.noteId)}`, {
        replace: true,
        state: navigationState,
      });
  }, [selectedSaved?.noteId, noteId, navigate, navigationState]);
  useEffect(() => {
    if (selected) void persistence.hydrate(selected.id);
  }, [selected?.id, persistence]);
  return {
    persistence,
    saved,
    selectedSaved,
    session,
    notes,
    query,
    setQuery,
    drawerOpen,
    setDrawerOpen,
    focusTitleAfterDrawer,
    noteId,
    selected,
    activeNotebookId,
    activeNotebook,
    notebookNotes,
    visibleNotes: notebookNotes.filter((note) =>
      (note.title.trim() || "Untitled note")
        .toLocaleLowerCase()
        .includes(query.trim().toLocaleLowerCase()),
    ),
    select,
    chooseNotebook(notebookId: string | null) {
      persistence.selectNotebook(notebookId);
      setQuery("");
      navigate("/hub/notebook", { state: navigationState });
    },
    create() {
      focusTitleAfterDrawer.current = drawerOpen;
      setQuery("");
      select(persistence.createDraft(activeNotebookId).id, true);
    },
    async remove(id: string) {
      persistence.selectNotebook(activeNotebookId);
      await persistence.remove(id);
      if (getNotebookSession() !== session) return;
      if (persistence.getSnapshot().activeNotebookId !== activeNotebookId) return;
      const next = session.getSnapshot().find((note) =>
        (persistence.getSnapshot().records[note.id]?.notebookId || null) ===
        activeNotebookId,
      );
      if (next) select(next.id);
      else navigate("/hub/notebook", { state: navigationState });
    },
  };
}

const NotebookContext = createContext<ReturnType<
  typeof useNotebookState
> | null>(null);
export function NotebookProvider({ children }: { children: ReactNode }) {
  const value = useNotebookState();
  return (
    <NotebookContext.Provider value={value}>
      {children}
    </NotebookContext.Provider>
  );
}
export function useNotebook() {
  const context = useContext(NotebookContext);
  if (!context) throw new Error("NotebookProvider is required.");
  return context;
}
