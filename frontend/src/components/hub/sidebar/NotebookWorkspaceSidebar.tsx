import { useId } from "react";
import { NavLink } from "react-router-dom";
import { BookOpen, ChevronDown, FileText, Plus, Search } from "lucide-react";
import { useNotebook } from "@/components/notebook/NotebookProvider";
import { notebookTitle } from "@/data/notebook/notebookSession";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useWorkspace } from "@/app/WorkspaceProvider";
import { SidebarFrame } from "./SidebarFrame";

export function NotebookWorkspaceSidebar() {
  const { email, navigationState } = useWorkspace();
  const { create } = useNotebook();
  return (
    <SidebarFrame
      alignedHeader
      email={email}
      navigationState={navigationState}
      headerBackLabel="Back to Deal Hub"
      sidebarIcon="bookOpen"
      sidebarLabel="Notebook"
    >
      {({ collapsed }) =>
        collapsed ? (
          <nav className="space-y-2" aria-label="Notebook actions">
            <Button
              aria-label="New note"
              title="New note"
              variant="ghost"
              size="icon"
              onClick={create}
            >
              <Plus />
            </Button>
          </nav>
        ) : (
          <NotebookNotesList />
        )
      }
    </SidebarFrame>
  );
}

export function NotebookNotesList() {
  const {
    notebookNotes,
    activeNotebook,
    activeNotebookId,
    chooseNotebook,
    visibleNotes,
    query,
    setQuery,
    create,
    session,
    setDrawerOpen,
    saved,
    persistence,
  } = useNotebook();
  const { navigationState } = useWorkspace();
  const searchId = useId();
  let notebookLabel = activeNotebook?.title.trim() ?? "Unsaved notes";
  if (activeNotebook && !/\bnotes$/i.test(notebookLabel)) notebookLabel += " notes";
  return (
    <div
      className="flex min-h-0 flex-col gap-4"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget))
          session.sortByRecent();
      }}
    >
      <div className="relative">
        <label className="sr-only" htmlFor={searchId}>
          Search notes
        </label>
        <Search className="pointer-events-none absolute left-3 top-2.5 size-4 text-sidebar-muted" />
        <input
          id={searchId}
          type="search"
          value={query}
          maxLength={100}
          placeholder="Search notes"
          className="h-9 w-full rounded-lg border border-border bg-background/60 pl-9 pr-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring"
          onChange={(event) => setQuery(event.currentTarget.value)}
        />
      </div>
      <div>
        <div className="mb-2 flex items-center justify-between gap-2 px-2">
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                aria-label="Choose notebook"
                title={notebookLabel}
                className="flex min-w-0 items-center gap-1 rounded-sm text-left text-[11px] font-medium uppercase tracking-wider text-sidebar-muted outline-none hover:text-sidebar-active focus-visible:ring-2 focus-visible:ring-ring"
              >
                <span className="truncate">
                  {notebookLabel} · {notebookNotes.length}
                </span>
                <ChevronDown className="size-3 shrink-0" aria-hidden="true" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent
              align="start"
              className="w-64 max-w-[calc(100vw-2rem)]"
            >
              <DropdownMenuRadioGroup
                value={activeNotebookId ?? "unsaved"}
                onValueChange={(value) =>
                  chooseNotebook(value === "unsaved" ? null : value)
                }
              >
                {saved.notebooks.map((book) => (
                  <DropdownMenuRadioItem
                    key={book.notebookId}
                    value={book.notebookId}
                  >
                    <span className="truncate">{book.title}</span>
                  </DropdownMenuRadioItem>
                ))}
                {saved.notebooks.length > 0 && <DropdownMenuSeparator />}
                <DropdownMenuRadioItem value="unsaved">Unsaved notes</DropdownMenuRadioItem>
              </DropdownMenuRadioGroup>
            </DropdownMenuContent>
          </DropdownMenu>
          <Button
            type="button"
            aria-label="New note"
            title="New note"
            variant="ghost"
            size="icon-xs"
            className="text-sidebar-muted"
            onClick={create}
          >
            <Plus className="size-3.5" aria-hidden="true" />
          </Button>
        </div>
        {saved.catalog === "loading" && (
          <p role="status" className="px-2 py-2 text-xs text-sidebar-muted">
            Loading notes…
          </p>
        )}
        {saved.catalogError && (
          <div className="px-2 py-2 text-xs text-sidebar-muted">
            <p role="alert">{saved.catalogError}</p>
            <Button
              variant="link"
              size="sm"
              onClick={() => { void persistence.load(); }}
            >
              Retry loading notebooks
            </Button>
          </div>
        )}
        {visibleNotes.length ? (
          <nav aria-label="Notes" className="space-y-1">
            {visibleNotes.map((note) => (
              <NavLink
                key={note.id}
                to={`/hub/notebook/${encodeURIComponent(saved.records[note.id]?.noteId ?? note.id)}`}
                state={navigationState}
                onClick={() => setDrawerOpen(false)}
                className={({ isActive }) =>
                  `flex items-start gap-2.5 rounded-lg px-3 py-3 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring ${isActive ? "bg-sidebar-selected text-sidebar-active" : "text-sidebar-text hover:bg-sidebar-hover"}`
                }
              >
                <FileText className="mt-0.5 size-4 shrink-0 text-sidebar-muted" />
                <div className="min-w-0">
                  <p className="truncate font-medium">{notebookTitle(note)}</p>
                  <time
                    className="mt-1 block text-[11px] text-sidebar-muted"
                    dateTime={note.updatedAt}
                  >
                    {new Date(note.updatedAt).toLocaleDateString(undefined, {
                      month: "short",
                      day: "numeric",
                    })}
                  </time>
                </div>
              </NavLink>
            ))}
          </nav>
        ) : saved.catalog !== "loading" && !saved.catalogError ? (
          <div
            className="px-2 py-8 text-center text-sm text-sidebar-muted"
            role="status"
          >
            <BookOpen className="mx-auto mb-3 size-5" />
            <p>{query.trim() ? "No matching notes" : "No notes yet"}</p>
            {query.trim() ? (
              <Button variant="link" size="sm" onClick={() => setQuery("")}>
                Clear search
              </Button>
            ) : (
              <p className="mt-1 text-xs">Create a note to get started.</p>
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
