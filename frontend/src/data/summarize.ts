export type SummarizableFile = {
  filename: string;
  mimeType: string;
  path: string;
  relativePath: string;
  sizeBytes: number;
  supported: boolean;
};

export type FileTreeNode = {
  fileCount: number;
  files: SummarizableFile[];
  folders: FileTreeNode[];
  id: string;
  name: string;
  supportedFileCount: number;
};

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

export function getBrowserFilePath(file: File) {
  return (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
}

export function toSummarizableFile(file: File): SummarizableFile {
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

export function formatBytes(bytes: number) {
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

export function buildFileTree(files: SummarizableFile[], rootPath: string): FileTreeNode {
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
