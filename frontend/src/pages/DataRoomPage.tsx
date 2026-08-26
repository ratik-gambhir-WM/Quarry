import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import { ChipBankPanel } from "../components/data-room/ChipBankPanel";
import { ConnectSharePointModal } from "../components/data-room/ConnectSharePointModal";
import { DataRoomExplorer } from "../components/data-room/DataRoomExplorer";
import type {
  PreviewState,
  RawTextState,
} from "../components/data-room/DocumentPreviewPanel";
import { EdgePanelOpenButton } from "../components/data-room/EdgePanelOpenButton";
import { ReportEditorPanel } from "../components/data-room/ReportEditorPanel";
import { UploadFilesModal } from "../components/data-room/UploadFilesModal";
import { EmptyState } from "../components/empty-state/empty-state";
import type { DealDocumentSummary } from "../contracts/quarryApi";
import {
  getDealDataRoomView,
  hasDataRoomFiles,
  isUnconfiguredDataRoomError,
} from "../data/dataRoom";
import type { DataRoomTreeNode } from "../data/dataRoom";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { DealExtractionLocationState } from "../data/dealExtraction";
import { buildWorkspaceDealFromExtractionResult } from "../data/dealExtraction";
import { getDealRoomPath } from "../data/workspace";
import { useWorkspaceDeals } from "../hooks/useWorkspaceDeals";
import { useWorkspaceSession } from "../hooks/useWorkspaceSession";

const DocumentPreviewPanel = lazy(() =>
  import("../components/data-room/DocumentPreviewPanel").then((module) => ({
    default: module.DocumentPreviewPanel,
  })),
);

export function DataRoomPage() {
  const { dealId } = useParams();
  const location = useLocation();
  const { deals, loaded } = useWorkspaceDeals();
  const { email } = useWorkspaceSession();
  const [isChipBankOpen, setIsChipBankOpen] = useState(false);
  const [isConnectSharePointModalOpen, setIsConnectSharePointModalOpen] = useState(false);
  const [isUploadModalOpen, setIsUploadModalOpen] = useState(false);
  const [localDataRoom, setLocalDataRoom] = useState<DealDataRoom | null>(null);
  const [dealDocuments, setDealDocuments] = useState<DealDocumentSummary[]>([]);
  const [localTreeError, setLocalTreeError] = useState("");
  const [storedTreeError, setStoredTreeError] = useState("");
  const [localTreeLoading, setLocalTreeLoading] = useState(true);
  const [storedTreeLoading, setStoredTreeLoading] = useState(true);
  const [dataRoomRefreshVersion, setDataRoomRefreshVersion] = useState(0);
  const [selectedDocument, setSelectedDocument] = useState<DataRoomTreeNode | null>(null);
  const [preview, setPreview] = useState<PreviewState>({ status: "loading" });
  const [rawText, setRawText] = useState<RawTextState>({ status: "idle" });
  const previewRequestId = useRef(0);
  const rawTextRequestId = useRef(0);
  const storedDocumentsDealId = useRef<string | undefined>(undefined);
  const storedDocumentsRequestId = useRef(0);
  const extractionResult = (location.state as DealExtractionLocationState | null)?.result;
  const extractedDeal =
    extractionResult && extractionResult.deal.dealId === dealId
      ? buildWorkspaceDealFromExtractionResult(extractionResult)
      : undefined;
  const deal = extractedDeal ?? deals.find((workspaceDeal) => workspaceDeal.room.id === dealId);
  const explorerNodes = useMemo(
    () => [...buildStoredDocumentNodes(dealDocuments), ...(localDataRoom?.tree ?? [])],
    [dealDocuments, localDataRoom?.tree],
  );
  const dataRoomHasFiles = useMemo(() => hasDataRoomFiles(explorerNodes), [explorerNodes]);
  const treeError = [storedTreeError, localTreeError].filter(Boolean).join(" ");
  const treeLoading = storedTreeLoading || localTreeLoading;
  const isUnavailableDataRoom = !treeLoading && Boolean(treeError);
  const isEmptyDataRoom =
    localDataRoom !== null && !treeLoading && !treeError && !dataRoomHasFiles;

  const loadDealDocuments = useCallback(async () => {
    const requestId = storedDocumentsRequestId.current + 1;
    storedDocumentsRequestId.current = requestId;

    if (!dealId) {
      storedDocumentsDealId.current = undefined;
      setDealDocuments([]);
      setStoredTreeError("");
      setStoredTreeLoading(false);
      return;
    }

    if (storedDocumentsDealId.current !== dealId) {
      storedDocumentsDealId.current = dealId;
      setDealDocuments([]);
    }
    setStoredTreeLoading(true);
    setStoredTreeError("");
    try {
      const response = await runtime.api.listDealDocuments(dealId);
      if (storedDocumentsRequestId.current === requestId) {
        setDealDocuments(response);
        setStoredTreeLoading(false);
      }
    } catch (error) {
      if (storedDocumentsRequestId.current === requestId) {
        setDealDocuments([]);
        setStoredTreeError(error instanceof Error ? error.message : String(error));
        setStoredTreeLoading(false);
      }
    }
  }, [dealId]);

  useEffect(() => {
    let cancelled = false;
    void loadDealDocuments();

    if (!dealId) {
      setLocalTreeLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setLocalTreeLoading(true);
    setLocalTreeError("");
    setLocalDataRoom(null);
    setSelectedDocument(null);
    previewRequestId.current += 1;
    rawTextRequestId.current += 1;
    setRawText({ status: "idle" });
    runtime.api.listDealDataRoom(dealId)
      .then((response) => {
        if (!cancelled) {
          setLocalDataRoom(response);
          setLocalTreeLoading(false);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          if (isUnconfiguredDataRoomError(error)) {
            setLocalDataRoom({
              dealId,
              rootName: "Data Room",
              rootPath: "",
              tree: [],
            });
            setLocalTreeLoading(false);
            return;
          }
          setLocalTreeError(error instanceof Error ? error.message : String(error));
          setLocalTreeLoading(false);
        }
      });

    return () => {
      cancelled = true;
      storedDocumentsRequestId.current += 1;
    };
  }, [dataRoomRefreshVersion, dealId, loadDealDocuments]);

  const handleSelectDocument = useCallback(
    async (document: DataRoomTreeNode) => {
      setSelectedDocument(document);
      setPreview({ status: "loading" });
      setRawText({ status: "idle" });
      rawTextRequestId.current += 1;
      const requestId = previewRequestId.current + 1;
      previewRequestId.current = requestId;

      if (!deal?.room.id || (!document.storedFileId && !document.relativePath)) {
        setPreview({ message: "This selection is not a previewable file.", status: "error" });
        return;
      }

      try {
        let response: DocumentPreviewResponse;
        if (document.storedFileId) {
          const pdf = await runtime.api.getDealDocumentPdf(
            deal.room.id,
            document.storedFileId,
          );
          response = {
            fileName: document.name,
            mimeType: pdf.mimeType,
            pdfBytes: pdf.bytes,
            sourceKind: "stored",
          };
        } else {
          response = await runtime.api.previewDealDocument(
            deal.room.id,
            document.relativePath!,
          );
        }
        if (!isDocumentPreviewResponse(response)) {
          throw new Error("The preview backend returned an invalid PDF response.");
        }
        if (previewRequestId.current === requestId) {
          setPreview({ response, status: "ready" });
        }
      } catch (error) {
        if (previewRequestId.current === requestId) {
          setPreview({
            message: error instanceof Error ? error.message : String(error),
            status: "error",
          });
        }
      }
    },
    [deal?.room.id],
  );

  const handleRequestRawText = useCallback(async () => {
    const document = selectedDocument;
    const selectedDealId = deal?.room.id;
    if (!selectedDealId || !document?.storedFileId) {
      setRawText({ message: "Raw text is unavailable for this file.", status: "error" });
      return;
    }

    const requestId = rawTextRequestId.current + 1;
    rawTextRequestId.current = requestId;
    setRawText({ status: "loading" });
    try {
      const response = await runtime.api.getDealDocumentText(
        selectedDealId,
        document.storedFileId,
      );
      if (rawTextRequestId.current === requestId) {
        setRawText({ response, status: "ready" });
      }
    } catch (error) {
      if (rawTextRequestId.current === requestId) {
        setRawText({
          message: error instanceof Error ? error.message : String(error),
          status: "error",
        });
      }
    }
  }, [deal?.room.id, selectedDocument]);

  const handleClosePreview = useCallback(() => {
    previewRequestId.current += 1;
    rawTextRequestId.current += 1;
    setSelectedDocument(null);
  }, []);

  const handleCloseUploadModal = useCallback(() => {
    setIsUploadModalOpen(false);
    void loadDealDocuments();
  }, [loadDealDocuments]);

  const handleRetryDataRoom = useCallback(() => {
    setDataRoomRefreshVersion((version) => version + 1);
  }, []);

  if (!deal && loaded) {
    return <Navigate replace to="/hub" />;
  }

  if (!deal) {
    return <div className="flex min-h-screen items-center justify-center bg-background text-muted">Loading data room…</div>;
  }

  const dataRoomView = getDealDataRoomView(deal.room);

  return (
    <div className="relative min-h-screen overflow-hidden bg-background text-on-surface">
      <div className="workspace-ambient pointer-events-none absolute inset-0 overflow-hidden">
        <div className="absolute left-[15%] top-[22%] h-[22rem] w-[22rem] rounded-full bg-tertiary-fixed/16 blur-3xl" />
        <div className="absolute right-[10%] top-[12%] h-[26rem] w-[26rem] rounded-full bg-primary-fixed/18 blur-3xl" />
        <div className="absolute bottom-[8%] left-[30%] h-[30rem] w-[30rem] rounded-full bg-surface-container-high/70 blur-3xl" />
      </div>

      <div className="relative z-10">
        <div className="relative flex h-screen">
          <DataRoomExplorer
            dealName={deal.room.name}
            dealRoomPath={getDealRoomPath(deal.room.id)}
            email={email}
            key={deal.room.id}
            navigationState={location.state as DealExtractionLocationState | undefined}
            nodes={explorerNodes}
            onConnectToSharePoint={() => setIsConnectSharePointModalOpen(true)}
            onSelectFile={handleSelectDocument}
            onUploadNewFile={() => setIsUploadModalOpen(true)}
            rootPath={localDataRoom?.rootPath}
            selectedFilePath={selectedDocument?.relativePath}
            treeLoading={treeLoading}
          />
          <main className="relative flex min-h-0 min-w-0 flex-1 gap-0 overflow-hidden p-0">
            {isUnavailableDataRoom ? (
              <UnavailableDataRoomState
                onConnectToSharePoint={() => setIsConnectSharePointModalOpen(true)}
                onRetry={handleRetryDataRoom}
              />
            ) : isEmptyDataRoom ? (
              <EmptyDataRoomState
                onConnectToSharePoint={() => setIsConnectSharePointModalOpen(true)}
                onUploadFiles={() => setIsUploadModalOpen(true)}
              />
            ) : (
              <>
                <div className="flex min-h-0 min-w-[420px] flex-1 basis-0 overflow-hidden">
                  {selectedDocument ? (
                    <Suspense fallback={<div className="min-h-0 flex-1 bg-surface-container" />}>
                      <DocumentPreviewPanel
                        document={selectedDocument}
                        key={selectedDocument.id}
                        onClose={handleClosePreview}
                        onRequestRawText={handleRequestRawText}
                        preview={preview}
                        rawText={rawText}
                      />
                    </Suspense>
                  ) : (
                    <ReportEditorPanel
                      blocks={dataRoomView.editorBlocks}
                      reportTitle={dataRoomView.reportTitle}
                      versionLabel={dataRoomView.versionLabel}
                    />
                  )}
                </div>
                {isChipBankOpen ? <ChipBankPanel chips={dataRoomView.chips} onCollapse={() => setIsChipBankOpen(false)} /> : null}
                {!isChipBankOpen ? (
                  <EdgePanelOpenButton
                    label="Open document search"
                    onClick={() => setIsChipBankOpen(true)}
                    side="right"
                  />
                ) : null}
              </>
            )}
          </main>
        </div>
      </div>
      {isUploadModalOpen ? (
        <UploadFilesModal
          dealId={dealId ?? ""}
          onClose={handleCloseUploadModal}
          userId={email ?? ""}
        />
      ) : null}
      {isConnectSharePointModalOpen ? (
        <ConnectSharePointModal onClose={() => setIsConnectSharePointModalOpen(false)} />
      ) : null}
    </div>
  );
}

type EmptyDataRoomStateProps = {
  onConnectToSharePoint: () => void;
  onUploadFiles: () => void;
};

function EmptyDataRoomState({
  onConnectToSharePoint,
  onUploadFiles,
}: EmptyDataRoomStateProps) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-16 shrink-0 items-center border-b border-outline-variant bg-background px-5">
        <h1 className="text-[1rem] font-bold text-text-main [font-family:var(--font-heading)]">
          Data Room Vault
        </h1>
      </header>
      <div className="flex flex-1 items-center justify-center p-8">
        <EmptyState
          action={{ label: "Upload files", onClick: onUploadFiles }}
          className="w-full max-w-[36rem] border-outline-variant/80 bg-surface-container-lowest/70 px-8 py-16"
          description="Add deal documents from your device, or connect SharePoint to bring your data room into Quarry."
          frame="dashed"
          headingLevel={2}
          secondaryAction={{
            label: "Connect SharePoint",
            onClick: onConnectToSharePoint,
          }}
          size="lg"
          title="Upload your first file"
          variant="first-use"
        />
      </div>
    </section>
  );
}

type UnavailableDataRoomStateProps = {
  onConnectToSharePoint: () => void;
  onRetry: () => void;
};

function UnavailableDataRoomState({
  onConnectToSharePoint,
  onRetry,
}: UnavailableDataRoomStateProps) {
  return (
    <section className="glass-panel workspace-pane flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-none border-y-0">
      <header className="flex h-16 shrink-0 items-center border-b border-outline-variant bg-background px-5">
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
          secondaryAction={{
            label: "Connect SharePoint",
            onClick: onConnectToSharePoint,
          }}
          size="lg"
          title="Data room unavailable"
          variant="error"
        />
      </div>
    </section>
  );
}

function isDocumentPreviewResponse(value: unknown): value is DocumentPreviewResponse {
  if (!value || typeof value !== "object") {
    return false;
  }

  const response = value as Partial<DocumentPreviewResponse>;
  const hasPdfBytes = response.pdfBytes instanceof Uint8Array && response.pdfBytes.byteLength > 0;
  const hasPdfBase64 =
    typeof response.pdfBase64 === "string" && response.pdfBase64.length > 0;
  return (
    typeof response.fileName === "string" &&
    response.mimeType === "application/pdf" &&
    (hasPdfBytes || hasPdfBase64) &&
    typeof response.sourceKind === "string"
  );
}

function buildStoredDocumentNodes(documents: DealDocumentSummary[]): DataRoomTreeNode[] {
  if (documents.length === 0) {
    return [];
  }

  return [
    {
      children: documents.map((document) => ({
        id: `stored-document:${document.fileId}`,
        kind: storedDocumentKind(document.displayName),
        name: document.displayName,
        relativePath: `stored-document:${document.fileId}`,
        storedFileId: document.fileId,
      })),
      defaultExpanded: true,
      id: "stored-documents",
      kind: "folder",
      name: "Saved documents",
    },
  ];
}

function storedDocumentKind(displayName: string): DataRoomTreeNode["kind"] {
  const extension = displayName.split(".").pop()?.toLowerCase();
  if (extension === "pdf") {
    return "pdf";
  }
  if (extension === "xls" || extension === "xlsx") {
    return "sheet";
  }
  return "doc";
}
