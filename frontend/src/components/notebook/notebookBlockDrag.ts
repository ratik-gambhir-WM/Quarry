import type { BlockNoteEditor } from "@blocknote/core";
import type { notebookSchema, NotebookBlock } from "./notebookSchema";

type NotebookEditor = BlockNoteEditor<
  typeof notebookSchema.blockSchema,
  typeof notebookSchema.inlineContentSchema,
  typeof notebookSchema.styleSchema
>;

type DropTarget = { id: string; placement: "before" | "after" };

// Pointer movement only updates a DOM preview. The document changes once, on drop,
// so dragging doesn't repeatedly save edit buffers or fill the undo history.
export function startNotebookBlockDrag({
  editor,
  blockId,
  event,
  onFinish,
}: {
  editor: NotebookEditor;
  blockId: string;
  event: PointerEvent;
  onFinish: (moved: boolean) => void;
}): () => void {
  const editorElement = editor.domElement;
  const editorWindow = editorElement?.ownerDocument.defaultView;
  if (!editorElement || !editorWindow) {
    onFinish(false);
    return () => {};
  }
  const root = editorElement;
  const host = root.closest(".notebook-editor") ?? root.parentElement ?? root;
  const document = root.ownerDocument;
  const window = editorWindow;
  const blockElements = () => Array.from(
    root.querySelectorAll<HTMLElement>(".bn-block-outer[data-id]"),
  );
  const source = blockElements().find((element) => element.dataset.id === blockId);
  const block = editor.getBlock(blockId);
  if (!source || !block) {
    onFinish(false);
    return () => {};
  }

  const selected = editor.getSelection()?.blocks;
  const blocks = selected?.some((entry) => entry.id === blockId)
    ? selected : [block];
  const excluded = new Set<string>();
  const exclude = (entry: NotebookBlock) => {
    excluded.add(entry.id);
    entry.children.forEach(exclude);
  };
  blocks.forEach(exclude);
  const sources = blockElements().filter((element) =>
    blocks.some((entry) => entry.id === element.dataset.id),
  );
  let scroller: HTMLElement = root;
  while (
    scroller.parentElement &&
    !/(auto|scroll)/.test(window.getComputedStyle(scroller).overflowY)
  ) {
    scroller = scroller.parentElement;
  }

  const startRect = (sources[0] ?? source).getBoundingClientRect();
  let x = event.clientX;
  let y = event.clientY;
  let preview: HTMLElement | undefined;
  let indicator: HTMLElement | undefined;
  let target: DropTarget | undefined;
  let frame = 0;
  let finished = false;
  let previousTime = 0;
  const listeners = new AbortController();

  function finish(commit: boolean) {
    if (finished) return;
    finished = true;
    window.cancelAnimationFrame(frame);
    listeners.abort();
    preview?.remove();
    indicator?.remove();
    host.classList.remove("notebook-dragging");
    let moved = false;
    try {
      if (
        commit && target &&
        blocks.every((entry) => editor.getBlock(entry.id)) &&
        editor.getBlock(target.id)
      ) {
        const destination = target;
        const parent = editor.getParentBlock(destination.id);
        if (blocks.every((entry) =>
          editor.getParentBlock(entry.id)?.id === parent?.id,
        )) {
          const siblings = parent?.children ?? editor.document;
          const ids = new Set(blocks.map((entry) => entry.id));
          const reordered = siblings
            .filter((entry) => !ids.has(entry.id)).map((entry) => entry.id);
          const index = reordered.indexOf(destination.id) +
            (destination.placement === "after" ? 1 : 0);
          reordered.splice(index, 0, ...blocks.map((entry) => entry.id));
          if (siblings.every((entry, index) => entry.id === reordered[index])) return;
        }
        editor.transact(() => {
          // Read current blocks, preserving edits, IDs, formatting and descendants.
          const current = blocks.map((entry) => editor.getBlock(entry.id))
            .filter((entry): entry is NotebookBlock => !!entry);
          editor.removeBlocks(current);
          editor.insertBlocks(current, destination.id, destination.placement);
          editor.setTextCursorPosition(current[0], "start");
        });
        editor.focus();
        moved = true;
      }
    } finally {
      onFinish(moved);
    }
  }

  function draw(time: number) {
    const elapsed = previousTime ? Math.min(time - previousTime, 32) / 16.67 : 1;
    previousTime = time;
    const viewport = scroller.getBoundingClientRect();
    const top = Math.max(0, viewport.top);
    const bottom = Math.min(window.innerHeight, viewport.bottom);
    const inside = x >= viewport.left - 64 && x <= viewport.right + 64 &&
      y >= top && y <= bottom;
    if (inside) {
      const edge = 48;
      const speed = y < top + edge ? -Math.min(1, (top + edge - y) / edge)
        : y > bottom - edge ? Math.min(1, (y - bottom + edge) / edge) : 0;
      scroller.scrollTop += speed * 12 * elapsed;
    }

    target = undefined;
    let nearest = Infinity;
    let line: { left: number; top: number; width: number } | undefined;
    if (inside) {
      for (const element of blockElements()) {
        if (!element.dataset.id || excluded.has(element.dataset.id)) continue;
        const rect = element.getBoundingClientRect();
        for (const placement of ["before", "after"] as const) {
          const lineY = placement === "before" ? rect.top : rect.bottom;
          // Horizontal proximity disambiguates nested list boundaries.
          const distance = Math.abs(y - lineY) +
            Math.abs(startRect.left + x - event.clientX - rect.left) * 0.15;
          if (distance >= nearest) continue;
          nearest = distance;
          target = { id: element.dataset.id, placement };
          line = { left: rect.left, top: lineY, width: rect.width };
        }
      }
    }
    if (preview) {
      preview.style.transform = `translate3d(${startRect.left + x - event.clientX}px, ${startRect.top + y - event.clientY}px, 0)`;
    }
    if (indicator) {
      indicator.hidden = !line;
      if (line) {
        indicator.style.transform = `translate3d(${line.left}px, ${line.top - 1}px, 0)`;
        indicator.style.width = `${line.width}px`;
      }
    }
    frame = window.requestAnimationFrame(draw);
  }

  function activate() {
    preview = document.createElement("div");
    preview.className = "notebook-editor notebook-drag-preview";
    preview.setAttribute("aria-hidden", "true");
    preview.inert = true;
    preview.style.width = `${startRect.width}px`;
    const content = document.createElement("div");
    content.className = "bn-root quarry-notebook-theme";
    const editorContent = document.createElement("div");
    editorContent.className = "bn-editor";
    sources.forEach((element) => editorContent.append(element.cloneNode(true)));
    // A visual clone must not duplicate editor IDs or focusable controls.
    editorContent.querySelectorAll("[id], [data-id], [contenteditable]")
      .forEach((element) => {
        element.removeAttribute("id");
        element.removeAttribute("data-id");
        element.removeAttribute("contenteditable");
      });
    content.append(editorContent);
    preview.append(content);
    // Keep transient source styling outside ProseMirror's observed document.
    const style = document.createElement("style");
    style.textContent = blocks.map((entry) =>
      `.notebook-dragging .bn-block-outer[data-id="${CSS.escape(entry.id)}"] { opacity: 0.25; }`,
    ).join("\n");
    preview.append(style);
    indicator = document.createElement("div");
    indicator.className = "notebook-drop-indicator";
    indicator.setAttribute("aria-hidden", "true");
    document.body.append(preview, indicator);
    host.classList.add("notebook-dragging");
    frame = window.requestAnimationFrame(draw);
  }

  document.addEventListener("pointermove", (next) => {
    if (next.pointerId !== event.pointerId) return;
    x = next.clientX;
    y = next.clientY;
    if (!preview && Math.hypot(x - event.clientX, y - event.clientY) >= 4) activate();
    if (preview) next.preventDefault();
  }, { signal: listeners.signal, passive: false });
  document.addEventListener("pointerup", (next) => {
    if (next.pointerId !== event.pointerId) return;
    x = next.clientX;
    y = next.clientY;
    if (preview) {
      window.cancelAnimationFrame(frame);
      draw(previousTime);
    }
    finish(true);
  }, { signal: listeners.signal });
  document.addEventListener("pointercancel", () => finish(false), { signal: listeners.signal });
  document.addEventListener("keydown", (key) => {
    if (key.key === "Escape") {
      key.preventDefault();
      finish(false);
    }
  }, { signal: listeners.signal });
  window.addEventListener("blur", () => finish(false), { signal: listeners.signal });
  return () => finish(false);
}
