import { useEffect, useRef, useState } from "react";
import { ChevronDown, Download, FileCode2, FileText } from "lucide-react";
import { runtime } from "@quarry/runtime";
import type { NotebookDraft } from "@/data/notebook/notebookSession";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { NotebookExportFormat } from "./exportNotebook";

type ExportState =
  | { status: "idle" | "pending" | "success" | "cancelled" }
  | { status: "error"; message: string };

export function NotebookExportMenu({ note }: { note?: NotebookDraft }) {
  const [state, setState] = useState<ExportState>({ status: "idle" });
  const active = useRef(true);
  const exporting = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  async function exportNote(format: NotebookExportFormat) {
    if (!note || exporting.current) return;
    const snapshot = structuredClone(note);
    exporting.current = true;
    setState({ status: "pending" });
    try {
      const { createNotebookExport } = await import("./exportNotebook");
      if (!active.current) return;
      const file = createNotebookExport(snapshot, format);
      const saved = await runtime.platform.saveFile(file);
      if (active.current) setState({ status: saved ? "success" : "cancelled" });
    } catch (error) {
      if (active.current)
        setState({
          status: "error",
          message:
            error instanceof Error
              ? error.message
              : "Could not export this note. Please retry.",
        });
    } finally {
      exporting.current = false;
    }
  }

  return (
    <div className="flex min-w-0 items-center gap-2">
      <span className="sr-only" role="status">
        {state.status === "pending"
          ? "Preparing export…"
          : state.status === "success"
            ? "Export created."
            : state.status === "cancelled"
              ? "Export cancelled."
              : ""}
      </span>
      {state.status === "error" ? (
        <span
          role="alert"
          title={state.message}
          className="max-w-[180px] truncate text-xs text-error"
        >
          {state.message}
        </span>
      ) : null}
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              variant="ghost"
              size="sm"
              className="rounded-full"
              disabled={!note || state.status === "pending"}
            />
          }
        >
          <Download className="size-3.5" />
          {state.status === "pending" ? "Exporting…" : "Export"}
          <ChevronDown className="size-3" />
        </DropdownMenuTrigger>
        <DropdownMenuContent
          align="end"
          className="w-[180px] rounded-[18px] p-1.5"
        >
          <DropdownMenuItem
            className="min-h-[34px] rounded-full px-3 text-[13px] focus:bg-[var(--theme-workspace-chrome)]"
            onSelect={() => {
              void exportNote("markdown");
            }}
          >
            <FileText />
            Markdown
            <span className="ml-auto text-[11px] text-muted-foreground">
              .md
            </span>
          </DropdownMenuItem>
          <DropdownMenuItem
            className="min-h-[34px] rounded-full px-3 text-[13px] focus:bg-[var(--theme-workspace-chrome)]"
            onSelect={() => {
              void exportNote("html");
            }}
          >
            <FileCode2 />
            HTML
            <span className="ml-auto text-[11px] text-muted-foreground">
              .html
            </span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
