import { createContext, useContext, useRef, useState, useSyncExternalStore, type ReactNode } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { getNotebookSession } from "@/data/notebook/notebookSession";

function useNotebookState() {
  const [session] = useState(getNotebookSession);
  const notes = useSyncExternalStore(session.subscribe, session.getSnapshot, session.getSnapshot);
  const [query, setQuery] = useState("");
  const [drawerOpen, setDrawerOpen] = useState(false);
  const focusTitleAfterDrawer = useRef(false);
  const { noteId } = useParams();
  const { navigationState } = useWorkspace();
  const navigate = useNavigate();
  const select = (id: string, focusTitle = false) => {
    setDrawerOpen(false);
    navigate(`/hub/notebook/${encodeURIComponent(id)}`, {
      state: { ...navigationState, focusNotebookTitle: focusTitle },
    });
  };
  return {
    session, notes, query, setQuery, drawerOpen, setDrawerOpen, focusTitleAfterDrawer, noteId,
    selected: notes.find((note) => note.id === noteId),
    visibleNotes: notes.filter((note) => (note.title.trim() || "Untitled note").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())),
    select,
    create() {
      focusTitleAfterDrawer.current = drawerOpen;
      setQuery("");
      select(session.create().id, true);
    },
    remove(id: string) {
      session.remove(id);
      const next = session.getSnapshot()[0];
      if (next) select(next.id);
      else navigate("/hub/notebook", { state: navigationState });
    },
  };
}

const NotebookContext = createContext<ReturnType<typeof useNotebookState> | null>(null);
export function NotebookProvider({ children }: { children: ReactNode }) {
  const value = useNotebookState();
  return <NotebookContext.Provider value={value}>{children}</NotebookContext.Provider>;
}
export function useNotebook() {
  const context = useContext(NotebookContext);
  if (!context) throw new Error("NotebookProvider is required.");
  return context;
}
