import { Component, lazy, Suspense, useEffect, useRef, useState, type ComponentProps } from "react";
import { NavLink, useLocation } from "react-router-dom";
import { BookOpen, FileText, List, Plus, Trash2 } from "lucide-react";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { useUser } from "@/auth/UserProvider";
import { WorkspaceHomeShell } from "@/components/hub/WorkspaceHomeShell";
import { NotebookNotesList } from "@/components/hub/sidebar/NotebookWorkspaceSidebar";
import { NotebookExportMenu } from "@/components/notebook/NotebookExportMenu";
import { NotebookProvider, useNotebook } from "@/components/notebook/NotebookProvider";
import { Button } from "@/components/ui/button";
import { Dialog, DialogClose, DialogContent, DialogDescription, DialogFooter, DialogTitle, DialogTrigger } from "@/components/ui/dialog";
import { notebookTitle, type NotebookDraft } from "@/data/notebook/notebookSession";

function loadEditor() {
  return lazy(() => import("@/components/notebook/NotebookEditor").then((module) => ({ default: module.NotebookEditor })));
}
type EditorProps = ComponentProps<typeof import("@/components/notebook/NotebookEditor").NotebookEditor>;

export function NotebookPage() {
  const { generation } = useUser();
  return <NotebookProvider key={generation}><NotebookWorkspace /></NotebookProvider>;
}

function NotebookWorkspace() {
  const { selected, noteId, notes, create, drawerOpen, setDrawerOpen, focusTitleAfterDrawer } = useNotebook();
  const { navigationState } = useWorkspace();
  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => {
      if (!notes.length) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [notes.length]);

  return <WorkspaceHomeShell activeHomeSection="notebook" sidebarMode="notebook" contentMode="fill" header={
    <div className="flex w-full items-center gap-3">
      <Dialog open={drawerOpen} onOpenChange={setDrawerOpen}>
        <DialogTrigger asChild><Button className="lg:hidden" size="sm" variant="ghost"><List />Notes</Button></DialogTrigger>
        <DialogContent onCloseAutoFocus={(event) => {
          if (focusTitleAfterDrawer.current) {
            focusTitleAfterDrawer.current = false;
            event.preventDefault();
            document.getElementById("notebook-title")?.focus();
          }
        }} className="flex flex-col inset-y-0 left-0 top-0 h-dvh max-h-dvh w-80 max-w-[90vw] translate-x-0 translate-y-0 overflow-y-auto rounded-none sm:max-w-80">
          <DialogTitle>Notebook</DialogTitle>
          <DialogDescription className="sr-only">Find or create a session draft.</DialogDescription>
          <Button asChild className="justify-start" variant="ghost"><NavLink to="/hub" state={navigationState}>← Back to Deal Hub</NavLink></Button>
          <NotebookNotesList />
        </DialogContent>
      </Dialog>
      <BookOpen className="hidden size-4 text-muted-foreground sm:block" />
      <h1 className="text-[13px] font-semibold">Notebook</h1>
      <span className="ml-auto hidden text-[11px] text-muted-foreground sm:inline">Session drafts</span>
      <div className="ml-auto sm:ml-0"><NotebookExportMenu key={selected?.id ?? "empty"} note={selected} /></div>
    </div>
  }>
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 border-b border-border bg-surface-container-low px-5 py-2 text-xs leading-5 text-muted-foreground sm:px-8" role="note">
        Notes stay in this session only. Reloading or signing out clears them.
      </div>
      {selected ? <NotebookDocument key={selected.id} note={selected} /> : (
        <div className="flex min-h-0 flex-1 items-center justify-center overflow-y-auto p-8">
          <div className="w-full max-w-[24rem] text-center">
            <div className="mx-auto mb-6 flex size-14 items-center justify-center rounded-2xl border border-border bg-surface-container-low"><BookOpen className="size-6 text-primary" /></div>
            <h2 className="text-xl font-semibold text-text-main">{noteId ? "Note unavailable" : "A little room to think"}</h2>
            <p className="mb-6 mt-3 text-sm leading-6 text-muted-foreground">{noteId
              ? "This note is no longer in this session. Select another note or start a new one."
              : notes.length ? "Select a note from the sidebar, or start a new one." : "Capture an idea, work through a question, or keep your thoughts together."}</p>
            <Button onClick={create}><Plus />New note</Button>
          </div>
        </div>
      )}
    </div>
  </WorkspaceHomeShell>;
}

function NotebookDocument({ note }: { note: NotebookDraft }) {
  const { session, remove } = useNotebook();
  const [confirmDelete, setConfirmDelete] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const location = useLocation();
  // The route requests title focus only when the user creates a note.
  const [focusTitle] = useState(() => Boolean(location.state?.focusNotebookTitle));
  useEffect(() => {
    if (focusTitle) titleRef.current?.focus();
  }, [focusTitle]);

  return <section aria-label="Note editor" className="min-h-0 flex-1 overflow-y-auto" onBlur={(event) => {
    if (!event.currentTarget.contains(event.relatedTarget)
      && !(event.relatedTarget instanceof Element && event.relatedTarget.closest('nav[aria-label="Notes"]'))) session.sortByRecent();
  }}>
    <div className="mx-auto w-full max-w-4xl px-10 pb-8 pt-8 sm:px-16 sm:pt-12">
      <div className="mb-6 flex items-center justify-between gap-3">
        <span className="flex items-center gap-2 text-xs text-muted-foreground"><FileText className="size-3.5" />Personal note</span>
        <Dialog open={confirmDelete} onOpenChange={setConfirmDelete}>
          <DialogTrigger asChild><Button aria-label="Delete note" variant="ghost" size="icon-sm"><Trash2 className="text-muted-foreground" /></Button></DialogTrigger>
          <DialogContent className="sm:max-w-[24rem]">
            <DialogTitle>Delete this note?</DialogTitle>
            <DialogDescription>“{notebookTitle(note)}” will be removed from this session. This cannot be undone.</DialogDescription>
            <DialogFooter>
              <DialogClose asChild><Button variant="outline">Cancel</Button></DialogClose>
              <Button variant="destructive" onClick={() => { setConfirmDelete(false); remove(note.id); }}>Delete note</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
      <label className="sr-only" htmlFor="notebook-title">Note title</label>
      <input ref={titleRef} id="notebook-title" value={note.title} maxLength={200} placeholder="Untitled note"
        className="w-full rounded-sm border-0 bg-transparent py-2 text-3xl font-semibold tracking-tight text-text-main outline-none placeholder:text-muted-foreground/70 focus-visible:ring-2 focus-visible:ring-ring sm:text-4xl"
        onChange={(event) => session.update(note.id, { title: event.currentTarget.value })} />
      <p className="mb-10 mt-2 text-xs text-muted-foreground">Draft in this session · Type / for blocks</p>
      <EditorBoundary blocks={note.blocks} onChange={(blocks) => session.update(note.id, { blocks })} />
    </div>
  </section>;
}

class EditorBoundary extends Component<EditorProps, { failed: boolean }> {
  state = { failed: false };
  Editor = loadEditor();
  static getDerivedStateFromError() { return { failed: true }; }
  render() {
    if (this.state.failed) return <div role="alert" className="rounded-lg border border-border p-5 text-sm">
      <p>The editor could not open. Your draft is still in this session.</p>
      <Button className="mt-3" variant="outline" onClick={() => { this.Editor = loadEditor(); this.setState({ failed: false }); }}>Retry editor</Button>
    </div>;
    const Editor = this.Editor;
    return <Suspense fallback={<p role="status" className="py-6 text-sm text-muted-foreground">Loading editor…</p>}>
      <Editor {...this.props} />
    </Suspense>;
  }
}
