import { useId } from "react";
import { NavLink } from "react-router-dom";
import { BookOpen, FileText, Plus, Search } from "lucide-react";
import { useNotebook } from "@/components/notebook/NotebookProvider";
import { notebookTitle } from "@/data/notebook/notebookSession";
import { Button } from "@/components/ui/button";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { SidebarFrame } from "./SidebarFrame";

export function NotebookWorkspaceSidebar() {
  const { email, navigationState } = useWorkspace();
  const { create } = useNotebook();
  return (
    <SidebarFrame alignedHeader email={email} navigationState={navigationState}
      headerBackLabel="Back to Deal Hub" sidebarIcon="bookOpen" sidebarLabel="Notebook">
      {({ collapsed }) => collapsed ? (
        <nav className="space-y-2" aria-label="Notebook actions">
          <Button aria-label="New note" title="New note" variant="ghost" size="icon" onClick={create}><Plus /></Button>
        </nav>
      ) : <NotebookNotesList />}
    </SidebarFrame>
  );
}

export function NotebookNotesList() {
  const { notes, visibleNotes, query, setQuery, create, session, setDrawerOpen } = useNotebook();
  const { navigationState } = useWorkspace();
  const searchId = useId();
  return (
    <div className="flex min-h-0 flex-col gap-4" onBlur={(event) => {
      if (!event.currentTarget.contains(event.relatedTarget)) session.sortByRecent();
    }}>
      <Button className="w-full justify-start" onClick={create}><Plus />New note</Button>
      <div className="relative">
        <label className="sr-only" htmlFor={searchId}>Search notes</label>
        <Search className="pointer-events-none absolute left-3 top-2.5 size-4 text-sidebar-muted" />
        <input id={searchId} type="search" value={query} maxLength={100} placeholder="Search notes"
          className="h-9 w-full rounded-lg border border-border bg-background/60 pl-9 pr-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring"
          onChange={(event) => setQuery(event.currentTarget.value)} />
      </div>
      <div>
        <p className="mb-2 px-2 text-[11px] font-medium uppercase tracking-wider text-sidebar-muted">Your notes · {notes.length}</p>
        {visibleNotes.length ? (
          <nav aria-label="Notes" className="space-y-1">
            {visibleNotes.map((note) => (
              <NavLink key={note.id} to={`/hub/notebook/${encodeURIComponent(note.id)}`} state={navigationState}
                onClick={() => setDrawerOpen(false)}
                className={({ isActive }) => `flex items-start gap-2.5 rounded-lg px-3 py-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring ${isActive ? "bg-sidebar-selected text-sidebar-active" : "text-sidebar-text hover:bg-sidebar-hover"}`}>
                <FileText className="mt-0.5 size-4 shrink-0 text-sidebar-muted" />
                <div className="min-w-0"><p className="truncate font-medium">{notebookTitle(note)}</p>
                  <time className="mt-1 block text-[11px] text-sidebar-muted" dateTime={note.updatedAt}>
                    {new Date(note.updatedAt).toLocaleDateString(undefined, { month: "short", day: "numeric" })}
                  </time>
                </div>
              </NavLink>
            ))}
          </nav>
        ) : (
          <div className="px-2 py-8 text-center text-sm text-sidebar-muted" role="status">
            <BookOpen className="mx-auto mb-3 size-5" />
            <p>{query.trim() ? "No matching notes" : "No notes yet"}</p>
            {query.trim() ? <Button variant="link" size="sm" onClick={() => setQuery("")}>Clear search</Button> : <p className="mt-1 text-xs">Create a note to get started.</p>}
          </div>
        )}
      </div>
      <p className="px-2 text-xs leading-5 text-sidebar-muted">Session drafts only. Notes disappear on reload or sign-out.</p>
    </div>
  );
}
