import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { Navigate, useLocation, useParams } from "react-router-dom";
import { ChipBankPanel } from "../components/data-room/ChipBankPanel";
import { DataRoomExplorer } from "../components/data-room/DataRoomExplorer";
import type { PreviewState } from "../components/data-room/DocumentPreviewPanel";
import { EdgePanelOpenButton } from "../components/data-room/EdgePanelOpenButton";
import { ReportEditorPanel } from "../components/data-room/ReportEditorPanel";
import { getDealDataRoomView } from "../data/dataRoom";
import type { DataRoomTreeNode } from "../data/dataRoom";
import type { DealDataRoom, DocumentPreviewResponse } from "../data/dataRoomPreview";
import type { DealExtractionLocationState } from "../data/dealExtraction";
import { buildWorkspaceDealFromExtractionResult } from "../data/dealExtraction";
import { getDealRoomPath } from "../data/workspace";
import { useWorkspaceDeals } from "../hooks/useWorkspaceDeals";
import { productApi } from "../lib/product";
import { UploadFilesModal } from "../components/data-room/UploadFilesModal";
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
  const [isUploadOpen, setIsUploadOpen] = useState(false);
  const [isChipBankOpen, setIsChipBankOpen] = useState(true);
  const [isExplorerOpen, setIsExplorerOpen] = useState(true);
  const [localDataRoom, setLocalDataRoom] = useState<DealDataRoom | null>(null);
  const [treeError, setTreeError] = useState("");
  const [treeLoading, setTreeLoading] = useState(true);
  const [selectedDocument, setSelectedDocument] = useState<DataRoomTreeNode | null>(null);
  const [preview, setPreview] = useState<PreviewState>({ status: "loading" });
  const previewRequestId = useRef(0);
  const extractionResult = (location.state as DealExtractionLocationState | null)?.result;
  const extractedDeal =
    extractionResult && String(extractionResult.deal.id) === dealId
      ? buildWorkspaceDealFromExtractionResult(extractionResult)
      : undefined;
  const deal =
    extractedDeal ?? deals.find((workspaceDeal) => workspaceDeal.room.id === dealId);

  useEffect(() => {
    let cancelled = false;

    if (!dealId) {
      setTreeLoading(false);
      return () => {
        cancelled = true;
      };
    }

    setTreeLoading(true);
    setTreeError("");
    setLocalDataRoom(null);
    setSelectedDocument(null);
    previewRequestId.current += 1;
    productApi
      .listDealDataRoom(dealId)
      .then((response) => {
        if (!cancelled) {
          setLocalDataRoom(response);
          setTreeLoading(false);
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setTreeError(error instanceof Error ? error.message : String(error));
          setTreeLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [dealId]);

  const handleSelectDocument = useCallback(
    async (document: DataRoomTreeNode) => {
      setSelectedDocument(document);
      setPreview({ status: "loading" });
      const requestId = previewRequestId.current + 1;
      previewRequestId.current = requestId;

      if (!deal?.room.id || !document.relativePath) {
        setPreview({ message: "This selection is not a previewable file.", status: "error" });
        return;
      }

      try {
        const response = await productApi.previewDealDocument(
          deal.room.id,
          document.relativePath,
        );
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

  const handleClosePreview = useCallback(() => {
    previewRequestId.current += 1;
    setSelectedDocument(null);
  }, []);

  if (!deal && loaded) {
    return <Navigate replace to="/hub" />;
  }

  if (!deal) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background text-muted">
        Loading data room…
      </div>
    );
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
            collapsed={!isExplorerOpen}
            dealName={deal.room.name}
            dealRoomPath={getDealRoomPath(deal.room.id)}
            key={deal.room.id}
            navigationState={location.state as DealExtractionLocationState | undefined}
            nodes={localDataRoom?.tree ?? []}
            onCollapse={() => setIsExplorerOpen(false)}
            onSelectFile={handleSelectDocument}
            onUploadNewFile={() => setIsUploadOpen(true)}
            rootPath={localDataRoom?.rootPath}
            selectedFilePath={selectedDocument?.relativePath}
            treeError={treeError}
            treeLoading={treeLoading}
          />
          {!isExplorerOpen ? (
            <EdgePanelOpenButton
              label="Open data room sidebar"
              onClick={() => setIsExplorerOpen(true)}
              side="left"
            />
          ) : null}

          <main className="relative flex min-h-0 min-w-0 flex-1 gap-0 overflow-hidden p-0">
            <div className="flex min-h-0 min-w-[420px] flex-1 basis-0 overflow-hidden">
              {selectedDocument ? (
                <Suspense fallback={<div className="min-h-0 flex-1 bg-surface-container" />}>
                  <DocumentPreviewPanel
                    document={selectedDocument}
                    onClose={handleClosePreview}
                    preview={preview}
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
          </main>
        </div>
      </div>
      {isUploadOpen ? (
        <UploadFilesModal onClose={() => setIsUploadOpen(false)} userId={email ?? ""} />
      ) : null}
    </div>
  );
}

function isDocumentPreviewResponse(value: unknown): value is DocumentPreviewResponse {
  if (!value || typeof value !== "object") {
    return false;
  }

  const response = value as Partial<DocumentPreviewResponse>;
  return (
    typeof response.fileName === "string" &&
    response.mimeType === "application/pdf" &&
    typeof response.pdfBase64 === "string" &&
    response.pdfBase64.length > 0 &&
    typeof response.sourceKind === "string"
  );
}
