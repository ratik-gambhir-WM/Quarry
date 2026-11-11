import { describe, expect, it } from "vitest";
import {
  createNotebookSession,
  getNotebookSession,
} from "@/data/notebook/notebookSession";
import { sessionStore } from "@/auth/sessionStore";

describe("Notebook session drafts", () => {
  it("keeps edits, sorts on demand, and never recreates deleted notes from late changes", () => {
    const store = createNotebookSession();
    const first = store.create();
    const second = store.create();
    store.update(first.id, { title: "Changed" });
    expect(store.getSnapshot()[0].id).toBe(second.id);
    expect(
      store.getSnapshot().find((note) => note.id === first.id)?.title,
    ).toBe("Changed");
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

it("keeps catalog identities stable during body edits but publishes title and order changes", () => {
  const store = createNotebookSession();
  const note = store.create();
  const catalog = store.getCatalogSnapshot();
  store.update(note.id, { blocks: [{
    id: "paragraph", type: "paragraph", children: [],
    props: { backgroundColor: "default", textColor: "default", textAlignment: "left" },
    content: [{ type: "text", text: "Text", styles: {} }],
  }] });
  expect(store.getCatalogSnapshot()).toBe(catalog);
  expect(store.getSnapshot()[0].editVersion).toBeGreaterThan(note.editVersion);
  store.update(note.id, { title: "New title" });
  expect(store.getCatalogSnapshot()).not.toBe(catalog);
  expect(store.getCatalogSnapshot()[0].title).toBe("New title");
});

it("restores the saved edit version when undo returns to the canonical content", () => {
  const store = createNotebookSession();
  const original = store.create();
  store.markSaved(original.id, original, original.editVersion);
  store.update(original.id, { title: "Edited" });
  expect(store.getSnapshot()[0].editVersion).not.toBe(original.editVersion);
  store.update(original.id, { title: "  " });
  expect(store.getSnapshot()[0].editVersion).toBe(original.editVersion);
  store.update(original.id, { blocks: [{
    id: "paragraph", type: "paragraph", children: [],
    props: { backgroundColor: "default", textColor: "default", textAlignment: "left" },
    content: [{ type: "text", text: "Text", styles: {} }],
  }] });
  expect(store.getSnapshot()[0].editVersion).not.toBe(original.editVersion);
  store.update(original.id, { blocks: [] });
  expect(store.getSnapshot()[0].editVersion).toBe(original.editVersion);
});
