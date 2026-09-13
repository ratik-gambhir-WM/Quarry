import { useRef } from "react";
import type { FormEvent } from "react";
import { Icon } from "../ui/Icon";

type SummarizeSourcePickerProps = {
  canSubmit: boolean;
  isSummarizing: boolean;
  onBrowserSelection: (files: FileList | null, directory: boolean) => void;
  onPathChange: (path: string) => void;
  onSubmit: () => void;
  selectedPath: string;
};

export function SummarizeSourcePicker({
  canSubmit,
  isSummarizing,
  onBrowserSelection,
  onPathChange,
  onSubmit,
  selectedPath,
}: SummarizeSourcePickerProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    onSubmit();
  }

  return (
    <form
      className="flex min-h-16 w-full items-center gap-3 rounded-full border border-white/85 bg-white/82 px-6 py-3 text-text-main shadow-[0_12px_34px_rgba(7,1,84,0.07)] backdrop-blur-md"
      onSubmit={handleSubmit}
    >
      <input
        className="hidden"
        onChange={(event) => onBrowserSelection(event.target.files, false)}
        ref={fileInputRef}
        type="file"
      />
      <input
        className="hidden"
        multiple
        onChange={(event) => onBrowserSelection(event.target.files, true)}
        ref={(node) => {
          folderInputRef.current = node;
          node?.setAttribute("directory", "");
          node?.setAttribute("webkitdirectory", "");
        }}
        type="file"
      />
      <Icon className="h-6 w-6 shrink-0 text-primary" name="search" />
      <input
        className="min-w-0 flex-1 bg-transparent text-[16px] text-text-main outline-none placeholder:text-muted"
        onChange={(event) => onPathChange(event.target.value)}
        placeholder="Search or browse files in Finder..."
        value={selectedPath}
      />
      <button
        className="shrink-0 rounded-full border border-primary/18 bg-primary/8 px-5 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
        onClick={() => fileInputRef.current?.click()}
        type="button"
      >
        Browse File
      </button>
      <button
        className="shrink-0 rounded-full border border-primary/18 bg-primary/8 px-5 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
        onClick={() => folderInputRef.current?.click()}
        type="button"
      >
        Browse Folder
      </button>
      <button
        className="shrink-0 rounded-full bg-action px-5 py-2 text-[13px] font-semibold text-on-action transition enabled:hover:bg-action-hover disabled:cursor-not-allowed disabled:opacity-35"
        disabled={!canSubmit}
        type="submit"
      >
        {isSummarizing ? "Summarizing" : "Submit"}
      </button>
    </form>
  );
}
