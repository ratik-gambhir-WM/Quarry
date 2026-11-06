import {
  lazy,
  Suspense,
  useCallback,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import type { DocumentSearchResult } from "./document-search/documentSearchModel";
import type { DocumentPreviewPanelHandle } from "./DocumentPreviewPanel";
import { flattenDataRoomFiles, type DataRoomTreeNode } from "../../data/dataRoom";
import type { WorkspaceLocationState } from "../../data/workspace";
import type { DataRoomContents } from "../../hooks/useDataRoomContents";
import { useDocumentSession } from "../../hooks/useDocumentSession";
import { BrowserTabs } from "../tab-browser";
import { EmptyState } from "../empty-state/empty-state";
import { DataRoomExplorer } from "./DataRoomExplorer";
import { FileReviewTable } from "./FileReviewTable";
import { useDataRoomDocumentSearch } from "./dataRoomToolsContext";

const DocumentPreviewPanel = lazy(() =>
  import("./DocumentPreviewPanel").then((module) => ({
    default: module.DocumentPreviewPanel,
  })),
);

type DataRoomWorkspaceProps = {
  contents: DataRoomContents;
  dealId: string;
  dealName: string;
  dealRoomPath: string;
  email?: string;
  navigationState?: WorkspaceLocationState;
  onConnectToSharePoint?: () => void;
  onUploadFiles?: () => void;
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
  const {
    closeDocument,
    documents,
    showFiles,
    selectDocument,
    state: documentSession,
  } = useDocumentSession(dealId);
  const tabsId = useId();
  const mainRef = useRef<HTMLElement>(null);
  const [documentPageCount, setDocumentPageCount] = useState(0);
  const [requestedPreviewPage, setRequestedPreviewPage] = useState<
    number | null
  >(null);
  const [searchBoundaryElement, setSearchBoundaryElement] =
    useState<HTMLDivElement | null>(null);
  const documentPreviewRef = useRef<DocumentPreviewPanelHandle>(null);
  const selectedDocument =
    documentSession.status === "open" ? documentSession.document : null;

  const handleSelectDocument = useCallback(
    (document: DataRoomTreeNode) => {
      if (
        selectedDocument?.id === document.id &&
        documentSession.status === "open" &&
        documentSession.preview.status !== "error"
      )
        return;
      setDocumentPageCount(0);
      setRequestedPreviewPage(null);
      void selectDocument(document);
    },
    [selectDocument, selectedDocument, documentSession],
  );

  const handleClosePreview = useCallback(
    (id?: string) => {
      if (!id || selectedDocument?.id === id) {
        setDocumentPageCount(0);
        setRequestedPreviewPage(null);
      }
      closeDocument(id);
      if (documents.length === 1) {
        requestAnimationFrame(() => mainRef.current?.focus());
      }
    },
    [closeDocument, selectedDocument, documents.length],
  );

  const focusDocumentPreview = useCallback(() => {
    documentPreviewRef.current?.focusViewer();
  }, []);

  const searchContext = useMemo(() => {
    return {
      files: flattenDataRoomFiles(contents.explorerNodes).flatMap(({ node }) =>
        node.storedFileId ? [{ fileId: node.storedFileId, name: node.name }] : [],
      ),
      currentFileId: selectedDocument?.storedFileId,
      boundaryElement: searchBoundaryElement,
      currentFileName: selectedDocument?.name ?? "",
      currentPageCount: documentPageCount,
      onActivateResult: (result: DocumentSearchResult) => {
        if (result.target?.kind === "pdf-page") {
          setRequestedPreviewPage(result.target.page);
        }
      },
      onSelectionFocus: focusDocumentPreview,
    };
  }, [
    selectedDocument,
    contents.explorerNodes,
    searchBoundaryElement,
    documentPageCount,
    focusDocumentPreview,
  ]);
  useDataRoomDocumentSearch(searchContext);

  return (
    <div className="relative flex h-full min-h-0 min-w-0 flex-1">
      {documents.length > 0 ? (
        <div className="contents" data-workspace-sidebar-replacement>
          <DataRoomExplorer
            dealName={dealName}
            dealRoomPath={dealRoomPath}
            email={email}
            navigationState={navigationState}
            nodes={contents.explorerNodes}
            onSelectFile={handleSelectDocument}
            onUploadNewFile={onUploadFiles}
            rootPath={contents.rootPath}
            selectedFilePath={selectedDocument?.relativePath}
            treeLoading={contents.isLoading}
          />
        </div>
      ) : null}
      <main
        id="main-content"
        ref={mainRef}
        tabIndex={-1}
        className="workspace-main-surface relative m-2 flex min-h-0 min-w-0 flex-1 gap-0 overflow-hidden p-0 lg:ml-0"
      >
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
            {documents.length > 0 ? (
              <BrowserTabs
                id={tabsId}
                tabs={[
                  ...documents.map((document) => ({
                    id: document.id,
                    label: document.name,
                  })),
                  ...(documentSession.status === "closed"
                    ? [{ id: "new-tab", label: "New tab" }]
                    : []),
                ]}
                selectedId={selectedDocument?.id ?? "new-tab"}
                onAdd={() => {
                  setDocumentPageCount(0);
                  setRequestedPreviewPage(null);
                  showFiles();
                }}
                onSelect={(id) => {
                  const document = documents.find(
                    (document) => document.id === id,
                  );
                  if (document) handleSelectDocument(document);
                }}
                onClose={(id) => {
                  if (id === "new-tab") {
                    const lastDocument = documents[documents.length - 1];
                    if (lastDocument) handleSelectDocument(lastDocument);
                  } else {
                    handleClosePreview(id);
                  }
                }}
              >
                {documentSession.status === "open" ? (
                  <Suspense
                    fallback={
                      <DocumentPreviewFallback
                        fileName={documentSession.document.name}
                        onClose={() => handleClosePreview()}
                      />
                    }
                  >
                    <DocumentPreviewPanel
                      document={documentSession.document}
                      key={documentSession.document.id}
                      onClose={() => handleClosePreview()}
                      onPageCountChange={setDocumentPageCount}
                      onRetry={() =>
                        void selectDocument(documentSession.document)
                      }
                      onRequestedPageHandled={() =>
                        setRequestedPreviewPage(null)
                      }
                      preview={documentSession.preview}
                      ref={documentPreviewRef}
                      requestedPage={requestedPreviewPage}
                    />
                  </Suspense>
                ) : (
                  <FileReviewTable
                    files={contents.reviewFiles}
                    onSelectFile={handleSelectDocument}
                  />
                )}
              </BrowserTabs>
            ) : (
              <FileReviewTable
                files={contents.reviewFiles}
                onSelectFile={handleSelectDocument}
              />
            )}
          </div>
        )}
        <div
          className={`pointer-events-none absolute inset-x-0 bottom-0 z-50 ${selectedDocument ? "top-12" : "top-0"}`}
          ref={setSearchBoundaryElement}
        />
      </main>
    </div>
  );
}

function DocumentPreviewFallback({
  fileName,
  onClose,
}: {
  fileName: string;
  onClose: () => void;
}) {
  return (
    <section
      aria-busy="true"
      aria-label="Selected document preview"
      className="flex min-h-0 min-w-0 flex-1 flex-col"
      role="region"
    >
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-outline-variant bg-background px-5">
        <span className="truncate text-sm font-semibold text-text-main">
          {fileName}
        </span>
        <button
          aria-label="Close document preview"
          className="text-sm font-semibold text-primary"
          onClick={onClose}
          type="button"
        >
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
  onConnectToSharePoint?: () => void;
  onUploadFiles?: () => void;
}) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center border-b border-outline-variant bg-background px-5">
        <h1 className="text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">
          Data Room Vault
        </h1>
      </header>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          action={
            onUploadFiles
              ? { label: "Upload files", onClick: onUploadFiles }
              : undefined
          }
          className="w-full max-w-[36rem] border-outline-variant/80 bg-surface-container-lowest/70 px-8 py-16"
          description={
            onUploadFiles
              ? "Add deal documents from your device."
              : "No documents have been uploaded. A contributor can upload documents to this deal."
          }
          frame="dashed"
          headingLevel={2}
          secondaryAction={
            onConnectToSharePoint
              ? {
                  label: "Connect SharePoint",
                  onClick: onConnectToSharePoint,
                }
              : undefined
          }
          size="lg"
          title={
            onUploadFiles ? "Upload your first file" : "No shared documents yet"
          }
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
  onConnectToSharePoint?: () => void;
  onRetry: () => void;
}) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-12 shrink-0 items-center border-b border-outline-variant bg-background px-5">
        <h1 className="text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">
          Data Room Vault
        </h1>
      </header>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          action={{ label: "Try again", onClick: onRetry }}
          className="w-full max-w-[36rem] border-outline-variant/80 bg-surface-container-lowest/70 px-8 py-16"
          description="We couldn’t load the files for this data room. Try again in a moment, or connect SharePoint to restore access."
          frame="card"
          headingLevel={2}
          secondaryAction={
            onConnectToSharePoint
              ? {
                  label: "Connect SharePoint",
                  onClick: onConnectToSharePoint,
                }
              : undefined
          }
          size="lg"
          title="Data room unavailable"
          variant="error"
        />
      </div>
    </section>
  );
}
