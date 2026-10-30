// @vitest-environment happy-dom
import { BlockNoteEditor } from "@blocknote/core";
import { afterEach, describe, expect, it, vi } from "vitest";
import { notebookSchema } from "@/components/notebook/notebookSchema";
import { startNotebookBlockDrag } from "@/components/notebook/notebookBlockDrag";

const editors: Parameters<typeof startNotebookBlockDrag>[0]["editor"][] = [];
const cancellations: (() => void)[] = [];
afterEach(() => {
  cancellations.splice(0).forEach((cancel) => cancel());
  editors.splice(0).forEach((editor) => editor.unmount());
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

function setup() {
  const host = document.createElement("section");
  host.style.overflowY = "auto";
  document.body.append(host);
  const editor = BlockNoteEditor.create({
    schema: notebookSchema,
    animations: false,
    initialContent: [
      { id: "a", type: "paragraph", content: [{ type: "text", text: "First", styles: { bold: true } }] },
      { id: "b", type: "bulletListItem", content: "Parent", children: [{ id: "child", type: "bulletListItem", content: "Child" }] },
      { id: "c", type: "paragraph", content: "Last" },
    ],
  });
  editor.mount(host);
  editors.push(editor);
  vi.spyOn(host, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 800, 600));
  for (const [id, top, height] of [["a", 100, 40], ["b", 140, 80], ["child", 180, 40], ["c", 220, 40]] as const) {
    const element = host.querySelector<HTMLElement>(`.bn-block-outer[data-id="${id}"]`);
    if (!element) throw new Error(`Missing block ${id}`);
    vi.spyOn(element, "getBoundingClientRect").mockReturnValue(new DOMRect(100, top, 600, height));
  }
  vi.spyOn(window, "requestAnimationFrame").mockReturnValue(1);
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation(() => {});
  const onFinish = vi.fn();
  function start(blockId = "a") {
    const cancel = startNotebookBlockDrag({
      editor, blockId, onFinish,
      event: new PointerEvent("pointerdown", { pointerId: 1, clientX: 90, clientY: 120 }),
    });
    cancellations.push(cancel);
    return cancel;
  }
  function pointer(type: string, y: number, x = 90) {
    document.dispatchEvent(new PointerEvent(type, { pointerId: 1, clientX: x, clientY: y }));
  }
  return { editor, start, pointer, onFinish };
}

describe("Notebook block dragging", () => {
  it("commits once, preserves native content and children, and undoes in one step", () => {
    const { editor, start, pointer, onFinish } = setup();
    const original = editor.document;
    const changed = vi.fn();
    editor.onChange(changed);
    start("b");
    pointer("pointermove", 259);
    expect(editor.document).toEqual(original);
    expect(changed).not.toHaveBeenCalled();
    pointer("pointerup", 259);
    expect(editor.document.map((block) => block.id)).toEqual(["a", "c", "b"]);
    expect(editor.getBlock("b")).toEqual(original[1]);
    expect(changed).toHaveBeenCalledTimes(1);
    expect(onFinish).toHaveBeenCalledWith(true);
    editor.undo();
    expect(editor.document).toEqual(original);
    expect(document.querySelector(".notebook-drag-preview")).toBeNull();
    expect(document.querySelector(".notebook-drop-indicator")).toBeNull();
  });

  it.each(["Escape", "pointercancel", "outside", "unmount", "blur"])("cancels without changing content on %s", (reason) => {
    const { editor, start, pointer, onFinish } = setup();
    const original = editor.document;
    const cancel = start();
    pointer("pointermove", 259);
    if (reason === "Escape") document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    if (reason === "pointercancel") document.dispatchEvent(new PointerEvent("pointercancel"));
    if (reason === "outside") pointer("pointerup", 700, 900);
    if (reason === "unmount") cancel();
    if (reason === "blur") window.dispatchEvent(new Event("blur"));
    expect(editor.document).toEqual(original);
    expect(onFinish).toHaveBeenCalledExactlyOnceWith(false);
    expect(document.querySelector(".notebook-drag-preview")).toBeNull();
    expect(document.querySelector(".notebook-drop-indicator")).toBeNull();
  });

  it("does not start a drag for a click or tiny pointer movement", () => {
    const { editor, start, pointer, onFinish } = setup();
    const original = editor.document;
    start();
    pointer("pointermove", 121);
    pointer("pointerup", 121);
    expect(editor.document).toEqual(original);
    expect(onFinish).toHaveBeenCalledExactlyOnceWith(false);
  });

  it("lifts a nested block without leaving an empty list child", () => {
    const { editor, start, pointer } = setup();
    start("child");
    pointer("pointermove", 259);
    pointer("pointerup", 259);
    expect(editor.document.map((block) => block.id)).toEqual(["a", "b", "c", "child"]);
    expect(editor.getBlock("b")?.children).toEqual([]);
    editor.undo();
    expect(editor.getBlock("b")?.children.map((block) => block.id)).toEqual(["child"]);
  });

  it("moves a selection together and keeps descendants with their parent", () => {
    const { editor, start, pointer } = setup();
    editor.setSelection("a", "b");
    start("b");
    pointer("pointermove", 259);
    pointer("pointerup", 259);
    expect(editor.document.map((block) => block.id)).toEqual(["c", "a", "b"]);
    expect(editor.getBlock("b")?.children.map((block) => block.id)).toEqual(["child"]);
  });

  it("does not dirty the note when dropped in its original position", () => {
    const { editor, start, pointer, onFinish } = setup();
    const changed = vi.fn();
    editor.onChange(changed);
    start();
    pointer("pointermove", 139);
    pointer("pointerup", 139);
    expect(editor.document.map((block) => block.id)).toEqual(["a", "b", "c"]);
    expect(changed).not.toHaveBeenCalled();
    expect(onFinish).toHaveBeenCalledExactlyOnceWith(false);
  });
});
