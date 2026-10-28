import { sessionStore } from "@/auth/sessionStore";
import type { NotebookBlock } from "@/components/notebook/notebookSchema";

export type NotebookDraft = {
  id: string;
  title: string;
  blocks: NotebookBlock[];
  contentSchemaVersion: 1;
  createdAt: string;
  updatedAt: string;
};

export function createNotebookSession() {
  let notes: readonly NotebookDraft[] = [];
  const listeners = new Set<() => void>();
  const publish = (next: readonly NotebookDraft[]) => {
    notes = next;
    listeners.forEach((listener) => listener());
  };
  return {
    getSnapshot: () => notes,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    create() {
      const now = new Date().toISOString();
      const note: NotebookDraft = {
        id: crypto.randomUUID(), title: "", blocks: [], contentSchemaVersion: 1,
        createdAt: now, updatedAt: now,
      };
      publish([note, ...notes]);
      return note;
    },
    update(id: string, patch: Partial<Pick<NotebookDraft, "title" | "blocks">>) {
      publish(notes.map((note) => note.id === id
        ? { ...note, ...patch, updatedAt: new Date().toISOString() }
        : note));
    },
    sortByRecent() {
      const sorted = [...notes].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt) || a.id.localeCompare(b.id));
      if (sorted.some((note, index) => note !== notes[index])) publish(sorted);
    },
    remove(id: string) { publish(notes.filter((note) => note.id !== id)); },
    clear() { publish([]); },
  };
}

// Drafts survive workspace navigation, but never reload or an authentication change.
let current = createNotebookSession();
sessionStore.subscribe(() => {
  current.clear();
  current = createNotebookSession();
});
export function getNotebookSession() { return current; }
export function notebookTitle(note: Pick<NotebookDraft, "title">) {
  return note.title.trim() || "Untitled note";
}
