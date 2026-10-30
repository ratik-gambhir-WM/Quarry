import { useEffect, useRef, useState } from "react";
import { GripVertical, Plus } from "lucide-react";
import { SideMenuExtension, SuggestionMenu } from "@blocknote/core/extensions";
import {
  SideMenu,
  useBlockNoteEditor,
  useExtension,
  useExtensionState,
} from "@blocknote/react";
import type { notebookSchema } from "./notebookSchema";
import { startNotebookBlockDrag } from "./notebookBlockDrag";

export function NotebookSideMenu() {
  const editor = useBlockNoteEditor<
    typeof notebookSchema.blockSchema,
    typeof notebookSchema.inlineContentSchema,
    typeof notebookSchema.styleSchema
  >();
  const sideMenu = useExtension(SideMenuExtension);
  const suggestionMenu = useExtension(SuggestionMenu);
  const block = useExtensionState(SideMenuExtension, {
    selector: (state) => state?.block,
  });
  const cancelDrag = useRef<(() => void) | undefined>(undefined);
  const [announcement, setAnnouncement] = useState("");
  useEffect(() => () => cancelDrag.current?.(), []);

  if (!block) return null;

  return (
    <SideMenu>
      <button
        type="button"
        className="notebook-block-button notebook-block-add"
        aria-label="Add block"
        title="Add block"
        onClick={() => {
          const current = editor.getBlock(block.id);
          if (!current) return;
          const empty = Array.isArray(current.content) && current.content.length === 0;
          const target = empty ? current : editor.insertBlocks(
            [{ type: "paragraph" }], current, "after",
          )[0];
          editor.setTextCursorPosition(target);
          suggestionMenu.openSuggestionMenu("/");
        }}
      >
        <Plus size={16} aria-hidden="true" />
      </button>
      <button
        type="button"
        className="notebook-block-button notebook-block-grip"
        aria-label="Move block"
        aria-description="Drag to move. Use Arrow Up or Arrow Down to reorder with the keyboard."
        title="Drag to move"
        onFocus={() => sideMenu.freezeMenu()}
        onBlur={() => sideMenu.unfreezeMenu()}
        onPointerDown={(event) => {
          if (event.button !== 0 || !event.isPrimary) return;
          event.preventDefault();
          cancelDrag.current?.();
          sideMenu.freezeMenu();
          cancelDrag.current = startNotebookBlockDrag({
            editor,
            blockId: block.id,
            event: event.nativeEvent,
            onFinish: (moved) => {
              sideMenu.unfreezeMenu();
              if (moved) setAnnouncement("Block moved.");
            },
          });
        }}
        onKeyDown={(event) => {
          if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
          event.preventDefault();
          if (event.key === "ArrowUp") editor.moveBlocksUp(block.id);
          else editor.moveBlocksDown(block.id);
          setAnnouncement(`Block moved ${event.key === "ArrowUp" ? "up" : "down"}.`);
        }}
      >
        <GripVertical size={16} aria-hidden="true" />
      </button>
      <span className="sr-only" role="status">
        {announcement}
      </span>
    </SideMenu>
  );
}
