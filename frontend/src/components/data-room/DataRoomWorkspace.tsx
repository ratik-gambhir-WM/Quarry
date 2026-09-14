import { lazy, Suspense, useCallback, useRef, useState } from "react";
import type { DocumentSearchResult } from "./document-search/documentSearchModel";
import type { DocumentPreviewPanelHandle } from "./DocumentPreviewPanel";
import type { DataRoomTreeNode } from "../../data/dataRoom";
import type { WorkspaceLocationState } from "../../data/workspace";
import type { DataRoomContents } from "../../hooks/useDataRoomContents";
import { useDocumentSession } from "../../hooks/useDocumentSession";
import { EmptyState } from "../empty-state/empty-state";
import { DataRoomArcMenu } from "./DataRoomArcMenu";
import { DataRoomExplorer } from "./DataRoomExplorer";
import { FileReviewTable } from "./FileReviewTable";

const DocumentPreviewPanel = lazy(() =>
  import("./DocumentPreviewPanel").then((module) => ({ default: module.DocumentPreviewPanel })),
);

type DataRoomWorkspaceProps = {
  contents: DataRoomContents;
  dealId: string;
  dealName: string;
  dealRoomPath: string;
  email?: string;
  navigationState?: WorkspaceLocationState;
  onConnectToSharePoint: () => void;
  onUploadFiles: () => void;
};

export function DataRoomWorkspace({
  contents,
  dealId,
  dealName,
  dealRoomPath,
  email,
  navigationState,
  onConnectToSharePoint,
  onUploadFiles,
}: DataRoomWorkspaceProps) {
  const { closeDocument, requestRawText, selectDocument, state: documentSession } = useDocumentSession(dealId);
  const [documentPageCount, setDocumentPageCount] = useState(0);
  const [documentSearchOpen, setDocumentSearchOpen] = useState(false);
  const [requestedPreviewPage, setRequestedPreviewPage] = useState<number | null>(null);
  const [searchPortalContainer, setSearchPortalContainer] = useState<HTMLDivElement | null>(null);
  const documentPreviewRef = useRef<DocumentPreviewPanelHandle>(null);
  const selectedDocument = documentSession.status === "open" ? documentSession.document : null;

  const handleSelectDocument = useCallback((document: DataRoomTreeNode) => {
    setDocumentPageCount(0);
    setDocumentSearchOpen(false);
    setRequestedPreviewPage(null);
    void selectDocument(document);
  }, [selectDocument]);

  const handleClosePreview = useCallback(() => {
    setDocumentPageCount(0);
    setDocumentSearchOpen(false);
    setRequestedPreviewPage(null);
    closeDocument();
  }, [closeDocument]);

  const focusDocumentPreview = useCallback(() => {
    documentPreviewRef.current?.focusViewer();
  }, []);

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-1">
      {selectedDocument ? (
        <div className="contents" data-workspace-sidebar-replacement>
          <DataRoomExplorer
            dealName={dealName}
            dealRoomPath={dealRoomPath}
            email={email}
            navigationState={navigationState}
            nodes={contents.explorerNodes}
            onConnectToSharePoint={onConnectToSharePoint}
            onSelectFile={handleSelectDocument}
            onUploadNewFile={onUploadFiles}
            rootPath={contents.rootPath}
            selectedFilePath={selectedDocument.relativePath}
            treeLoading={contents.isLoading}
          />
        </div>
      ) : null}
      <main className="workspace-main-surface relative m-2 flex min-h-0 min-w-0 flex-1 gap-0 overflow-hidden p-0 lg:ml-0">
        {contents.isUnavailable ? (
          <UnavailableDataRoomState
            onConnectToSharePoint={onConnectToSharePoint}
            onRetry={contents.reloadAll}
          />
        ) : contents.isEmpty ? (
          <EmptyDataRoomState
            onConnectToSharePoint={onConnectToSharePoint}
            onUploadFiles={onUploadFiles}
          />
        ) : (
          <div className="flex min-h-0 min-w-0 flex-1 basis-0 overflow-hidden">
            {documentSession.status === "open" ? (
              <Suspense
                fallback={
                  <DocumentPreviewFallback
                    fileName={documentSession.document.name}
                    onClose={handleClosePreview}
                  />
                }
              >
                <DocumentPreviewPanel
                  document={documentSession.document}
                  key={documentSession.document.id}
                  onClose={handleClosePreview}
                  onPageCountChange={setDocumentPageCount}
                  onRequestRawText={() => void requestRawText()}
                  onRequestedPageHandled={() => setRequestedPreviewPage(null)}
                  preview={documentSession.preview}
                  rawText={documentSession.rawText}
                  ref={documentPreviewRef}
                  requestedPage={requestedPreviewPage}
                />
              </Suspense>
            ) : (
              <FileReviewTable files={contents.reviewFiles} onSelectFile={handleSelectDocument} />
            )}
          </div>
        )}
        <div
          className={`pointer-events-none absolute inset-x-0 bottom-0 z-50 ${selectedDocument ? "top-12" : "top-0"}`}
          ref={setSearchPortalContainer}
        />
        <DataRoomArcMenu
          documentSearch={{
            currentFileName: selectedDocument?.name ?? "Data Room",
            currentPageCount: selectedDocument ? documentPageCount : 0,
            onActivateResult: (result: DocumentSearchResult) => {
              if (result.target?.kind === "pdf-page") setRequestedPreviewPage(result.target.page);
            },
            onOpenChange: setDocumentSearchOpen,
            onSelectionFocus: selectedDocument ? focusDocumentPreview : undefined,
            portalContainer: searchPortalContainer,
          }}
          documentSearchOpen={documentSearchOpen}
        />
      </main>
    </div>
  );
}

function DocumentPreviewFallback({ fileName, onClose }: { fileName: string; onClose: () => void }) {
  return (
    <section aria-busy="true" aria-label="Selected document preview" className="flex min-h-0 min-w-0 flex-1 flex-col" role="region">
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-outline-variant bg-background px-5">
        <span className="truncate text-sm font-semibold text-text-main">{fileName}</span>
        <button aria-label="Close document preview" className="text-sm font-semibold text-primary" onClick={onClose} type="button">
          Close
        </button>
      </header>
      <div className="flex flex-1 items-center justify-center" role="status">
        <span className="text-sm text-muted">Loading document viewer</span>
      </div>
    </section>
  );
}

function EmptyDataRoomState({
  onConnectToSharePoint,
  onUploadFiles,
}: {
  onConnectToSharePoint: () => void;
  onUploadFiles: () => void;
}) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center border-b border-outline-variant bg-background px-5">
        <h1 className="text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">Data Room Vault</h1>
      </header>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          action={{ label: "Upload files", onClick: onUploadFiles }}
          className="w-full max-w-[36rem] border-outline-variant/80 bg-surface-container-lowest/70 px-8 py-16"
          description="Add deal documents from your device, or connect SharePoint to bring your data room into Quarry."
          frame="dashed"
          headingLevel={2}
          secondaryAction={{ label: "Connect SharePoint", onClick: onConnectToSharePoint }}
          size="lg"
          title="Upload your first file"
          variant="first-use"
        />
      </div>
    </section>
  );
}

function UnavailableDataRoomState({
  onConnectToSharePoint,
  onRetry,
}: {
  onConnectToSharePoint: () => void;
  onRetry: () => void;
}) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center border-b border-outline-variant bg-background px-5">
        <h1 className="text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">Data Room Vault</h1>
      </header>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          action={{ label: "Try again", onClick: onRetry }}
          className="w-full max-w-[36rem] border-outline-variant/80 bg-surface-container-lowest/70 px-8 py-16"
          description="We couldn’t load the files for this data room. Try again in a moment, or connect SharePoint to restore access."
          frame="card"
          headingLevel={2}
          secondaryAction={{ label: "Connect SharePoint", onClick: onConnectToSharePoint }}
          size="lg"
          title="Data room unavailable"
          variant="error"
        />
      </div>
    </section>
  );
}
