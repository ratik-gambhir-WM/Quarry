import { SideMenuController, useCreateBlockNote } from "@blocknote/react";
import { BlockNoteView } from "@blocknote/shadcn";
import "@blocknote/shadcn/style.css";
import { notebookSchema, type NotebookBlock } from "./notebookSchema";
import "./notebook.css";
import { NotebookSideMenu } from "./NotebookSideMenu";

export function NotebookEditor({
  blocks,
  onChange,
}: {
  blocks: NotebookBlock[];
  onChange: (blocks: NotebookBlock[]) => void;
}) {
  const editor = useCreateBlockNote({
    schema: notebookSchema,
    animations: !window.matchMedia("(prefers-reduced-motion: reduce)").matches,
    links: {
      isValidLink: (href) => {
        try {
          return ["https:", "http:", "mailto:"].includes(
            new URL(href).protocol,
          );
        } catch {
          return false;
        }
      },
    },
    initialContent: blocks.length ? blocks : undefined,
    pasteHandler: ({ event, defaultPasteHandler }) => {
      if (event.clipboardData?.files.length) return true;
      return defaultPasteHandler();
    },
    _tiptapOptions: {
      editorProps: {
        attributes: { "aria-label": "Note content" },
        handleDOMEvents: {
          drop: (_view, event) => {
            if (!event.dataTransfer?.files.length) return false;
            event.preventDefault();
            return true;
          },
        },
      },
    },
  });
  return (
    <div className="notebook-editor">
      <BlockNoteView
        editor={editor}
        sideMenu={false}
        theme="light"
        className="quarry-notebook-theme"
        applyThemedRoot={(element) =>
          element.classList.add("quarry-notebook-theme")
        }
        onChange={() => onChange(editor.document)}
      >
        <SideMenuController sideMenu={NotebookSideMenu} />
      </BlockNoteView>
    </div>
  );
}
