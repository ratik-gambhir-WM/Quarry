import { describe, expect, it } from "vitest";
import { createNotebookSession, getNotebookSession } from "@/data/notebook/notebookSession";
import { sessionStore } from "@/auth/sessionStore";

describe("Notebook session drafts", () => {
  it("keeps edits, sorts on demand, and never recreates deleted notes from late changes", () => {
    const store = createNotebookSession();
    const first = store.create();
    const second = store.create();
    store.update(first.id, { title: "Changed" });
    expect(store.getSnapshot()[0].id).toBe(second.id);
    expect(store.getSnapshot().find((note) => note.id === first.id)?.title).toBe("Changed");
    store.remove(first.id);
    store.update(first.id, { title: "Late editor callback" });
    expect(store.getSnapshot()).toHaveLength(1);
  });
  it("clears drafts on authentication changes and isolates old callbacks", () => {
    const old = getNotebookSession();
    const note = old.create();
    sessionStore.set(null);
    old.update(note.id, { title: "Stale" });
    expect(getNotebookSession().getSnapshot()).toHaveLength(0);
    expect(old.getSnapshot()).toHaveLength(0);
  });
});
