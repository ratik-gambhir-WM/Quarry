import type { DataRoomTreeNode } from "./dataRoom";

export const FILE_TYPES = ["PDF", "Document", "Spreadsheet", "Presentation", "Markdown", "Image", "Text", "File"] as const;
export type FileType = (typeof FILE_TYPES)[number];

export function getFileExtension(filename: string): string {
  const normalized = filename.trim().toLowerCase();
  const dot = normalized.lastIndexOf(".");
  return dot > 0 ? normalized.slice(dot + 1) : "";
}

export function getFileType(filename: string, fallbackKind?: DataRoomTreeNode["kind"]): FileType {
  const extension = getFileExtension(filename);
  switch (extension) {
    case "pdf": return "PDF";
    case "doc": case "docx": case "odt": case "rtf": return "Document";
    case "xls": case "xlsx": case "xlsm": case "ods": case "csv": case "tsv": return "Spreadsheet";
    case "ppt": case "pptx": case "odp": return "Presentation";
    case "md": case "mdx": case "markdown": return "Markdown";
    case "jpg": case "jpeg": case "png": case "gif": case "webp": case "svg": case "bmp": case "tif": case "tiff": case "avif": case "heic": return "Image";
    case "txt": case "log": return "Text";
    default: return fallbackKind === "pdf" ? "PDF" : fallbackKind === "sheet" ? "Spreadsheet" : "File";
  }
}
