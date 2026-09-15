import { useState } from "react";

import { PptxTemplateImportButtons } from "../examples/c-file-upload-3";
import { Button } from "../ui/button";
import { Icon } from "../ui/Icon";
import { Input } from "../ui/input";
import { useTemplatePreviewActions, useTemplatePreviewState } from "./TemplatePreviewStore";

type DeliverablesHeaderProps =
  | { mode: "deliverables"; onViewTemplates: () => void }
  | { mode: "templates"; onBack: () => void }
  | {
    deliverableName: string;
    exportDisabled: boolean;
    exportLabel: string;
    isJsonOpen: boolean;
    jsonPanelId: string;
    mode: "editor";
    onBack: () => void;
    onDeliverableNameChange: (name: string) => void;
    onExport: () => void;
    onJsonOpenChange: (isOpen: boolean) => void;
  };

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

  if (props.mode === "templates") {
    return <TemplatesHeader onBack={props.onBack} />;
  }
  return (
    <div className="flex min-w-0 flex-1 items-center justify-between gap-4 self-stretch">
      <div className="flex min-w-0 items-center gap-3">
        <Button aria-label="Back to Templates" onClick={props.onBack} size="icon-sm" type="button" variant="ghost">
          <Icon className="h-4 w-4" name="chevronLeft" />
        </Button>
        <EditableDeliverableName
          name={props.deliverableName}
          onChange={props.onDeliverableNameChange}
        />
      </div>
      <div className="flex items-center gap-2">
        <Button
          aria-busy={props.exportLabel !== "Export"}
          disabled={props.exportDisabled}
          onClick={props.onExport}
          size="sm"
          type="button"
        >
          {props.exportLabel}
        </Button>
        <Button
          aria-controls={props.jsonPanelId}
          aria-expanded={props.isJsonOpen}
          onClick={() => props.onJsonOpenChange(!props.isJsonOpen)}
          size="sm"
          type="button"
        >
          {props.isJsonOpen ? "Hide JSON" : "Show JSON"}
        </Button>
      </div>
    </div>
  );
}

type DeliverableNameEditState =
  | { status: "display" }
  | { draft: string; status: "editing" };

function EditableDeliverableName({
  name,
  onChange,
}: {
  name: string;
  onChange: (name: string) => void;
}) {
  const [editState, setEditState] = useState<DeliverableNameEditState>({ status: "display" });

  if (editState.status === "editing") {
    const commit = () => {
      onChange(editState.draft);
      setEditState({ status: "display" });
    };

    return (
      <Input
        aria-label="Deliverable name"
        autoComplete="off"
        autoFocus
        className="w-56 max-w-[40vw] rounded-sm px-1.5 text-sm font-semibold text-text-main"
        maxLength={80}
        onBlur={commit}
        onChange={(event) => setEditState({
          draft: event.currentTarget.value,
          status: "editing",
        })}
        onFocus={(event) => event.currentTarget.select()}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            commit();
          } else if (event.key === "Escape") {
            event.preventDefault();
            setEditState({ status: "display" });
          }
        }}
        placeholder="Unnamed"
        value={editState.draft}
      />
    );
  }

  const beginEditing = () => setEditState({ draft: name, status: "editing" });
  return (
    <button
      aria-label={`Edit deliverable name: ${name || "Unnamed"}`}
      className="max-w-[40vw] truncate rounded-sm bg-transparent px-1.5 py-1 text-left text-sm font-semibold leading-none text-text-main outline-none hover:bg-surface-container-high focus-visible:ring-2 focus-visible:ring-ring"
      onDoubleClick={beginEditing}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === "F2") {
          event.preventDefault();
          beginEditing();
        }
      }}
      title="Double-click to rename"
      type="button"
    >
      {name || "Unnamed"}
    </button>
  );
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
