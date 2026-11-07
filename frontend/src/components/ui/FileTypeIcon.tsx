import { getFileExtension, getFileType, type FileType } from "@/data/fileTypes";
import type { DataRoomTreeNode } from "@/data/dataRoom";
import { cn } from "@/lib/utils";
import pdf from "@/assets/file-types/fluent/pdf.svg";
import word from "@/assets/file-types/fluent/docx.svg";
import excel from "@/assets/file-types/fluent/xlsx.svg";
import powerpoint from "@/assets/file-types/fluent/pptx.svg";
import markdown from "@/assets/file-types/fluent/md.svg";
import image from "@/assets/file-types/fluent/photo.svg";
import text from "@/assets/file-types/fluent/txt.svg";
import generic from "@/assets/file-types/fluent/genericfile.svg";
import csv from "@/assets/file-types/fluent/csv.svg";
import spreadsheet from "@/assets/file-types/fluent/spreadsheet.svg";
import document from "@/assets/file-types/fluent/rtf.svg";
import presentation from "@/assets/file-types/fluent/presentation.svg";

const icons: Record<FileType, string> = {
  PDF: pdf,
  Document: word,
  Spreadsheet: excel,
  Presentation: powerpoint,
  Markdown: markdown,
  Image: image,
  Text: text,
  File: generic,
};

/** Decorative format mark; the adjacent filename and type label supply its accessible text. */
export function FileTypeIcon({ filename, kind, className }: {
  filename: string;
  kind?: DataRoomTreeNode["kind"];
  className?: string;
}) {
  const type = getFileType(filename, kind);
  const extension = getFileExtension(filename);
  const source = extension === "csv" ? csv
    : extension === "ods" || extension === "tsv" ? spreadsheet
      : extension === "odt" || extension === "rtf" ? document
        : extension === "odp" ? presentation : icons[type];
  return <img alt="" aria-hidden="true" data-file-type={type} draggable={false}
    className={cn("size-8 shrink-0 object-contain", className)} src={source} width={32} height={32} />;
}
