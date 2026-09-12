import { templateDisplayName, type DeliverableSlide } from "@/data/deliverables";

import { PptxTemplateUploadEmptyState } from "../examples/c-file-upload-4";
import { Button } from "../ui/button";
import { Skeleton } from "../ui/skeleton";
import { DeliverablesCarousel } from "./DeliverablesCarousel";
import {
  useTemplatePreviewActions,
  useTemplatePreviewState,
} from "./TemplatePreviewStore";

type DeliverableTemplatesViewProps = {
  onRetry: () => void;
};

export function DeliverableTemplatesView({ onRetry }: DeliverableTemplatesViewProps) {
  const previewState = useTemplatePreviewState();
  const {
    deleteTemplate,
    importPptxTemplate,
    reload,
    reportImportError,
  } = useTemplatePreviewActions();
  const activeOperation = previewState.status === "success" ? previewState.activeOperation : null;
  const activeImportMode = activeOperation?.type === "pptxImport" ? activeOperation.importMode : null;

  const handleDelete = (slide: DeliverableSlide) => {
    const displayName = templateDisplayName(slide.id);
    if (!window.confirm(`Delete “${displayName}”? This cannot be undone.`)) return;
    void deleteTemplate(slide.id);
  };

  return (
    <div className="flex h-full min-h-[600px] flex-col">
      <section aria-labelledby="deliverable-template-gallery" className="flex min-h-0 flex-1 flex-col">
        <h2 className="sr-only" id="deliverable-template-gallery">Available slide templates</h2>
        <div className="flex min-h-0 flex-1 items-center justify-center py-6 text-center">
          {previewState.status === "loading" ? <TemplateCarouselSkeleton /> : null}
          {previewState.status === "error" ? (
            <div className="flex flex-col items-center gap-3">
              <p className="text-sm text-muted">{previewState.message}</p>
              <Button onClick={onRetry} size="sm" type="button" variant="outline">
                Retry
              </Button>
            </div>
          ) : null}
          {previewState.status === "success" && previewState.slides.length === 0 ? (
            <div className="flex w-full flex-col items-center gap-3">
              <TemplateActionFeedback onRefresh={reload} />
              {activeOperation?.type === "pptxImport" ? (
                <p aria-live="polite" className="text-sm text-muted" role="status">
                  Importing {activeOperation.importMode === "single" ? "slide" : "deck"}…
                </p>
              ) : null}
              {activeOperation?.type === "reload" ? (
                <p aria-live="polite" className="text-sm text-muted" role="status">
                  Refreshing templates…
                </p>
              ) : null}
              <PptxTemplateUploadEmptyState
                activeMode={activeImportMode}
                disabled={activeOperation !== null}
                onError={reportImportError}
                onImport={importPptxTemplate}
              />
            </div>
          ) : null}
          {previewState.status === "success" && previewState.slides.length > 0 ? (
            <div className="flex w-full flex-col items-center gap-3">
              {activeOperation?.type === "delete" ? (
                <p className="sr-only" role="status">
                  Deleting {templateDisplayName(activeOperation.templateId)}
                </p>
              ) : null}
              {activeOperation?.type === "pptxImport" ? (
                <p aria-live="polite" className="text-sm text-muted" role="status">
                  Importing {activeOperation.importMode === "single" ? "slide" : "deck"}…
                </p>
              ) : null}
              {activeOperation?.type === "reload" ? (
                <p aria-live="polite" className="text-sm text-muted" role="status">
                  Refreshing templates…
                </p>
              ) : null}
              <TemplateActionFeedback onRefresh={reload} />
              <DeliverablesCarousel
                actionsDisabled={activeOperation !== null}
                deletingSlideId={activeOperation?.type === "delete" ? activeOperation.templateId : null}
                onDeleteSlide={handleDelete}
                sectionLabel="Template preview carousel"
                slides={previewState.slides}
              />
            </div>
          ) : null}
        </div>
      </section>
    </div>
  );
}

function TemplateActionFeedback({ onRefresh }: { onRefresh: () => Promise<void> }) {
  const previewState = useTemplatePreviewState();
  if (previewState.status !== "success" || !previewState.feedback) return null;
  const feedback = previewState.feedback;
  return (
    <div className="flex flex-wrap items-center justify-center gap-2">
      <p
        className={feedback.type === "success" ? "text-sm text-muted" : "text-sm text-destructive"}
        role={feedback.type === "success" ? "status" : "alert"}
      >
        {feedback.message}
      </p>
      {feedback.canRefresh ? (
        <Button
          disabled={previewState.activeOperation !== null}
          onClick={() => void onRefresh()}
          size="sm"
          type="button"
          variant="outline"
        >
          Refresh templates
        </Button>
      ) : null}
    </div>
  );
}

function TemplateCarouselSkeleton() {
  return (
    <div className="w-full max-w-[1440px] px-10" role="status">
      <span className="sr-only">Loading template previews</span>
      <div aria-hidden="true" className="-ml-3 flex">
        <div className="min-w-0 shrink-0 grow-0 basis-full pl-3">
          <Skeleton className="aspect-video w-full motion-reduce:animate-none" />
        </div>
      </div>
    </div>
  );
}
