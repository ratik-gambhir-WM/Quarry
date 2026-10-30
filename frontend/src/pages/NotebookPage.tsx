import {
  Component,
  lazy,
  Suspense,
  useEffect,
  useRef,
  useState,
  type ComponentProps,
} from "react";
import { NavLink, useLocation } from "react-router-dom";
import { BookOpen, FileText, List, Plus, Trash2 } from "lucide-react";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { useUser } from "@/auth/UserProvider";
import { WorkspaceHomeShell } from "@/components/hub/WorkspaceHomeShell";
import { NotebookNotesList } from "@/components/hub/sidebar/NotebookWorkspaceSidebar";
import {
  NotebookSaveButton,
  NotebookSaveStatus,
} from "@/components/notebook/NotebookSaveButton";
import {
  hasUnconfirmedSave,
  noteContent,
} from "@/data/notebook/notebookPersistence";
import { NotebookExportMenu } from "@/components/notebook/NotebookExportMenu";
import {
  NotebookProvider,
  useNotebook,
} from "@/components/notebook/NotebookProvider";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  notebookTitle,
  type NotebookDraft,
} from "@/data/notebook/notebookSession";

function loadEditor() {
  return lazy(() =>
    import("@/components/notebook/NotebookEditor").then((module) => ({
      default: module.NotebookEditor,
    })),
  );
}
type EditorProps = ComponentProps<
  typeof import("@/components/notebook/NotebookEditor").NotebookEditor
>;

export function NotebookPage() {
  const { generation } = useUser();
  return (
    <NotebookProvider key={generation}>
      <NotebookWorkspace />
    </NotebookProvider>
  );
}

function NotebookWorkspace() {
  const {
    selected,
    noteId,
    notes,
    notebookNotes,
    create,
    drawerOpen,
    setDrawerOpen,
    focusTitleAfterDrawer,
    saved,
    selectedSaved,
    persistence,
  } = useNotebook();
  const { navigationState } = useWorkspace();
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (
        !notes.some((note) => {
          const remote = saved.records[note.id];
          return (
            !remote ||
            (remote.hydrated && remote.savedContent !== noteContent(note))
          );
        })
      )
        return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [notes, saved.records]);

  return (
    <WorkspaceHomeShell
      activeHomeSection="notebook"
      sidebarMode="notebook"
      contentMode="fill"
      header={
        <div className="flex w-full items-center gap-1.5 sm:gap-3">
          <Dialog open={drawerOpen} onOpenChange={setDrawerOpen}>
            <DialogTrigger asChild>
              <Button className="lg:hidden" size="sm" variant="ghost">
                <List />
                Notes
              </Button>
            </DialogTrigger>
            <DialogContent
              onCloseAutoFocus={(event) => {
                if (focusTitleAfterDrawer.current) {
                  focusTitleAfterDrawer.current = false;
                  event.preventDefault();
                  document.getElementById("notebook-title")?.focus();
                }
              }}
              className="flex flex-col inset-y-0 left-0 top-0 h-dvh max-h-dvh w-80 max-w-[90vw] translate-x-0 translate-y-0 overflow-y-auto rounded-none sm:max-w-80"
            >
              <DialogTitle>Notebook</DialogTitle>
              <DialogDescription className="sr-only">
                Find or create a note.
              </DialogDescription>
              <Button asChild className="justify-start" variant="ghost">
                <NavLink to="/hub" state={navigationState}>
                  ← Back to Deal Hub
                </NavLink>
              </Button>
              <NotebookNotesList />
            </DialogContent>
          </Dialog>
          <BookOpen className="hidden size-4 text-muted-foreground sm:block" />
          <h1 className="text-[13px] font-semibold">Notebook</h1>
          {selected && (
            <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
              <FileText className="size-3.5" aria-hidden="true" />
              <span className="sr-only sm:not-sr-only">Personal note</span>
            </span>
          )}
          <NotebookSaveButton />
          <div>
            <NotebookExportMenu key={selected?.id ?? "empty"} note={selected} />
          </div>
        </div>
      }
    >
      <div className="flex h-full min-h-0 flex-col">
        {selected && selectedSaved && !selectedSaved.hydrated ? (
          <div className="p-8 text-sm text-muted-foreground">
            {selectedSaved.message ? (
              <>
                <p role="alert">{selectedSaved.message}</p>
                <Button
                  className="mt-3"
                  variant="outline"
                  onClick={() => {
                    void persistence.hydrate(selected.id);
                  }}
                >
                  Retry opening note
                </Button>
              </>
            ) : (
              <p role="status">Opening note…</p>
            )}
          </div>
        ) : selected ? (
          <NotebookDocument
            key={`${selected.id}:${selectedSaved?.reloadKey ?? 0}`}
            note={selected}
          />
        ) : (
          <div className="flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-8">
            <div className="w-full max-w-[24rem] text-center">
              <div className="mx-auto mb-6 flex size-14 items-center justify-center rounded-2xl border border-border bg-surface-container-low">
                <BookOpen className="size-6 text-primary" />
              </div>
              <h2 className="text-xl font-semibold text-text-main">
                {noteId ? "Note unavailable" : "A little room to think"}
              </h2>
              <p className="mb-6 mt-3 text-sm leading-6 text-muted-foreground">
                {noteId
                  ? "Select a saved note from the sidebar or start a new one. If notes are still loading, this note will open when they arrive."
                  : notebookNotes.length
                    ? "Select a note from the sidebar, or start a new one."
                    : "Capture an idea, work through a question, or keep your thoughts together."}
              </p>
              <Button onClick={create}>
                <Plus />
                New note
              </Button>
            </div>
          </div>
        )}
        <NotebookSaveStatus />
      </div>
    </WorkspaceHomeShell>
  );
}

function NotebookDocument({ note }: { note: NotebookDraft }) {
  const { session, remove, selectedSaved } = useNotebook();
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState("");
  const [confirmDelete, setConfirmDelete] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const location = useLocation();
  // The route requests title focus only when the user creates a note.
  const [focusTitle] = useState(() =>
    Boolean(location.state?.focusNotebookTitle),
  );
  useEffect(() => {
    if (focusTitle) titleRef.current?.focus();
  }, [focusTitle]);

  return (
    <section
      aria-label="Note editor"
      className="min-h-0 flex-1 overflow-y-auto"
      onBlur={(event) => {
        if (
          !event.currentTarget.contains(event.relatedTarget) &&
          !(
            event.relatedTarget instanceof Element &&
            event.relatedTarget.closest('nav[aria-label="Notes"]')
          )
        )
          session.sortByRecent();
      }}
    >
      <div className="mx-auto w-full max-w-4xl px-10 pb-8 pt-3 sm:px-16 sm:pt-4">
        <div className="relative">
          <label className="sr-only" htmlFor="notebook-title">
            Note title
          </label>
          <input
            ref={titleRef}
            id="notebook-title"
            value={note.title}
            maxLength={300}
            placeholder="Untitled note"
            className="w-full rounded-sm border-0 bg-transparent py-2 pr-10 text-3xl font-semibold tracking-tight text-text-main outline-none placeholder:text-muted-foreground/70 focus-visible:ring-2 focus-visible:ring-ring sm:text-4xl"
            onChange={(event) =>
              session.update(note.id, { title: event.currentTarget.value })
            }
          />
          <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
            <DialogTrigger asChild>
              <Button
                disabled={
                  selectedSaved?.status === "saving" ||
                  hasUnconfirmedSave(selectedSaved)
                }
                aria-label="Delete note"
                className="absolute right-0 top-1/2 -translate-y-1/2"
                variant="ghost"
                size="icon-sm"
              >
                <Trash2 className="text-muted-foreground" />
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-[24rem]">
              <DialogTitle>Delete this note?</DialogTitle>
              <DialogDescription>
                “{notebookTitle(note)}” will be permanently deleted. This cannot
                be undone.
              </DialogDescription>
              {deleteError && (
                <p role="alert" className="text-sm text-error">
                  {deleteError}
                </p>
              )}
              <DialogFooter>
                <DialogClose asChild>
                  <Button variant="outline" disabled={deleting}>
                    Cancel
                  </Button>
                </DialogClose>
                <Button
                  variant="destructive"
                  disabled={deleting}
                  onClick={() => {
                    setDeleting(true);
                    setDeleteError("");
                    void remove(note.id).catch(() => {
                      setDeleteError(
                        "Could not delete this note. Please try again.",
                      );
                      setDeleting(false);
                    });
                  }}
                >
                  {deleting ? "Deleting…" : "Delete note"}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
        <p className="mb-10 mt-2 text-xs text-muted-foreground">
          {selectedSaved?.noteId ? "Personal note" : "Unsaved note"} · Type /
          for blocks
        </p>
        <EditorBoundary
          blocks={note.blocks}
          onChange={(blocks) => session.update(note.id, { blocks })}
        />
      </div>
    </section>
  );
}

class EditorBoundary extends Component<EditorProps, { failed: boolean }> {
  state = { failed: false };
  Editor = loadEditor();
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    if (this.state.failed)
      return (
        <div
          role="alert"
          className="rounded-lg border border-border p-5 text-sm"
        >
          <p>The editor could not open. Your draft is still in this session.</p>
          <Button
            className="mt-3"
            variant="outline"
            onClick={() => {
              this.Editor = loadEditor();
              this.setState({ failed: false });
            }}
          >
            Retry editor
          </Button>
        </div>
      );
    const Editor = this.Editor;
    return (
      <Suspense
        fallback={
          <p role="status" className="py-6 text-sm text-muted-foreground">
            Loading editor…
          </p>
        }
      >
        <Editor {...this.props} />
      </Suspense>
    );
  }
}
