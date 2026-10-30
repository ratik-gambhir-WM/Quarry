import { Input } from "../ui/input";
import { useState } from "react";
import { LoaderCircle, Save } from "lucide-react";
import { useNotebook } from "./NotebookProvider";
import { noteContent } from "@/data/notebook/notebookPersistence";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogTitle,
} from "@/components/ui/dialog";

export function NotebookSaveButton() {
  const { selected, selectedSaved, saved, persistence } = useNotebook();
  const [choosing, setChoosing] = useState(false);
  const [notebook, setNotebook] = useState("new");
  const [title, setTitle] = useState("Personal notes");
  const saving = selectedSaved?.status === "saving";
  const disabled =
    !selected ||
    saving ||
    saved.catalog !== "ready" ||
    selectedSaved?.status === "conflict" ||
    (selectedSaved && !selectedSaved.hydrated);
  function save() {
    if (!selected || disabled) return;
    if (selectedSaved?.notebookId) {
      void persistence.save(selected.id);
      return;
    }
    setNotebook(saved.notebooks[0]?.notebookId ?? "new");
    setChoosing(true);
  }
  return (
    <>
      <Button
        size="sm"
        className="ml-auto"
        disabled={Boolean(disabled)}
        onClick={save}
      >
        {saving ? (
          <LoaderCircle className="size-3.5 animate-spin motion-reduce:animate-none" />
        ) : (
          <Save className="size-3.5" />
        )}
        {saving ? "Saving…" : "Save"}
      </Button>
      <Dialog open={choosing} onOpenChange={setChoosing}>
        <DialogContent className="sm:max-w-[26rem]">
          <DialogTitle>Save note</DialogTitle>
          <DialogDescription>
            Choose where to keep this note. Later saves update the same note.
          </DialogDescription>
          <form
            onSubmit={(event) => {
              event.preventDefault();
              if (!selected || (notebook === "new" && !title.trim())) return;
              setChoosing(false);
              void persistence.save(
                selected.id,
                notebook === "new" ? { title } : { notebookId: notebook },
              );
            }}
            className="space-y-4"
          >
            <div className="space-y-2">
              <label htmlFor="save-notebook" className="text-sm font-medium">
                Notebook
              </label>
              <select
                id="save-notebook"
                value={notebook}
                onChange={(event) => setNotebook(event.target.value)}
                className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                {saved.notebooks.map((book) => (
                  <option key={book.notebookId} value={book.notebookId}>
                    {book.title}
                  </option>
                ))}
                <option value="new">Create a notebook…</option>
              </select>
            </div>
            {notebook === "new" && (
              <div className="space-y-2">
                <label
                  htmlFor="save-notebook-title"
                  className="text-sm font-medium"
                >
                  Notebook name
                </label>
                <Input
                  id="save-notebook-title"
                  value={title}
                  maxLength={300}
                  required
                  onChange={(event) => setTitle(event.target.value)}
                  className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                />
              </div>
            )}
            <DialogFooter>
              <DialogClose asChild>
                <Button type="button" variant="outline">
                  Cancel
                </Button>
              </DialogClose>
              <Button
                type="submit"
                disabled={notebook === "new" && !title.trim()}
              >
                Save note
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </>
  );
}

export function NotebookSaveStatus() {
  const { selected, selectedSaved, saved, persistence } = useNotebook();
  const [reload, setReload] = useState(false);
  const changed =
    selected && selectedSaved?.savedContent !== noteContent(selected);
  let text = "";
  if (saved.catalog === "loading")
    text = "Loading saved notes… You can keep writing.";
  if (saved.catalogError) text = saved.catalogError;
  if (
    selectedSaved?.hydrated &&
    selectedSaved.savedContent &&
    selectedSaved.status === "idle"
  )
    text = changed ? "Unsaved changes" : "Stored on server";
  if (selectedSaved?.status === "saving")
    text = "Saving this version… You can keep writing.";
  if (selectedSaved?.status === "saved")
    text = changed ? "Unsaved changes" : "Saved";
  if (selectedSaved?.message) text = selectedSaved.message;
  if (!text) return null;
  return (
    <>
      <div className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border bg-surface-container-low px-5 py-2 text-xs leading-5 text-muted-foreground sm:px-8">
        <span
          role={
            selectedSaved?.message || saved.catalogError ? "alert" : "status"
          }
        >
          {text}
        </span>
        {saved.catalogError && (
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0 text-xs"
            onClick={() => {
              void persistence.load();
            }}
          >
            Retry loading notes
          </Button>
        )}
        {selectedSaved?.status === "conflict" && (
          <Button
            variant="link"
            size="sm"
            className="h-auto p-0 text-xs"
            onClick={() => setReload(true)}
          >
            Reload saved note
          </Button>
        )}
      </div>
      <Dialog open={reload} onOpenChange={setReload}>
        <DialogContent>
          <DialogTitle>Reload saved note?</DialogTitle>
          <DialogDescription>
            This replaces your unsaved edits with the version on the server.
          </DialogDescription>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="outline">Keep editing</Button>
            </DialogClose>
            <Button
              onClick={() => {
                setReload(false);
                if (selected) void persistence.hydrate(selected.id, true);
              }}
            >
              Reload saved note
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}
