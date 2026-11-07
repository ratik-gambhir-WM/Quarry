import { FileText } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { runtime } from "@quarry/runtime";
import type { ChatFileReference } from "../../contracts/quarryApi";
import { loadMentionFiles, type MentionCandidate } from "./fileMentions";

type FileMentionPickerProps = {
  id: string;
  dealId: string | null;
  query: string;
  selected: readonly ChatFileReference[];
  activeIndex: number;
  limitReached: boolean;
  onSelect(file: ChatFileReference): void;
};

type CatalogResult =
  | { status: "ready"; files: MentionCandidate[] }
  | { status: "failed" };

export function FileMentionPicker(props: FileMentionPickerProps) {
  const [attempt, setAttempt] = useState(0);
  return (
    <MentionResults
      key={`${props.dealId ?? "all"}:${attempt}`}
      {...props}
      retry={() => setAttempt(attempt + 1)}
    />
  );
}

function MentionResults({
  id, dealId, query, selected, activeIndex, limitReached, onSelect, retry,
}: FileMentionPickerProps & { retry(): void }) {
  const [result, setResult] = useState<CatalogResult | null>(null);
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let current = true;
    const controller = new AbortController();
    void loadMentionFiles(runtime.api, dealId, controller.signal).then(
      (files) => { if (current) setResult({ status: "ready", files }); },
      () => { if (current) setResult({ status: "failed" }); },
    );
    return () => { current = false; controller.abort(); };
  }, [dealId]);

  const matches = useMemo(() => {
    if (result?.status !== "ready") return [];
    const search = query.toLocaleLowerCase();
    const excluded = new Set(selected.map((file) => `${file.dealId}:${file.fileId}`));
    return result.files.filter((file) =>
      file.name.toLocaleLowerCase().includes(search) &&
      !excluded.has(`${file.dealId}:${file.fileId}`),
    );
  }, [query, result, selected]);

  // Keep the DOM bounded while allowing keyboard access to every match.
  const active = matches.length ? activeIndex % matches.length : 0;
  const start = Math.max(0, active - 25);
  useEffect(() => {
    list.current?.querySelector('[aria-selected="true"]')?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  return (
    <div
      className="absolute bottom-full left-0 z-30 mb-2 w-full rounded-xl border border-outline-variant bg-surface-container-lowest p-2 shadow-lg"
      onMouseDown={(event) => event.preventDefault()}
    >
      <p className="px-2 pb-2 text-xs text-muted">
        {dealId ? "Files in this data room" : "Files across your deals"}
      </p>
      {limitReached ? (
        <p role="status" className="p-2 text-sm text-muted">
          Choose up to 20 uploads and references. Remove a file to add another.
        </p>
      ) : null}
      {!result ? (
        <p role="status" className="p-2 text-sm text-muted">Loading files…</p>
      ) : result.status === "failed" ? (
        <div role="alert" className="p-2 text-sm text-error">
          Couldn’t load files.{" "}
          <button type="button" className="underline focus-visible:outline-2" onClick={retry}>
            Try again
          </button>
        </div>
      ) : matches.length === 0 ? (
        <p role="status" className="p-2 text-sm text-muted">
          {query ? "No matching files." : "No files available."}
        </p>
      ) : null}
      <div
        ref={list}
        id={id}
        role="listbox"
        aria-busy={!result}
        aria-label="Mention a file"
        className="max-h-[min(14rem,40vh)] overflow-y-auto overscroll-contain"
      >
        {matches.slice(start, start + 50).map((file, index) => (
          <button
            id={start + index === active ? `${id}-active` : `${id}-${start + index}`}
            key={`${file.dealId}:${file.fileId}`}
            type="button"
            role="option"
            disabled={limitReached}
            tabIndex={-1}
            aria-selected={start + index === active}
            data-mention-active={start + index === active ? "" : undefined}
            className="flex w-full items-center gap-2 rounded-lg px-2 py-2 text-left text-sm text-on-surface data-[mention-active]:bg-surface-container hover:bg-surface-container focus-visible:outline-2 disabled:opacity-50"
            onClick={() => onSelect({ dealId: file.dealId, fileId: file.fileId, name: file.name })}
          >
            <FileText aria-hidden className="size-4 shrink-0 text-muted" />
            <span className="min-w-0 flex-1">
              <span className="block truncate">{file.name}</span>
              <span className="block truncate text-xs text-muted">{file.dealName}</span>
            </span>
          </button>
        ))}
      </div>
      {matches.length > 50 ? (
        <p className="px-2 pt-1 text-xs text-muted">
          {matches.length} files · Type to narrow results
        </p>
      ) : null}
    </div>
  );
}
