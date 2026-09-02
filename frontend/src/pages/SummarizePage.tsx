import { FormEvent, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { runtime } from "@quarry/runtime";
import { WestMonroeMark } from "../components/brand/WestMonroeMark";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { ChatPanel } from "../components/summarize/ChatPanel";
import { PanelTab } from "../components/summarize/PanelTab";
import { Icon } from "../components/ui/Icon";

type ActivePanel = "chat" | "summary";
type SelectedPathKind = "manual" | "file" | "folder";

type SummarizableFile = {
  filename: string;
  mimeType: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
  supported: boolean;
};

type FileTreeNode = {
  fileCount: number;
  files: SummarizableFile[];
  folders: FileTreeNode[];
  id: string;
  name: string;
  supportedFileCount: number;
};

export function SummarizePage() {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const [activePanel, setActivePanel] = useState<ActivePanel>("summary");
  const [browserFiles, setBrowserFiles] = useState<File[]>([]);
  const [expandedFolderIds, setExpandedFolderIds] = useState<Set<string>>(() => new Set());
  const [error, setError] = useState("");
  const [folderFiles, setFolderFiles] = useState<SummarizableFile[]>([]);
  const [isSummarizing, setIsSummarizing] = useState(false);
  const [selectedPathKind, setSelectedPathKind] = useState<SelectedPathKind>("manual");
  const [summary, setSummary] = useState("");
  const [selectedFilePaths, setSelectedFilePaths] = useState<Set<string>>(() => new Set());
  const [selectedPath, setSelectedPath] = useState("");
  const fileTree = useMemo(() => buildFileTree(folderFiles, selectedPath), [folderFiles, selectedPath]);
  const supportedFileCount = useMemo(() => folderFiles.filter((file) => file.supported).length, [folderFiles]);

  function handleBrowse(directory: boolean) {
    setError("");
    (directory ? folderInputRef : fileInputRef).current?.click();
  }

  function handleBrowserSelection(fileList: FileList | null, directory: boolean) {
    const files = Array.from(fileList ?? []);
    if (files.length === 0) {
      return;
    }

    const entries = files.map(toSummarizableFile);
    const firstRelativePath = (files[0] as File & { webkitRelativePath?: string }).webkitRelativePath;
    const displayPath = directory
      ? firstRelativePath?.split("/")[0] || "Selected folder"
      : files[0].name;
    setBrowserFiles(files);
    setSelectedPath(displayPath);
    setSelectedPathKind(directory ? "folder" : "file");
    setSummary("");
    setExpandedFolderIds(new Set());
    setFolderFiles(directory ? entries : []);
    setSelectedFilePaths(new Set(entries.filter((file) => file.supported).map((file) => file.path)));
  }

  function handlePathChange(path: string) {
    setSelectedPath(path);
    setSelectedPathKind("manual");
    setBrowserFiles([]);
    setExpandedFolderIds(new Set());
    setFolderFiles([]);
    setSelectedFilePaths(new Set());
  }

  function toggleSelectedFile(path: string) {
    setSelectedFilePaths((current) => {
      const next = new Set(current);
      if (next.has(path)) {
        next.delete(path);
      } else {
        next.add(path);
      }
      return next;
    });
  }

  function toggleFolder(folderId: string) {
    setExpandedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(folderId)) {
        next.delete(folderId);
      } else {
        next.add(folderId);
      }
      return next;
    });
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const path = selectedPath.trim();
    const selectedPaths = folderFiles.filter((file) => selectedFilePaths.has(file.path)).map((file) => file.path);
    if (!path && selectedPaths.length === 0) {
      return;
    }
    if (folderFiles.length > 0 && selectedPaths.length === 0) {
      setError("Select at least one file to summarize.");
      return;
    }

    setError("");
    setSummary("");
    setIsSummarizing(true);

    try {
      const result = browserFiles.length > 0
        ? await runtime.api.summarizeUpload(
            browserFiles.filter((file) => {
              const filePath = getBrowserFilePath(file);
              return folderFiles.length === 0 || selectedFilePaths.has(filePath);
            }),
          )
        : folderFiles.length > 0
          ? await runtime.api.summarizeSelected(selectedPaths)
          : selectedPathKind === "file"
            ? await runtime.api.summarizeSelected([path])
            : await runtime.api.summarizePath(path);
      setSummary(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsSummarizing(false);
    }
  }

  async function handleSaveSummary() {
    if (!summary) {
      return;
    }

    setError("");
    try {
      await runtime.platform.saveFile({
        contents: summary,
        extensions: ["md", "markdown"],
        mimeType: "text/markdown;charset=utf-8",
        suggestedName: "summary.md",
        title: "Save markdown summary",
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <WorkspaceHomeShell activeHomeSection="summarize" header={<WorkspaceHeader title="Summarize" />}>
      <div className="mx-auto flex w-full max-w-[1120px] flex-col pb-10">
        <form
          className="flex min-h-16 w-full items-center gap-3 rounded-full border border-white/85 bg-white/82 px-6 py-3 text-text-main shadow-[0_12px_34px_rgba(7,1,84,0.07)] backdrop-blur-md"
          onSubmit={handleSubmit}
        >
          <input
            className="hidden"
            onChange={(event) => handleBrowserSelection(event.target.files, false)}
            ref={fileInputRef}
            type="file"
          />
          <input
            className="hidden"
            multiple
            onChange={(event) => handleBrowserSelection(event.target.files, true)}
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
            onChange={(event) => handlePathChange(event.target.value)}
            placeholder="Search or browse files in Finder..."
            value={selectedPath}
          />
          <button
            className="shrink-0 rounded-full border border-primary/18 bg-primary/8 px-5 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
            onClick={() => handleBrowse(false)}
            type="button"
          >
            Browse File
          </button>
          <button
            className="shrink-0 rounded-full border border-primary/18 bg-primary/8 px-5 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
            onClick={() => handleBrowse(true)}
            type="button"
          >
            Browse Folder
          </button>
          <button
            className="shrink-0 rounded-full bg-action px-5 py-2 text-[13px] font-semibold text-on-action transition enabled:hover:bg-action-hover disabled:cursor-not-allowed disabled:opacity-35"
            disabled={!selectedPath.trim() || isSummarizing || (folderFiles.length > 0 && selectedFilePaths.size === 0)}
            type="submit"
          >
            {isSummarizing ? "Summarizing" : "Submit"}
          </button>
        </form>

        {folderFiles.length > 0 ? (
          <section className="mt-5 overflow-hidden rounded-[13px] border border-white/85 bg-white/74 shadow-[0_12px_34px_rgba(7,1,84,0.05)]">
            <div className="flex flex-wrap items-center justify-between gap-3 border-b border-primary/8 px-5 py-4">
              <div>
                <h2 className="text-[14px] font-semibold text-text-main">Files in folder</h2>
                <p className="mt-1 text-[12px] text-muted">
                  {selectedFilePaths.size} of {supportedFileCount} selected
                </p>
              </div>
              <div className="flex items-center gap-2">
                <button
                  className="rounded-full border border-primary/18 bg-primary/8 px-4 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/12"
                  onClick={() => setSelectedFilePaths(new Set(folderFiles.filter((file) => file.supported).map((file) => file.path)))}
                  type="button"
                >
                  Select All
                </button>
                <button
                  className="rounded-full border border-primary/18 bg-white/75 px-4 py-2 text-[13px] font-semibold text-primary transition hover:bg-primary/8"
                  onClick={() => setSelectedFilePaths(new Set())}
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
                onToggleFile={toggleSelectedFile}
                onToggleFolder={toggleFolder}
                selectedFilePaths={selectedFilePaths}
              />
            </div>
          </section>
        ) : null}

        <div className="mt-5 flex justify-center">
          <div className="inline-flex rounded-full border border-white/85 bg-white/68 p-1 shadow-[0_10px_28px_rgba(7,1,84,0.05)]">
            <PanelTab active={activePanel === "summary"} icon="sparkles" label="Summary" onClick={() => setActivePanel("summary")} />
            <PanelTab active={activePanel === "chat"} icon="send" label="Chat" onClick={() => setActivePanel("chat")} />
          </div>
        </div>

        {activePanel === "summary" ? (
          <div aria-live="polite" className="mt-6 min-h-[calc(100vh-210px)]">
            {error ? <p className="px-6 text-[13px] font-semibold text-error">{error}</p> : null}
            {isSummarizing ? <SummaryLoadingState /> : null}
            {summary ? (
              <div className="vault-markdown relative rounded-[19px] border border-white/85 bg-white/76 p-8 pr-16 text-[16px] leading-7 text-text-main shadow-[0_12px_34px_rgba(7,1,84,0.05)]">
                <button
                  aria-label="Save markdown summary"
                  className="absolute right-5 top-5 flex h-10 w-10 items-center justify-center rounded-full text-primary transition hover:bg-primary/8"
                  onClick={() => void handleSaveSummary()}
                  title="Save markdown summary"
                  type="button"
                >
                  <Icon className="h-5 w-5" name="bookmark" />
                </button>
                <ReactMarkdown remarkPlugins={[remarkGfm]}>{summary}</ReactMarkdown>
              </div>
            ) : null}
          </div>
        ) : (
          <ChatPanel />
        )}
      </div>
    </WorkspaceHomeShell>
  );
}

function SummaryLoadingState() {
  return (
    <div className="flex min-h-[220px] items-center justify-center rounded-[19px] border border-white/85 bg-white/64 shadow-[0_12px_34px_rgba(7,1,84,0.05)] backdrop-blur-md">
      <div className="flex flex-col items-center gap-4 text-primary">
        <div className="relative flex h-20 w-20 animate-[wm-loader-pulse_1.8s_ease-in-out_infinite] items-center justify-center rounded-full bg-surface-container-high shadow-[0_12px_28px_rgba(7,1,84,0.08)] [&_svg]:animate-[wm-loader-spin_1.35s_linear_infinite] [&_svg]:[transform-origin:center]">
          <span className="absolute inset-0 animate-[wm-loader-orbit_1.35s_linear_infinite] rounded-full border border-primary/20" />
          <WestMonroeMark className="h-12 w-12" />
        </div>
        <span className="text-[14px] font-semibold">Summarizing documents...</span>
      </div>
    </div>
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
              if (file.supported) {
                onToggleFile(file.path);
              }
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

const SUPPORTED_SUMMARY_EXTENSIONS = new Set([
  "csv",
  "docx",
  "jpeg",
  "jpg",
  "md",
  "pdf",
  "png",
  "pptx",
  "txt",
  "xls",
  "xlsx",
]);

function getBrowserFilePath(file: File) {
  return (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
}

function toSummarizableFile(file: File): SummarizableFile {
  const path = getBrowserFilePath(file);
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  return {
    filename: file.name,
    mimeType: file.type || "application/octet-stream",
    path,
    relativePath: path,
    sizeBytes: file.size,
    supported: SUPPORTED_SUMMARY_EXTENSIONS.has(extension),
  };
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 bytes";
  }

  const units = ["bytes", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value >= 10 || unitIndex === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unitIndex]}`;
}

function buildFileTree(files: SummarizableFile[], rootPath: string): FileTreeNode {
  const root: FileTreeNode = {
    fileCount: 0,
    files: [],
    folders: [],
    id: "__selected_folder__",
    name: getPathDisplayName(rootPath) || "Selected folder",
    supportedFileCount: 0,
  };

  for (const file of files) {
    const parts = (file.relativePath || file.filename).split(/[\\/]/).filter(Boolean);
    const fileName = parts.pop() || file.filename;
    let current = root;
    const folderPathParts: string[] = [root.id];

    for (const part of parts) {
      folderPathParts.push(part);
      const folderId = folderPathParts.join("/");
      let child = current.folders.find((folder) => folder.id === folderId);
      if (!child) {
        child = {
          fileCount: 0,
          files: [],
          folders: [],
          id: folderId,
          name: part,
          supportedFileCount: 0,
        };
        current.folders.push(child);
      }
      current = child;
    }

    current.files.push({ ...file, filename: fileName });
  }

  finalizeFileTreeNode(root);
  return root;
}

function getPathDisplayName(path: string) {
  const normalized = path.trim().replace(/[\\/]+$/, "");
  if (!normalized) {
    return "";
  }

  const parts = normalized.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || normalized;
}

function finalizeFileTreeNode(node: FileTreeNode): void {
  node.files.sort((left, right) => left.filename.localeCompare(right.filename));
  node.folders.sort((left, right) => left.name.localeCompare(right.name));

  let fileCount = node.files.length;
  let supportedFileCount = node.files.filter((file) => file.supported).length;

  for (const folder of node.folders) {
    finalizeFileTreeNode(folder);
    fileCount += folder.fileCount;
    supportedFileCount += folder.supportedFileCount;
  }

  node.fileCount = fileCount;
  node.supportedFileCount = supportedFileCount;
}
