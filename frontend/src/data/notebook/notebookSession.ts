import { sessionStore } from "@/auth/sessionStore";
import type { NotebookBlock } from "@/components/notebook/notebookSchema";

export type NotebookDraft = {
  id: string;
  title: string;
  blocks: NotebookBlock[];
  contentSchemaVersion: 1;
  editVersion: number;
  createdAt: string;
  updatedAt: string;
};

export type NotebookSummary = Pick<NotebookDraft, "id" | "title" | "createdAt" | "updatedAt">;

export function createNotebookSession() {
  let version = 0;
  let catalog: readonly NotebookSummary[] = [];
  const checkpoints = new Map<string, { content: string; version: number }>();
  const contentKey = (note: Pick<NotebookDraft, "title" | "blocks" | "contentSchemaVersion">) =>
    JSON.stringify({ title: note.title.trim(), blocks: note.blocks, contentSchemaVersion: note.contentSchemaVersion });
  const editedVersion = (note: NotebookDraft) => {
    const checkpoint = checkpoints.get(note.id);
    return checkpoint && contentKey(note) === checkpoint.content ? checkpoint.version : ++version;
  };
  let notes: readonly NotebookDraft[] = [];
  const listeners = new Set<() => void>();
  const publish = (next: readonly NotebookDraft[]) => {
    notes = next;
    const byId = new Map(catalog.map((note) => [note.id, note]));
    const summaries = notes.map((note) => {
      const previous = byId.get(note.id);
      return previous && previous.title === note.title &&
        new Date(previous.updatedAt).toDateString() === new Date(note.updatedAt).toDateString()
        ? previous : { id: note.id, title: note.title, createdAt: note.createdAt, updatedAt: note.updatedAt };
    });
    if (summaries.length !== catalog.length || summaries.some((note, index) => note !== catalog[index]))
      catalog = summaries;
    listeners.forEach((listener) => listener());
  };
  return {
    getSnapshot: () => notes,
    getCatalogSnapshot: () => catalog,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    create() {
      const now = new Date().toISOString();
      const note: NotebookDraft = {
        id: crypto.randomUUID(),
        title: "",
        blocks: [],
        contentSchemaVersion: 1,
        editVersion: ++version,
        createdAt: now,
        updatedAt: now,
      };
      publish([note, ...notes]);
      return note;
    },
    importMany(incoming: NotebookDraft[]) {
      const ids = new Set(notes.map((note) => note.id));
      const additions = incoming.filter((note) => !ids.has(note.id));
      if (additions.length) publish([...notes, ...additions]);
    },
    replace(
      id: string,
      note: Pick<
        NotebookDraft,
        "title" | "blocks" | "contentSchemaVersion" | "createdAt" | "updatedAt"
      >,
    ) {
      publish(
        notes.map((existing) =>
          existing.id === id ? { ...existing, ...note, editVersion: ++version } : existing,
        ),
      );
    },
    markSaved(id: string, content: Pick<NotebookDraft, "title" | "blocks" | "contentSchemaVersion">, savedVersion: number) {
      const checkpoint = { content: contentKey(content), version: savedVersion };
      checkpoints.set(id, checkpoint);
      const note = notes.find((note) => note.id === id);
      if (note && note.editVersion !== savedVersion && contentKey(note) === checkpoint.content)
        publish(notes.map((note) => note.id === id ? { ...note, editVersion: savedVersion } : note));
    },
    update(
      id: string,
      patch: Partial<Pick<NotebookDraft, "title" | "blocks">>,
    ) {
      publish(
        notes.map((note) =>
          note.id === id
            ? { ...note, ...patch, editVersion: editedVersion({ ...note, ...patch }), updatedAt: new Date().toISOString() }
            : note,
        ),
      );
    },
    sortByRecent() {
      const sorted = [...notes].sort(
        (a, b) =>
          b.updatedAt.localeCompare(a.updatedAt) || a.id.localeCompare(b.id),
      );
      if (sorted.some((note, index) => note !== notes[index])) publish(sorted);
    },
    remove(id: string) {
      checkpoints.delete(id);
      publish(notes.filter((note) => note.id !== id));
    },
    clear() {
      checkpoints.clear();
      publish([]);
    },
  };
}

export type NotebookSession = ReturnType<typeof createNotebookSession>;

// Drafts survive workspace navigation, but never reload or an authentication change.
let current = createNotebookSession();
sessionStore.subscribe(() => {
  current.clear();
  current = createNotebookSession();
});
export function getNotebookSession() {
  return current;
}
export function notebookTitle(note: Pick<NotebookDraft, "title">) {
  return note.title.trim() || "Untitled note";
}
