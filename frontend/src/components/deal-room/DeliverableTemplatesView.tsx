import type { PptxTemplateImportMode } from "@/contracts/quarryApi";
import {
  templateDisplayName,
  type DeliverableSlide,
} from "@/data/deliverables";

import { PptxTemplateUploadEmptyState } from "../examples/c-file-upload-4";
import { Button } from "../ui/button";
import { Skeleton } from "../ui/skeleton";
import { Spinner } from "../ui/spinner";
import { DeliverablesCarousel } from "./DeliverablesCarousel";
import {
  useTemplatePreviewActions,
  useTemplatePreviewState,
} from "./TemplatePreviewStore";

type DeliverableTemplatesViewProps = {
  onSelectSlide: (slide: DeliverableSlide) => void;
  onRetry: () => void;
};

export function DeliverableTemplatesView({
  onRetry,
  onSelectSlide,
}: DeliverableTemplatesViewProps) {
  const previewState = useTemplatePreviewState();
  const { deleteTemplate, importPptxTemplate, reload, reportImportError } =
    useTemplatePreviewActions();
  const activeOperation =
    previewState.status === "success" ? previewState.activeOperation : null;
  const activeImportMode =
    activeOperation?.type === "pptxImport" ? activeOperation.importMode : null;

  const handleDelete = (slide: DeliverableSlide) => {
    const displayName = templateDisplayName(slide.id);
    if (!window.confirm(`Delete “${displayName}”? This cannot be undone.`))
      return;
    void deleteTemplate(slide.id);
  };

  return (
    <div className="relative flex h-full min-h-[600px] flex-col">
      <section
        aria-busy={activeImportMode !== null}
        aria-labelledby="deliverable-template-gallery"
        className="flex min-h-0 flex-1 flex-col"
      >
        <h2 className="sr-only" id="deliverable-template-gallery">
          Available slide templates
        </h2>
        <div
          inert={activeImportMode !== null}
          className="flex min-h-0 flex-1 items-center justify-center py-6 text-center"
        >
          {previewState.status === "loading" ? (
            <TemplateCarouselSkeleton />
          ) : null}
          {previewState.status === "error" ? (
            <div className="flex flex-col items-center gap-3">
              <p className="text-sm text-muted">{previewState.message}</p>
              <Button
                onClick={onRetry}
                size="sm"
                type="button"
                variant="outline"
              >
                Retry
              </Button>
            </div>
          ) : null}
          {previewState.status === "success" &&
          previewState.slides.length === 0 ? (
            <div className="flex w-full flex-col items-center gap-3">
              <TemplateActionFeedback onRefresh={reload} />
              {activeOperation?.type === "reload" ? (
                <p
                  aria-live="polite"
                  className="text-sm text-muted"
                  role="status"
                >
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
          {previewState.status === "success" &&
          previewState.slides.length > 0 ? (
            <div className="flex w-full flex-col items-center gap-3">
              {activeOperation?.type === "delete" ? (
                <p className="sr-only" role="status">
                  Deleting {templateDisplayName(activeOperation.templateId)}
                </p>
              ) : null}
              {activeOperation?.type === "reload" ? (
                <p
                  aria-live="polite"
                  className="text-sm text-muted"
                  role="status"
                >
                  Refreshing templates…
                </p>
              ) : null}
              <TemplateActionFeedback onRefresh={reload} />
              <DeliverablesCarousel
                actionsDisabled={activeOperation !== null}
                deletingSlideId={
                  activeOperation?.type === "delete"
                    ? activeOperation.templateId
                    : null
                }
                onDeleteSlide={handleDelete}
                onSelectSlide={onSelectSlide}
                sectionLabel="Template preview carousel"
                slides={previewState.slides}
              />
            </div>
          ) : null}
        </div>
      </section>
      {activeOperation?.type === "pptxImport" ? (
        <TemplateImportProgress
          mode={activeOperation.importMode}
          phase={activeOperation.phase}
        />
      ) : null}
    </div>
  );
}

function TemplateImportProgress({ mode, phase }: {
  mode: PptxTemplateImportMode;
  phase: "saving" | "refreshing";
}) {
  const subject = mode === "single" ? "slide" : "deck";
  const saving = phase === "saving";
  return (
    <div className="template-import-progress absolute inset-0 z-10 flex items-center justify-center bg-surface/90 px-6">
      <div
        aria-label={saving ? `Importing ${subject}` : "Refreshing template gallery"}
        aria-live="polite"
        className="flex w-full max-w-[24rem] flex-col items-center gap-4 rounded-xl bg-surface px-6 py-5 text-center"
        role="status"
      >
        <span
          aria-hidden="true"
          className="flex size-14 items-center justify-center rounded-full border border-outline-variant bg-surface-container-lowest text-primary shadow-sm dark:shadow-none"
        >
          <Spinner className="size-6 motion-reduce:animate-none" />
        </span>
        <div className="space-y-1.5">
          <p className="text-sm font-semibold text-text-main">
            {saving ? `Importing ${subject}…` : "Refreshing gallery…"}
          </p>
          <p className="text-xs leading-relaxed text-muted">
            {saving
              ? "Preparing slide previews and saving your templates."
              : "Your templates are saved. Updating the gallery."}
          </p>
        </div>
      </div>
    </div>
  );
}

function TemplateActionFeedback({
  onRefresh,
}: {
  onRefresh: () => Promise<void>;
}) {
  const previewState = useTemplatePreviewState();
  if (previewState.status !== "success" || !previewState.feedback) return null;
  const feedback = previewState.feedback;
  return (
    <div className="flex flex-wrap items-center justify-center gap-2">
      <p
        className={
          feedback.type === "success"
            ? "text-sm text-muted"
            : "text-sm text-destructive"
        }
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
