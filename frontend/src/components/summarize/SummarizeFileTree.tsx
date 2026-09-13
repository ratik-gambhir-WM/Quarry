import type { FileTreeNode, SummarizableFile } from "../../data/summarize";
import { formatBytes } from "../../data/summarize";
import { Icon } from "../ui/Icon";

type SummarizeFileTreeProps = {
  expandedFolderIds: Set<string>;
  fileTree: FileTreeNode;
  onClear: () => void;
  onSelectAll: () => void;
  onToggleFile: (path: string) => void;
  onToggleFolder: (folderId: string) => void;
  selectedFileCount: number;
  selectedFilePaths: Set<string>;
  supportedFileCount: number;
};

export function SummarizeFileTree({
  expandedFolderIds,
  fileTree,
  onClear,
  onSelectAll,
  onToggleFile,
  onToggleFolder,
  selectedFileCount,
  selectedFilePaths,
  supportedFileCount,
}: SummarizeFileTreeProps) {
  return (
    <section className="mt-5 overflow-hidden rounded-[13px] border border-white/85 bg-white/74 shadow-[0_12px_34px_rgba(7,1,84,0.05)]">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-primary/8 px-5 py-4">
        <div>
          <h2 className="text-[14px] font-semibold text-text-main">Files in folder</h2>
          <p className="mt-1 text-[12px] text-muted">
            {selectedFileCount} of {supportedFileCount} selected
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="rounded-full border border-primary/18 bg-primary/8 px-4 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
            onClick={onSelectAll}
            type="button"
          >
            Select All
          </button>
          <button
            className="rounded-full border border-primary/18 bg-white/75 px-4 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/8"
            onClick={onClear}
            type="button"
          >
            Clear
          </button>
        </div>
      </div>
      <div className="max-h-[300px] overflow-y-auto">
        <FolderTreeRows
          expandedFolderIds={expandedFolderIds}
          files={[]}
          folders={[fileTree]}
          level={0}
          onToggleFile={onToggleFile}
          onToggleFolder={onToggleFolder}
          selectedFilePaths={selectedFilePaths}
        />
      </div>
    </section>
  );
}

type FolderTreeRowsProps = {
  expandedFolderIds: Set<string>;
  files: SummarizableFile[];
  folders: FileTreeNode[];
  level: number;
  onToggleFile: (path: string) => void;
  onToggleFolder: (folderId: string) => void;
  selectedFilePaths: Set<string>;
};

function FolderTreeRows({
  expandedFolderIds,
  files,
  folders,
  level,
  onToggleFile,
  onToggleFolder,
  selectedFilePaths,
}: FolderTreeRowsProps) {
  return (
    <>
      {folders.map((folder) => {
        const expanded = expandedFolderIds.has(folder.id);
        return (
          <div key={folder.id}>
            <button
              aria-expanded={expanded}
              className="flex min-h-11 w-full items-center gap-2 border-b border-primary/6 py-2.5 pr-5 text-left transition hover:bg-primary/5"
              onClick={() => onToggleFolder(folder.id)}
              style={{ paddingLeft: `${20 + level * 18}px` }}
              type="button"
            >
              <Icon className="h-4 w-4 shrink-0 text-muted" name={expanded ? "chevronDown" : "chevronRight"} />
              <Icon className="h-5 w-5 shrink-0 text-primary/75" name="folderOpen" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[14px] font-semibold text-text-main">{folder.name}</span>
                <span className="mt-0.5 block text-[12px] text-muted">
                  {folder.supportedFileCount} of {folder.fileCount} supported
                </span>
              </span>
            </button>
            {expanded ? (
              <FolderTreeRows
                expandedFolderIds={expandedFolderIds}
                files={folder.files}
                folders={folder.folders}
                level={level + 1}
                onToggleFile={onToggleFile}
                onToggleFolder={onToggleFolder}
                selectedFilePaths={selectedFilePaths}
              />
            ) : null}
          </div>
        );
      })}
      {files.map((file) => {
        const selected = selectedFilePaths.has(file.path);
        return (
          <button
            aria-disabled={!file.supported}
            aria-pressed={selected}
            className={`flex min-h-12 w-full items-center gap-3 border-b border-primary/6 py-2.5 pr-5 text-left transition last:border-b-0 ${
              file.supported ? "hover:bg-primary/5" : "cursor-not-allowed opacity-60"
            }`}
            key={file.path}
            onClick={() => {
              if (file.supported) onToggleFile(file.path);
            }}
            style={{ paddingLeft: `${44 + level * 18}px` }}
            type="button"
          >
            <span
              aria-hidden="true"
              className={`flex h-5 w-5 shrink-0 items-center justify-center rounded border transition ${
                selected ? "border-emerald-500 bg-emerald-500 text-white" : "border-primary/20 bg-white/80 text-transparent"
              }`}
            >
              <Icon className="h-3.5 w-3.5" name="check" />
            </span>
            <FileTypeIcon mimeType={file.mimeType} />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[14px] font-semibold text-text-main">{file.filename}</span>
              <span className="mt-0.5 block text-[12px] text-muted">
                {formatBytes(file.sizeBytes)}
                {file.supported ? "" : " · Unsupported"}
              </span>
            </span>
          </button>
        );
      })}
    </>
  );
}

function FileTypeIcon({ mimeType }: { mimeType: string }) {
  const iconName: "pdf" | "sheet" | "image" | "doc" =
    mimeType === "application/pdf"
      ? "pdf"
      : mimeType.includes("spreadsheet") || mimeType.includes("excel") || mimeType === "text/csv"
        ? "sheet"
        : mimeType.includes("image")
          ? "image"
          : "doc";

  return <Icon className="h-5 w-5 shrink-0 text-primary/75" name={iconName} />;
}
