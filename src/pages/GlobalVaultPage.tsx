import { ChangeEvent, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { WorkspaceCard } from "../components/hub/WorkspaceCard";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { Icon } from "../components/ui/Icon";

const emptyMarkdown = "";

type SelectionSummary = {
  fileNames: string[];
  totalCount: number;
};

export function GlobalVaultPage() {
  const [fileSelection, setFileSelection] = useState<SelectionSummary>({ fileNames: [], totalCount: 0 });
  const [directorySelection, setDirectorySelection] = useState<SelectionSummary>({ fileNames: [], totalCount: 0 });

  return (
    <WorkspaceHomeShell activeHomeSection="vault" header={<WorkspaceHeader title="Global Vault" />}>
      <div className="mx-auto flex w-full max-w-[1120px] flex-col gap-6 pb-10">
        <header className="space-y-3">
          <div className="inline-flex w-fit items-center gap-3 rounded-full border border-white/80 bg-white/70 px-4 py-2 text-[12px] font-semibold uppercase tracking-[0.18em] text-primary shadow-[0_8px_20px_rgba(7,1,84,0.05)]">
            <Icon className="h-4 w-4" name="folderOpen" />
            Global Vault
          </div>
          <p className="type-subtle max-w-3xl text-muted">
            Upload files or entire folders from your local file system. When markdown summaries become available,
            they will render below.
          </p>
        </header>

        <WorkspaceCard className="p-8">
          <div className="grid gap-6 lg:grid-cols-2">
            <UploadField
              accept="*"
              helperText="Select one or more files from your device."
              label="Upload files"
              onChange={setFileSelection}
            />
            <DirectoryUploadField
              helperText="Choose a local folder to stage its contents."
              label="Upload folder"
              onChange={setDirectorySelection}
            />
          </div>

          <div className="mt-6 grid gap-4 lg:grid-cols-2">
            <SelectionDetails
              emptyLabel="No files selected"
              selectedItems={fileSelection.fileNames}
              totalCount={fileSelection.totalCount}
            />
            <SelectionDetails
              emptyLabel="No folder selected"
              selectedItems={directorySelection.fileNames}
              totalCount={directorySelection.totalCount}
            />
          </div>
        </WorkspaceCard>

        <WorkspaceCard className="p-8">
          <div className="flex items-center justify-between gap-4">
            <div>
              <p className="text-[12px] font-semibold uppercase tracking-[0.18em] text-muted">Markdown Preview</p>
              <h2 className="type-h2 mt-3 text-text-main">Rendered Summary</h2>
            </div>
            <div className="rounded-full border border-white/80 bg-white/70 px-4 py-2 text-[12px] font-semibold text-muted">
              Pending Data
            </div>
          </div>

          <div className="mt-6 min-h-[220px] rounded-[19px] border border-dashed border-primary/18 bg-white/65 p-6">
            {emptyMarkdown.trim() ? (
              <div className="vault-markdown text-[16px] leading-7 text-text-main">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{emptyMarkdown}</ReactMarkdown>
              </div>
            ) : (
              <div className="flex h-full min-h-[172px] items-center justify-center rounded-[15px] bg-surface-container-low/60 px-6 text-center">
                <p className="text-[16px] text-muted">No data available yet</p>
              </div>
            )}
          </div>
        </WorkspaceCard>
      </div>
    </WorkspaceHomeShell>
  );
}

type UploadFieldProps = {
  accept?: string;
  helperText: string;
  label: string;
  onChange: (selection: SelectionSummary) => void;
};

function UploadField({ accept, helperText, label, onChange }: UploadFieldProps) {
  function handleChange(event: ChangeEvent<HTMLInputElement>) {
    onChange(summarizeSelection(event.target.files));
  }

  return (
    <label className="flex flex-col gap-3">
      <span className="text-[13px] font-semibold uppercase tracking-[0.14em] text-muted">{label}</span>
      <input
        accept={accept}
        className="rounded-[22px] border border-primary/16 bg-white px-5 py-4 text-[14px] text-text-main shadow-[0_8px_20px_rgba(7,1,84,0.04)] file:mr-4 file:rounded-full file:border-0 file:bg-primary/10 file:px-4 file:py-2 file:text-[13px] file:font-semibold file:text-primary hover:file:bg-primary/14"
        multiple
        onChange={handleChange}
        type="file"
      />
      <span className="text-[13px] text-muted">{helperText}</span>
    </label>
  );
}

type DirectoryUploadFieldProps = {
  helperText: string;
  label: string;
  onChange: (selection: SelectionSummary) => void;
};

function DirectoryUploadField({ helperText, label, onChange }: DirectoryUploadFieldProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!inputRef.current) {
      return;
    }

    inputRef.current.setAttribute("directory", "");
    inputRef.current.setAttribute("webkitdirectory", "");
  }, []);

  function handleChange(event: ChangeEvent<HTMLInputElement>) {
    onChange(summarizeSelection(event.target.files));
  }

  return (
    <label className="flex flex-col gap-3">
      <span className="text-[13px] font-semibold uppercase tracking-[0.14em] text-muted">{label}</span>
      <input
        className="rounded-[22px] border border-primary/16 bg-white px-5 py-4 text-[14px] text-text-main shadow-[0_8px_20px_rgba(7,1,84,0.04)] file:mr-4 file:rounded-full file:border-0 file:bg-primary/10 file:px-4 file:py-2 file:text-[13px] file:font-semibold file:text-primary hover:file:bg-primary/14"
        multiple
        onChange={handleChange}
        ref={inputRef}
        type="file"
      />
      <span className="text-[13px] text-muted">{helperText}</span>
    </label>
  );
}

type SelectionDetailsProps = {
  emptyLabel: string;
  selectedItems: string[];
  totalCount: number;
};

function SelectionDetails({ emptyLabel, selectedItems, totalCount }: SelectionDetailsProps) {
  const previewItems = selectedItems.slice(0, 3);
  const remainingCount = Math.max(totalCount - previewItems.length, 0);

  return (
    <div className="rounded-[16px] border border-white/80 bg-white/70 p-5 shadow-[0_8px_20px_rgba(7,1,84,0.04)]">
      {totalCount ? (
        <div className="space-y-3">
          <p className="text-[13px] font-semibold text-text-main">
            {totalCount} {totalCount === 1 ? "item staged" : "items staged"}
          </p>
          <div className="space-y-2 text-[13px] text-muted">
            {previewItems.map((item) => (
              <p key={item}>{item}</p>
            ))}
            {remainingCount ? <p>+ {remainingCount} more</p> : null}
          </div>
        </div>
      ) : (
        <p className="text-[13px] text-muted">{emptyLabel}</p>
      )}
    </div>
  );
}

function summarizeSelection(fileList: FileList | null): SelectionSummary {
  const files = Array.from(fileList ?? []);

  return {
    fileNames: files.map((file) => file.webkitRelativePath || file.name),
    totalCount: files.length,
  };
}
