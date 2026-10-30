import { BlockNoteEditor } from "@blocknote/core";
import type { SaveFileInput } from "@/contracts/quarryApi";
import {
  notebookTitle,
  type NotebookDraft,
} from "@/data/notebook/notebookSession";
import { notebookSchema } from "./notebookSchema";

export type NotebookExportFormat = "markdown" | "html";
// This detached converter never receives draft content as its own editable document.
const converter = BlockNoteEditor.create({ schema: notebookSchema });
const MAX_EXPORT_BYTES = 5 * 1024 * 1024;

export function createNotebookExport(
  note: NotebookDraft,
  format: NotebookExportFormat,
): SaveFileInput {
  const title = notebookTitle(note);
  const blocks: (typeof notebookSchema.PartialBlock)[] = [
    { type: "heading", props: { level: 1 }, content: title },
    ...note.blocks,
  ];
  const extension = format === "markdown" ? "md" : "html";
  const filename =
    title
      .replace(/[<>:"/\\|?*\u0000-\u001f\u007f]/g, "-")
      .replace(/^\.+/, "")
      .trim()
      .slice(0, 100) || "Untitled note";
  const contents =
    format === "markdown"
      ? `${converter.blocksToMarkdownLossy(blocks)}\n`
      : standaloneHtml(title, converter.blocksToHTMLLossy(blocks));
  if (new TextEncoder().encode(contents).byteLength > MAX_EXPORT_BYTES) {
    throw new Error("This note exceeds the 5 MB export limit.");
  }
  return {
    contents,
    extensions: [extension],
    mimeType:
      format === "markdown"
        ? "text/markdown;charset=utf-8"
        : "text/html;charset=utf-8",
    suggestedName: `${filename}.${extension}`,
    title: `Export note as ${format === "markdown" ? "Markdown" : "HTML"}`,
  };
}

function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function standaloneHtml(title: string, body: string) {
  return `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">
  <title>${escapeHtml(title)}</title>
  <style>
    body { max-width:800px; margin:48px auto; padding:0 24px; font-family:system-ui,sans-serif; line-height:1.6; color:#242424 }
    table { border-collapse:collapse; width:100% }
    td, th { border:1px solid #ddd; padding:8px }
    pre { overflow:auto; padding:16px; background:#f3f3f3 }
    blockquote { border-left:3px solid #ccc; margin-left:0; padding-left:16px }
  </style>
</head>
<body>${body}</body>
</html>
`;
}
