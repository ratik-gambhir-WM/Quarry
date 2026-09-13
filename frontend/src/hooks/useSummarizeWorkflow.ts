import { useCallback, useMemo, useRef, useState } from "react";
import { runtime } from "@quarry/runtime";
import {
  getBrowserFilePath,
  toSummarizableFile,
  type SummarizableFile,
} from "../data/summarize";

type SelectedPathKind = "manual" | "file" | "folder";
type SummaryRequestState =
  | { status: "idle" }
  | { status: "submitting" }
  | { status: "success"; summary: string }
  | { message: string; status: "error" };

export function useSummarizeWorkflow() {
  const [browserFiles, setBrowserFiles] = useState<File[]>([]);
  const [folderFiles, setFolderFiles] = useState<SummarizableFile[]>([]);
  const [request, setRequest] = useState<SummaryRequestState>({ status: "idle" });
  const [selectedFilePaths, setSelectedFilePaths] = useState<Set<string>>(() => new Set());
  const [selectedPath, setSelectedPath] = useState("");
  const [selectedPathKind, setSelectedPathKind] = useState<SelectedPathKind>("manual");
  const requestId = useRef(0);
  const supportedFileCount = useMemo(
    () => folderFiles.filter((file) => file.supported).length,
    [folderFiles],
  );

  const selectBrowserFiles = useCallback((fileList: FileList | null, directory: boolean) => {
    const files = Array.from(fileList ?? []);
    if (files.length === 0) return false;

    requestId.current += 1;
    const entries = files.map(toSummarizableFile);
    const firstRelativePath = (files[0] as File & { webkitRelativePath?: string }).webkitRelativePath;
    setBrowserFiles(files);
    setSelectedPath(directory ? firstRelativePath?.split("/")[0] || "Selected folder" : files[0].name);
    setSelectedPathKind(directory ? "folder" : "file");
    setRequest({ status: "idle" });
    setFolderFiles(directory ? entries : []);
    setSelectedFilePaths(new Set(entries.filter((file) => file.supported).map((file) => file.path)));
    return true;
  }, []);

  const updateManualPath = useCallback((path: string) => {
    requestId.current += 1;
    setRequest({ status: "idle" });
    setSelectedPath(path);
    setSelectedPathKind("manual");
    setBrowserFiles([]);
    setFolderFiles([]);
    setSelectedFilePaths(new Set());
  }, []);

  const toggleSelectedFile = useCallback((path: string) => {
    setSelectedFilePaths((current) => {
      const next = new Set(current);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const selectAllFiles = useCallback(() => {
    setSelectedFilePaths(new Set(folderFiles.filter((file) => file.supported).map((file) => file.path)));
  }, [folderFiles]);
  const clearSelectedFiles = useCallback(() => setSelectedFilePaths(new Set()), []);

  const summarize = useCallback(async () => {
    const path = selectedPath.trim();
    const selectedPaths = folderFiles
      .filter((file) => selectedFilePaths.has(file.path))
      .map((file) => file.path);
    if (!path && selectedPaths.length === 0) return;
    if (folderFiles.length > 0 && selectedPaths.length === 0) {
      setRequest({ message: "Select at least one file to summarize.", status: "error" });
      return;
    }

    const currentRequestId = ++requestId.current;
    setRequest({ status: "submitting" });
    try {
      const summary = browserFiles.length > 0
        ? await runtime.api.summarizeUpload(
            browserFiles.filter((file) => folderFiles.length === 0 || selectedFilePaths.has(getBrowserFilePath(file))),
          )
        : folderFiles.length > 0
          ? await runtime.api.summarizeSelected(selectedPaths)
          : selectedPathKind === "file"
            ? await runtime.api.summarizeSelected([path])
            : await runtime.api.summarizePath(path);
      if (requestId.current === currentRequestId) setRequest({ status: "success", summary });
    } catch (error) {
      if (requestId.current === currentRequestId) {
        setRequest({ message: error instanceof Error ? error.message : String(error), status: "error" });
      }
    }
  }, [browserFiles, folderFiles, selectedFilePaths, selectedPath, selectedPathKind]);

  return {
    browserFiles,
    canSubmit: Boolean(selectedPath.trim()) && request.status !== "submitting"
      && (folderFiles.length === 0 || selectedFilePaths.size > 0),
    clearSelectedFiles,
    error: request.status === "error" ? request.message : "",
    folderFiles,
    isSummarizing: request.status === "submitting",
    request,
    selectAllFiles,
    selectBrowserFiles,
    selectedFilePaths,
    selectedPath,
    summarize,
    summary: request.status === "success" ? request.summary : "",
    supportedFileCount,
    toggleSelectedFile,
    updateManualPath,
  };
}
