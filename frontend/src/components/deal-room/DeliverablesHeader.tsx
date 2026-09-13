import { PptxTemplateImportButtons } from "../examples/c-file-upload-3";
import { Button } from "../ui/button";
import { Icon } from "../ui/Icon";
import { useTemplatePreviewActions, useTemplatePreviewState } from "./TemplatePreviewStore";

type DeliverablesHeaderProps =
  | { mode: "deliverables"; onViewTemplates: () => void }
  | { mode: "templates"; onBack: () => void };

export function DeliverablesHeader(props: DeliverablesHeaderProps) {
  if (props.mode === "deliverables") {
    return (
      <div className="flex min-w-0 flex-1 items-center justify-between gap-4 self-stretch">
        <h1 className="text-[12px] font-semibold leading-none text-text-main">Deliverables</h1>
        <Button
          className="bg-primary-container text-white hover:bg-primary-container/90"
          onClick={props.onViewTemplates}
          size="sm"
          type="button"
        >
          View Templates
        </Button>
      </div>
    );
  }

  return <TemplatesHeader onBack={props.onBack} />;
}

function TemplatesHeader({ onBack }: { onBack: () => void }) {
  const previewState = useTemplatePreviewState();
  const { importPptxTemplate, reportImportError } = useTemplatePreviewActions();
  const hasTemplates = previewState.status === "success" && previewState.slides.length > 0;
  const activeMode = previewState.status === "success"
    && previewState.activeOperation?.type === "pptxImport"
    ? previewState.activeOperation.importMode
    : null;

  return (
    <div className="flex min-w-0 flex-1 items-center justify-between gap-4 self-stretch">
      <div className="flex min-w-0 items-center gap-3">
        <Button aria-label="Back to Deliverables" onClick={onBack} size="icon-sm" type="button" variant="ghost">
          <Icon className="h-4 w-4" name="chevronLeft" />
        </Button>
        <div className="min-w-0">
          <h1 className="text-sm font-semibold leading-none text-text-main">Templates</h1>
          <p className="mt-1 truncate text-xs leading-none text-muted">Browse available slides to start a deliverable.</p>
        </div>
      </div>
      {hasTemplates ? (
        <PptxTemplateImportButtons
          activeMode={activeMode}
          disabled={previewState.status !== "success" || previewState.activeOperation !== null}
          onError={reportImportError}
          onImport={importPptxTemplate}
        />
      ) : null}
    </div>
  );
}
