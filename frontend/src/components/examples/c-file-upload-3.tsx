import { useRef } from "react";

import {
  MAX_PPTX_TEMPLATE_IMPORT_BYTES,
  type PptxTemplateImportMode,
} from "@/contracts/quarryApi";
import { useFileUpload, type FileWithPreview } from "@/hooks/use-file-upload";

import { Button } from "@/components/ui/button";
import { Icon } from "@/components/ui/Icon";

const POWERPOINT_TEMPLATE_ACCEPT = ".pptx";

type PptxTemplateImportButtonsProps = {
  activeMode: PptxTemplateImportMode | null;
  disabled: boolean;
  onError: (message: string) => void;
  onImport: (file: File, mode: PptxTemplateImportMode) => Promise<void>;
};

export function PptxTemplateImportButtons({
  activeMode,
  disabled,
  onError,
  onImport,
}: PptxTemplateImportButtonsProps) {
  const slideTrigger = useRef<HTMLButtonElement>(null);
  const deckTrigger = useRef<HTMLButtonElement>(null);
  const handleFiles = (files: FileWithPreview[], mode: PptxTemplateImportMode) => {
    const file = files[0]?.file;
    if (!(file instanceof File)) return;
    void onImport(file, mode).finally(() => {
      (mode === "single" ? slideTrigger : deckTrigger).current?.focus();
    });
  };
  const [, slideActions] = useFileUpload({
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
    <div className="flex items-center gap-2">
      <input
        {...slideActions.getInputProps({
          "aria-hidden": true,
          className: "hidden",
          disabled,
          tabIndex: -1,
        })}
      />
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
        onClick={slideActions.openFileDialog}
        ref={slideTrigger}
        size="sm"
        type="button"
      >
        <Icon className="size-3.5" name="upload" />
        Import Slide Template
      </Button>
      <Button
        disabled={disabled}
        onClick={deckActions.openFileDialog}
        ref={deckTrigger}
        size="sm"
        type="button"
      >
        <Icon className="size-3.5" name="upload" />
        Import Deck Template
      </Button>
      {activeMode ? (
        <span className="sr-only">Importing {activeMode === "single" ? "slide" : "deck"}</span>
      ) : null}
    </div>
  );
}
