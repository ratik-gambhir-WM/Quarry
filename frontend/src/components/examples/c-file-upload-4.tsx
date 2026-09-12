import { UploadIcon } from "lucide-react";
import { useRef } from "react";

import {
  MAX_PPTX_TEMPLATE_IMPORT_BYTES,
  type PptxTemplateImportMode,
} from "@/contracts/quarryApi";
import { useFileUpload, type FileWithPreview } from "@/hooks/use-file-upload";
import { cn } from "@/lib/utils";

import { Button } from "@/components/ui/button";

const POWERPOINT_TEMPLATE_ACCEPT = ".pptx";

type PptxTemplateUploadEmptyStateProps = {
  activeMode: PptxTemplateImportMode | null;
  disabled: boolean;
  onError: (message: string) => void;
  onImport: (file: File, mode: PptxTemplateImportMode) => Promise<void>;
};

export function PptxTemplateUploadEmptyState({
  activeMode,
  disabled,
  onError,
  onImport,
}: PptxTemplateUploadEmptyStateProps) {
  const slideTrigger = useRef<HTMLButtonElement>(null);
  const deckTrigger = useRef<HTMLButtonElement>(null);
  const handleFiles = (files: FileWithPreview[], mode: PptxTemplateImportMode) => {
    const file = files[0]?.file;
    if (!(file instanceof File)) return;
    void onImport(file, mode).finally(() => {
      (mode === "single" ? slideTrigger : deckTrigger).current?.focus();
    });
  };
  const [
    { isDragging },
    {
      getInputProps,
      handleDragEnter,
      handleDragLeave,
      handleDragOver,
      handleDrop,
      openFileDialog,
    },
  ] = useFileUpload({
    accept: POWERPOINT_TEMPLATE_ACCEPT,
    maxFiles: 1,
    maxSize: MAX_PPTX_TEMPLATE_IMPORT_BYTES,
    multiple: false,
    onError: (errors) => onError(errors[0] ?? "Choose a valid .pptx PowerPoint file."),
    onFilesAdded: (files) => handleFiles(files, "single"),
  });
  const [, deckActions] = useFileUpload({
    accept: POWERPOINT_TEMPLATE_ACCEPT,
    maxFiles: 1,
    maxSize: MAX_PPTX_TEMPLATE_IMPORT_BYTES,
    multiple: false,
    onError: (errors) => onError(errors[0] ?? "Choose a valid .pptx PowerPoint file."),
    onFilesAdded: (files) => handleFiles(files, "batch"),
  });

  return (
    <div className="w-full max-w-2xl">
      <div
        aria-busy={activeMode !== null}
        aria-disabled={disabled}
        className={cn(
          "relative rounded-xl border border-dashed p-10 text-center transition-colors",
          isDragging
            ? "border-primary bg-primary/5"
            : "border-outline-variant bg-surface-container-lowest hover:border-outline",
        )}
        onDragEnter={disabled ? undefined : handleDragEnter}
        onDragLeave={disabled ? undefined : handleDragLeave}
        onDragOver={disabled ? undefined : handleDragOver}
        onDrop={disabled ? undefined : handleDrop}
      >
        <input
          {...getInputProps({
            "aria-hidden": true,
            className: "hidden",
            disabled,
            tabIndex: -1,
          })}
        />

        <div className="flex flex-col items-center gap-4">
          <div className={cn(
            "flex size-16 items-center justify-center rounded-full",
            isDragging ? "bg-primary/10 text-primary" : "bg-surface-container text-muted",
          )}>
            <UploadIcon className="size-6" />
          </div>
          <div className="space-y-2">
            <h3 className="text-lg font-semibold text-text-main">No slide templates yet</h3>
            <p className="text-sm text-muted">
              Drag and drop one single-slide PowerPoint template here or click to browse.
            </p>
            <p className="text-xs text-muted">PPTX files only, up to 25 MB</p>
          </div>
          <div className="flex flex-wrap items-center justify-center gap-2">
            <Button disabled={disabled} onClick={openFileDialog} ref={slideTrigger} type="button">
              <UploadIcon className="size-4" />
              Import Slide Template
            </Button>
            <input
              {...deckActions.getInputProps({
                "aria-hidden": true,
                className: "hidden",
                disabled,
                tabIndex: -1,
              })}
            />
            <Button
              disabled={disabled}
              onClick={deckActions.openFileDialog}
              ref={deckTrigger}
              type="button"
            >
              <UploadIcon className="size-4" />
              Import Deck Template
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
