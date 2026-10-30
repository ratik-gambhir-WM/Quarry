// @vitest-environment happy-dom
import { describe, expect, it } from "vitest";
import { BlockNoteEditor } from "@blocknote/core";
import { notebookSchema } from "@/components/notebook/notebookSchema";
import { createNotebookExport } from "@/components/notebook/exportNotebook";
import type { NotebookDraft } from "@/data/notebook/notebookSession";

function note(title = "Research"): NotebookDraft {
  const editor = BlockNoteEditor.create({
    schema: notebookSchema,
    initialContent: [
      {
        type: "paragraph",
        content: [{ type: "text", text: "Important", styles: { bold: true } }],
      },
      { type: "bulletListItem", content: "Check this" },
    ],
  });
  return {
    id: "note",
    title,
    blocks: editor.document,
    contentSchemaVersion: 1,
    createdAt: "now",
    updatedAt: "now",
  };
}

describe("Notebook exports", () => {
  it("exports the title, formatting, and lists as Markdown without changing the draft", () => {
    const draft = note();
    const original = JSON.stringify(draft);
    const result = createNotebookExport(draft, "markdown");
    expect(result.contents).toContain("# Research");
    expect(result.contents).toContain("**Important**");
    expect(result.contents).toMatch(/[-*] Check this/);
    expect(result.suggestedName).toBe("Research.md");
    expect(result.mimeType).toBe("text/markdown;charset=utf-8");
    expect(JSON.stringify(draft)).toBe(original);
  });
  it("creates standalone HTML and escapes titles instead of interpreting them as markup", () => {
    const result = createNotebookExport(
      note('<script>alert("x")</script>'),
      "html",
    );
    const document = new DOMParser().parseFromString(
      result.contents,
      "text/html",
    );
    expect(document.title).toBe('<script>alert("x")</script>');
    expect(document.querySelector("script")).toBeNull();
    expect(document.querySelector("h1")?.textContent).toBe(
      '<script>alert("x")</script>',
    );
    expect(document.querySelector("strong")?.textContent).toBe("Important");
    expect(document.querySelector("li")?.textContent).toBe("Check this");
    expect(
      document.querySelector('meta[http-equiv="Content-Security-Policy"]'),
    ).toBeTruthy();
    expect(result.extensions).toEqual(["html"]);
  });
  it("sanitizes filenames and uses a neutral title for untitled notes", () => {
    expect(
      createNotebookExport(note("../unsafe/name"), "html").suggestedName,
    ).not.toMatch(/[/\\]/);
    expect(createNotebookExport(note("   "), "markdown").suggestedName).toBe(
      "Untitled note.md",
    );
  });
  it("rejects exports above the shared 5 MB limit", () => {
    const draft = note("x".repeat(6 * 1024 * 1024));
    expect(() => createNotebookExport(draft, "markdown")).toThrow("5 MB");
  });
});
