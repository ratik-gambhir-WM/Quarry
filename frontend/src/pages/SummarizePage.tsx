import { lazy, Suspense, useMemo, useState } from "react";
import { WestMonroeMark } from "../components/brand/WestMonroeMark";
import { WorkspaceHeader } from "../components/hub/WorkspaceHeader";
import { WorkspaceHomeShell } from "../components/hub/WorkspaceHomeShell";
import { ChatPanel } from "../components/summarize/ChatPanel";
import { PanelTab } from "../components/summarize/PanelTab";
import { SummarizeFileTree } from "../components/summarize/SummarizeFileTree";
import { SummarizeSourcePicker } from "../components/summarize/SummarizeSourcePicker";
import { buildFileTree } from "../data/summarize";
import { useSummarizeWorkflow } from "../hooks/useSummarizeWorkflow";

const SummaryPanel = lazy(() =>
  import("../components/summarize/SummaryPanel").then((module) => ({ default: module.SummaryPanel })),
);

type ActivePanel = "chat" | "summary";

export function SummarizePage() {
  const [activePanel, setActivePanel] = useState<ActivePanel>("summary");
  const [expandedFolderIds, setExpandedFolderIds] = useState<Set<string>>(() => new Set());
  const [saveError, setSaveError] = useState("");
  const {
    canSubmit,
    clearSelectedFiles,
    error,
    folderFiles,
    isSummarizing,
    selectAllFiles,
    selectBrowserFiles,
    selectedFilePaths,
    selectedPath,
    summarize,
    summary,
    supportedFileCount,
    toggleSelectedFile,
    updateManualPath,
  } = useSummarizeWorkflow();
  const fileTree = useMemo(() => buildFileTree(folderFiles, selectedPath), [folderFiles, selectedPath]);

  function handleBrowserSelection(fileList: FileList | null, directory: boolean) {
    if (selectBrowserFiles(fileList, directory)) {
      setSaveError("");
      setExpandedFolderIds(new Set());
    }
  }

  function handlePathChange(path: string) {
    setSaveError("");
    updateManualPath(path);
    setExpandedFolderIds(new Set());
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

  return (
    <WorkspaceHomeShell activeHomeSection="summarize" header={<WorkspaceHeader title="Summarize" />}>
      <div className="mx-auto flex w-full max-w-[1120px] flex-col pb-10">
        <SummarizeSourcePicker
          canSubmit={canSubmit}
          isSummarizing={isSummarizing}
          onBrowserSelection={handleBrowserSelection}
          onPathChange={handlePathChange}
          onSubmit={() => {
            setSaveError("");
            void summarize();
          }}
          selectedPath={selectedPath}
        />

        {folderFiles.length > 0 ? (
          <SummarizeFileTree
            expandedFolderIds={expandedFolderIds}
            fileTree={fileTree}
            onClear={clearSelectedFiles}
            onSelectAll={selectAllFiles}
            onToggleFile={toggleSelectedFile}
            onToggleFolder={toggleFolder}
            selectedFileCount={selectedFilePaths.size}
            selectedFilePaths={selectedFilePaths}
            supportedFileCount={supportedFileCount}
          />
        ) : null}

        <div className="mt-5 flex justify-center">
          <div className="inline-flex rounded-full border border-white/85 bg-white/68 p-1 shadow-[0_10px_28px_rgba(7,1,84,0.05)]">
            <PanelTab active={activePanel === "summary"} icon="sparkles" label="Summary" onClick={() => setActivePanel("summary")} />
            <PanelTab active={activePanel === "chat"} icon="send" label="Chat" onClick={() => setActivePanel("chat")} />
          </div>
        </div>

        {activePanel === "summary" ? (
          <div aria-live="polite" className="mt-6 min-h-[calc(100vh-210px)]">
            {error || saveError ? <p className="px-6 text-[13px] font-semibold text-error">{error || saveError}</p> : null}
            {isSummarizing ? <SummaryLoadingState /> : null}
            {summary ? (
              <Suspense fallback={<p className="px-6 text-[13px] text-muted" role="status">Loading summary</p>}>
                <SummaryPanel onError={setSaveError} summary={summary} />
              </Suspense>
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
