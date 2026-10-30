import { useRef, useState, type FormEvent } from "react";
import { runtime } from "@quarry/runtime";
import { useWorkspace } from "../../app/WorkspaceProvider";
import type { WorkspaceDeal } from "../../data/workspace";
import { Button } from "../ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import {
  SourceFilesStep,
  type DealSourceLinks,
  type SelectedSourceFiles,
} from "./DealSourceFields";

export function DealResourcesDialog({
  deal,
  onClose,
}: {
  deal: WorkspaceDeal;
  onClose: () => void;
}) {
  const { saveDealResult } = useWorkspace();
  const returnFocusRef = useRef(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  const [links, setLinks] = useState<DealSourceLinks>(() => ({
    sharepointLink: deal.persistedMetadata?.sharepointLink ?? "",
    sowLink: deal.persistedMetadata?.sowLink ?? "",
    factSheetLink: deal.persistedMetadata?.factSheetLink ?? "",
    rlLink: deal.persistedMetadata?.rlLink ?? "",
    localPath: deal.persistedMetadata?.localPath ?? "",
  }));
  const [selected, setSelected] = useState<SelectedSourceFiles>({
    sowFile: null,
    projectTimelineFile: null,
  });
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (saving) return;
    setSaving(true);
    setError("");
    try {
      const files = [selected.sowFile, selected.projectTimelineFile].filter(
        (file): file is File => file instanceof File,
      );
      const response = await runtime.api.saveDealMetadata(deal.room.id, {
        files,
        sharepointLink: links.sharepointLink.trim() || null,
        sowLink: links.sowLink.trim() || null,
        factSheetLink: links.factSheetLink.trim() || null,
        rlLink: links.rlLink.trim() || null,
      });
      saveDealResult(
        response,
        selected.sowFile instanceof File ? selected.sowFile.name : undefined,
      );
      onClose();
    } catch (cause) {
      setError(
        `Resources could not be saved. Your entries are kept here; try Save resources again. ${cause instanceof Error ? cause.message : String(cause)}`,
      );
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open && !saving) onClose();
      }}
    >
      <DialogContent
        className="max-h-[calc(100vh-2rem)] gap-0 overflow-hidden p-0 sm:max-w-[720px]"
        showCloseButton={false}
        onEscapeKeyDown={(event) => {
          if (saving) event.preventDefault();
        }}
        onPointerDownOutside={(event) => {
          if (saving) event.preventDefault();
        }}
        onCloseAutoFocus={(event) => {
          if (returnFocusRef.current?.isConnected) {
            event.preventDefault();
            returnFocusRef.current.focus();
          }
        }}
      >
        <form
          className="flex max-h-[calc(100vh-2rem)] min-h-0 flex-col"
          onSubmit={save}
        >
          <DialogHeader className="shrink-0 border-b border-border px-6 py-5">
            <DialogTitle className="text-xl font-semibold">
              Add deal resources
            </DialogTitle>
            <DialogDescription>
              Save source links and documents for {deal.room.name}.
            </DialogDescription>
          </DialogHeader>
          <div className="min-h-0 overflow-y-auto px-6 py-5">
            <fieldset disabled={saving}>
              <SourceFilesStep
                availableFiles={null}
                form={links}
                selected={selected}
                onUpdateField={(field, value) => {
                  setLinks((current) => ({ ...current, [field]: value }));
                  setError("");
                }}
                onChange={(field, file) => {
                  setSelected((current) => ({ ...current, [field]: file }));
                  setError("");
                }}
              />
            </fieldset>
            {error ? (
              <p
                role="alert"
                className="mt-4 rounded-lg border border-error/25 bg-error/8 px-3 py-2 text-sm text-error"
              >
                {error}
              </p>
            ) : null}
            {saving ? (
              <p role="status" className="mt-4 text-sm text-muted">
                Saving resources and extracting questions from any submitted
                files…
              </p>
            ) : null}
          </div>
          <DialogFooter className="mx-0 mb-0 shrink-0 rounded-none rounded-b-xl px-6 py-4">
            <Button
              type="button"
              variant="outline"
              disabled={saving}
              onClick={onClose}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={saving}>
              {saving ? "Saving…" : "Save resources"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
