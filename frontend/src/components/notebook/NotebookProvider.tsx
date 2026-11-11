import {
  createContext, useContext, useEffect, useRef, useState, useSyncExternalStore, type ReactNode,
} from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { runtime } from "@quarry/runtime";
import { getNotebookPersistence } from "@/data/notebook/notebookPersistence";
import { getNotebookSession } from "@/data/notebook/notebookSession";
import { useStoreSelector } from "@/hooks/useStoreSelector";

function useNotebookOwner() {
  const [session] = useState(getNotebookSession);
  const [persistence] = useState(() => getNotebookPersistence(runtime.api));
  const [query, setQuery] = useState("");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const focusTitleAfterDrawer = useRef(false);
  useEffect(() => {
    if (persistence.getSnapshot().catalog === "idle") void persistence.load();
  }, [persistence]);
  return { session, persistence, query, setQuery, drawerOpen, setDrawerOpen, focusTitleAfterDrawer };
}
const NotebookContext = createContext<ReturnType<typeof useNotebookOwner> | null>(null);
export function NotebookProvider({ children }: { children: ReactNode }) {
  const value = useNotebookOwner();
  return <NotebookContext.Provider value={value}><NotebookSelectionLifecycle />{children}</NotebookContext.Provider>;
}
export function useNotebook() {
  const context = useContext(NotebookContext);
  if (!context) throw new Error("NotebookProvider is required.");
  return context;
}

/** Catalog consumers never subscribe to editor blocks or save-job progress. */
export function useNotebookCatalog() {
  const owner = useNotebook();
  const { session, persistence, query } = owner;
  const notes = useSyncExternalStore(session.subscribe, session.getCatalogSnapshot, session.getCatalogSnapshot);
  const saved = useSyncExternalStore(persistence.subscribe, persistence.getCatalogSnapshot, persistence.getCatalogSnapshot);
  const { noteId } = useParams();
  const selected = noteId ? notes.find((note) => note.id === noteId || saved.records[note.id]?.noteId === noteId) : undefined;
  const activeNotebookId = selected
    ? saved.records[selected.id]?.notebookId || null
    : saved.activeNotebookId !== undefined ? saved.activeNotebookId : saved.notebooks[0]?.notebookId ?? null;
  const notebookNotes = notes.filter((note) => (saved.records[note.id]?.notebookId || null) === activeNotebookId);
  return {
    ...owner, saved, notes, noteId, activeNotebookId,
    activeNotebook: saved.notebooks.find((book) => book.notebookId === activeNotebookId),
    notebookNotes,
    visibleNotes: notebookNotes.filter((note) =>
      (note.title.trim() || "Untitled note").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())),
  };
}

function useSelectedNotebookId() {
  const owner = useNotebook();
  const { session, persistence } = owner;
  const { noteId } = useParams();
  const savedId = useStoreSelector(persistence, (state) =>
    noteId ? Object.entries(state.records).find(([, record]) => record.noteId === noteId)?.[0] : undefined);
  const selectedId = useStoreSelector(session, (notes) => notes.find((note) => note.id === (savedId ?? noteId))?.id);
  const selectedSaved = useStoreSelector(persistence, (state) => selectedId ? state.records[selectedId] : undefined);
  return { ...owner, noteId, selectedId, selectedSaved };
}

/** Only the active edit buffer drives editor consumers. */
export function useNotebookSelection() {
  const state = useSelectedNotebookId();
  const selected = useStoreSelector(state.session, (notes) => notes.find((note) => note.id === state.selectedId));
  return { ...state, selected };
}

/** Save controls observe dirty transitions, not every editor change. */
export function useNotebookSaveState() {
  const state = useSelectedNotebookId();
  const changed = useStoreSelector(state.session, (notes) => {
    const note = notes.find((note) => note.id === state.selectedId);
    return Boolean(note && note.editVersion !== state.selectedSaved?.savedEditVersion);
  });
  return { ...state, changed };
}

function NotebookSelectionLifecycle() {
  const { selectedId, selectedSaved, noteId, persistence } = useSelectedNotebookId();
  const { navigationState } = useWorkspace();
  const navigate = useNavigate();
  useEffect(() => {
    if (selectedSaved?.noteId && noteId !== selectedSaved.noteId)
      navigate(`/hub/notebook/${encodeURIComponent(selectedSaved.noteId)}`, { replace: true, state: navigationState });
  }, [selectedSaved?.noteId, noteId, navigate, navigationState]);
  useEffect(() => {
    if (selectedId) void persistence.hydrate(selectedId);
  }, [selectedId, persistence]);
  return null;
}

export function useNotebookActions() {
  const owner = useNotebook();
  const { session, persistence, drawerOpen, setDrawerOpen, setQuery, focusTitleAfterDrawer } = owner;
  const { navigationState } = useWorkspace();
  const navigate = useNavigate();
  const { noteId } = useParams();
  const activeNotebook = () => {
    const saved = persistence.getSnapshot();
    const selected = noteId ? session.getSnapshot().find((note) => note.id === noteId || saved.records[note.id]?.noteId === noteId) : undefined;
    return selected ? saved.records[selected.id]?.notebookId || null
      : saved.activeNotebookId !== undefined ? saved.activeNotebookId : saved.notebooks[0]?.notebookId ?? null;
  };
  const select = (id: string, focusTitle = false) => {
    persistence.selectNotebook(persistence.getSnapshot().records[id]?.notebookId || null);
    setDrawerOpen(false);
    navigate(`/hub/notebook/${encodeURIComponent(id)}`, { state: { ...navigationState, focusNotebookTitle: focusTitle } });
  };
  return {
    chooseNotebook(notebookId: string | null) {
      persistence.selectNotebook(notebookId);
      setQuery("");
      navigate("/hub/notebook", { state: navigationState });
    },
    create() {
      focusTitleAfterDrawer.current = drawerOpen;
      setQuery("");
      select(persistence.createDraft(activeNotebook()).id, true);
    },
    async remove(id: string) {
      const notebookId = activeNotebook();
      persistence.selectNotebook(notebookId);
      await persistence.remove(id);
      if (getNotebookSession() !== session || persistence.getSnapshot().activeNotebookId !== notebookId) return;
      const next = session.getSnapshot().find((note) =>
        (persistence.getSnapshot().records[note.id]?.notebookId || null) === notebookId);
      if (next) select(next.id);
      else navigate("/hub/notebook", { state: navigationState });
    },
  };
}
