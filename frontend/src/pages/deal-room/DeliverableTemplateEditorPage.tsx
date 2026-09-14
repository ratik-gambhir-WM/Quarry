import { lazy, Suspense, useRef, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { runtime } from "@quarry/runtime";

import {
  TemplateDocumentProvider,
  useTemplateDocumentActions,
  useTemplateDocumentState,
  type TemplateDocumentState,
} from "@/components/deal-room/TemplateDocumentStore";
import { DeliverablesHeader } from "@/components/deal-room/DeliverablesHeader";
import { WorkspaceMain } from "@/components/hub/WorkspaceLayout";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { getDeliverableTemplatesPath } from "@/data/workspace";

import { useDealRoom } from "../DealRoomPage";

const diligenceCanvasModule = import("@/lib/diligence-canvas/DiligenceCanvas");
const DiligenceCanvas = lazy(() =>
  diligenceCanvasModule.then((module) => ({
    default: module.DiligenceCanvas,
  })),
);

const TEMPLATE_EDITOR_JSON_PANEL_ID = "template-editor-json-panel";

export function DeliverableTemplateEditorPage() {
  const { templateId } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const { deal, navigationState } = useDealRoom();

  if (!templateId) {
    return null;
  }

  const backToTemplates = () => navigate(
    getDeliverableTemplatesPath(deal.room.id),
    { state: navigationState },
  );

  return (
    <TemplateDocumentProvider requestKey={location.key} templateId={templateId}>
      <TemplateEditorWorkspace onBack={backToTemplates} />
    </TemplateDocumentProvider>
  );
}

type ExportFeedback =
  | { status: "idle" }
  | { message: string; status: "error" | "success" }
  | { status: "exporting" };

function TemplateEditorWorkspace({ onBack }: { onBack: () => void }) {
  const state = useTemplateDocumentState();
  const { retry, updateDocument } = useTemplateDocumentActions();
  const [deliverableName, setDeliverableName] = useState("Unnamed");
  const [isJsonOpen, setIsJsonOpen] = useState(false);
  const [exportFeedback, setExportFeedback] = useState<ExportFeedback>({ status: "idle" });
  const exportInFlight = useRef(false);

  const exportPowerPoint = async () => {
    if (state.status !== "success" || exportInFlight.current) return;
    exportInFlight.current = true;
    setExportFeedback({ status: "exporting" });
    try {
      const result = await runtime.api.exportPowerPoint(state.document);
      const saved = await runtime.platform.savePowerPoint({
        dataBase64: result.dataBase64,
        suggestedName: result.fileName,
        title: "Save PowerPoint presentation",
      });
      if (!saved) {
        setExportFeedback({ status: "idle" });
        return;
      }
      const warningMessage = result.warningCount === 0
        ? ""
        : ` with ${result.warningCount} export ${result.warningCount === 1 ? "warning" : "warnings"}`;
      setExportFeedback({
        message: `PowerPoint exported${warningMessage}.`,
        status: "success",
      });
    } catch {
      setExportFeedback({
        message: "The PowerPoint could not be exported. Try again.",
        status: "error",
      });
    } finally {
      exportInFlight.current = false;
    }
  };

  return (
    <WorkspaceMain
      contentClassName="overflow-hidden"
      header={
        <DeliverablesHeader
          deliverableName={deliverableName}
          exportDisabled={state.status !== "success" || exportFeedback.status === "exporting"}
          exportLabel={exportFeedback.status === "exporting" ? "Exporting…" : "Export"}
          isJsonOpen={isJsonOpen}
          jsonPanelId={TEMPLATE_EDITOR_JSON_PANEL_ID}
          mode="editor"
          onBack={onBack}
          onDeliverableNameChange={setDeliverableName}
          onExport={() => { void exportPowerPoint(); }}
          onJsonOpenChange={setIsJsonOpen}
        />
      }
    >
      <TemplateEditorContent
        exportFeedback={exportFeedback}
        isJsonOpen={isJsonOpen}
        onBack={onBack}
        onJsonOpenChange={setIsJsonOpen}
        retry={retry}
        state={state}
        updateDocument={(document) => {
          if (exportFeedback.status !== "exporting") {
            setExportFeedback({ status: "idle" });
          }
          updateDocument(document);
        }}
      />
    </WorkspaceMain>
  );
}

function TemplateEditorContent({
  exportFeedback,
  isJsonOpen,
  onBack,
  onJsonOpenChange,
  retry,
  state,
  updateDocument,
}: {
  exportFeedback: ExportFeedback;
  isJsonOpen: boolean;
  onBack: () => void;
  onJsonOpenChange: (isOpen: boolean) => void;
  retry: () => void;
  state: TemplateDocumentState;
  updateDocument: (document: Extract<TemplateDocumentState, { status: "success" }>["document"]) => void;
}) {
  if (state.status === "loading") {
    return <EditorLoadingStatus label="Loading template document" />;
  }
  if (state.status === "error") {
    return (
      <div className="flex h-full min-h-[32rem] items-center justify-center p-6">
        <div className="max-w-md text-center" role="alert">
          <p className="text-sm text-destructive">{state.message}</p>
          <div className="mt-4 flex justify-center gap-2">
            <Button onClick={retry} size="sm" type="button">Retry</Button>
            <Button onClick={onBack} size="sm" type="button" variant="outline">
              Back to Templates
            </Button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-2 p-3 sm:p-4">
      <div className="flex min-h-4 items-center justify-end text-xs">
        {exportFeedback.status === "exporting" ? (
          <p className="text-muted" role="status">Building PowerPoint…</p>
        ) : exportFeedback.status === "idle" ? null : (
          <p
            className={exportFeedback.status === "error" ? "text-destructive" : "text-muted"}
            role={exportFeedback.status === "error" ? "alert" : "status"}
          >
            {exportFeedback.message}
          </p>
        )}
      </div>
      <Suspense fallback={<EditorLoadingStatus label="Loading presentation editor" />}>
        <DiligenceCanvas
          className="min-h-0 flex-1"
          isJsonOpen={isJsonOpen}
          jsonPanelId={TEMPLATE_EDITOR_JSON_PANEL_ID}
          onChange={updateDocument}
          onJsonOpenChange={onJsonOpenChange}
          showPresentationHeader={false}
          value={state.document}
        />
      </Suspense>
    </div>
  );
}

function EditorLoadingStatus({ label }: { label: string }) {
  return (
    <div className="flex h-full min-h-[32rem] flex-col gap-3 p-6" role="status">
      <span className="sr-only">{label}</span>
      <Skeleton aria-hidden="true" className="h-8 w-64 motion-reduce:animate-none" />
      <Skeleton aria-hidden="true" className="min-h-[28rem] flex-1 motion-reduce:animate-none" />
    </div>
  );
}
