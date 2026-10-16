import { type ComponentProps, type FormEvent, useRef, useState } from "react";
import { X } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import type {
  DealExtractionLocationState,
  LocalDealDataRoom,
  LocalDealFileContents,
  LocalDealSourceFile,
  SaveDealInput,
  SaveDealResponse,
} from "../../../data/dealExtraction";
import { formatFileSize } from "../../../lib/formatters";
import { Button } from "../../ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../../ui/dialog";
import { Field, FieldError, FieldLabel } from "../../ui/field";
import { Icon } from "../../ui/Icon";
import { Input } from "../../ui/input";

import { DealCompanyRolesEditor, DealDetailsEditor, DealFieldsEditor } from "./DealFieldsEditor";
import type { DealFields } from "../../../data/dealModel";
import type { SavedDeal } from "../../../data/dealExtraction";
import { useWorkspace } from "../../../app/WorkspaceProvider";

type AddDealModalProps = {
  email?: string;
  initialDeal?: SavedDeal;
  onClose: () => void;
};

type AddDealFormState = DealFields & {
  dealId: string; factSheetLink: string; localPath: string; rlLink: string; sharepointLink: string; sowLink: string;
};

type SourceFileSelection = File | LocalDealSourceFile;

type SelectedSourceFiles = {
  projectTimelineFile: SourceFileSelection | null;
  sowFile: SourceFileSelection | null;
};

const emptyForm: AddDealFormState = {
  dealId: "", dealName: "", dealCode: null, engagementType: "buy_side", transactionType: "unknown", status: "pipeline",
  startDate: null, expectedCloseDate: null, actualCloseDate: null, engagementCompletedDate: null,
  parties: [], scopes: [], factSheetLink: "", localPath: "", rlLink: "", sharepointLink: "", sowLink: "",
};

const emptySourceFiles: SelectedSourceFiles = {
  projectTimelineFile: null,
  sowFile: null,
};

export function AddDealModal({ email, onClose, initialDeal }: AddDealModalProps) {
  const navigate = useNavigate();
  const returnFocusRef = useRef(
    document.activeElement instanceof HTMLElement && document.activeElement !== document.body
      ? document.activeElement
      : null,
  );
  const { saveDealResult, saveCompanyResult } = useWorkspace();
  const [form, setForm] = useState<AddDealFormState>(() => initialDeal ? { ...emptyForm, ...initialDeal } : emptyForm);
  const [localDataRoom, setLocalDataRoom] = useState<LocalDealDataRoom | null>(null);
  const [createdDeal, setCreatedDeal] = useState<SaveDealResponse | null>(null);
  const [selectedSourceFiles, setSelectedSourceFiles] = useState(emptySourceFiles);
  const [step, setStep] = useState<"details" | "companies" | "sources">("details");
  const [companyFormOpen, setCompanyFormOpen] = useState(false);
  const titleRef = useRef<HTMLHeadingElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [fieldError, setFieldError] = useState("");
  const [submitError, setSubmitError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  function updateField(field: keyof AddDealFormState, value: string) {
    setForm((current) => ({ ...current, [field]: value }));
    setFieldError("");
    setSubmitError("");
  }

  async function chooseLocalFolder() {
    setFieldError("");
    try {
      const selection = await runtime.platform.selectDealDataRoom();
      if (selection) {
        setLocalDataRoom(selection);
        updateField("localPath", selection.rootPath);
        setSelectedSourceFiles(emptySourceFiles);
      }
    } catch (error) {
      setFieldError(errorMessage(error));
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isSubmitting || companyFormOpen) return;
    if (initialDeal) {
      await saveDeal(false);
    } else if (step === "details") {
      changeStep("companies");
    } else if (step === "companies") {
      changeStep("sources");
    } else {
      await saveDeal(true);
    }
  }

  function changeStep(next: typeof step) {
    setStep(next);
    setSubmitError("");
    window.requestAnimationFrame(() => {
      titleRef.current?.focus();
      if (scrollRef.current) scrollRef.current.scrollTop = 0;
    });
  }

  function updateDealFields(fields: DealFields) {
    setForm((current) => ({ ...current, ...fields }));
    setSubmitError("");
  }

  async function saveDeal(includeMetadata: boolean) {
    if (!email) {
      setSubmitError("Sign in again before adding a deal.");
      return;
    }
    if (!form.transactionType) {
      setFieldError("Select a transaction type.");
      return;
    }
    setIsSubmitting(true);
    setSubmitError("");
    let savedCore = createdDeal;
    try {
      if (!savedCore) {
        const input = buildSaveDealInput(form, email);
        savedCore = initialDeal ? await runtime.api.updateDeal(initialDeal.dealId, editableFields(input)) : await runtime.api.createDeal(input);
        saveDealResult(savedCore);
        if (!initialDeal) setCreatedDeal(savedCore);
      }
      if (initialDeal) { onClose(); return; }
      if (includeMetadata) await saveSources(savedCore);
      else finishWithoutMetadata(savedCore);
    } catch (error) {
      if (savedCore && !initialDeal) {
        setSubmitError(`Deal created, but metadata could not be saved. Retry Finish deal or skip metadata. ${errorMessage(error)}`);
      } else {
        if (!initialDeal) changeStep("companies");
        setSubmitError(errorMessage(error));
      }
    } finally {
      setIsSubmitting(false);
    }
  }

  async function saveSources(core: SaveDealResponse) {
    const selections = [
      selectedSourceFiles.sowFile,
      selectedSourceFiles.projectTimelineFile,
    ].filter((file): file is SourceFileSelection => file !== null);
    const uploads = localDataRoom && selections.length > 0
      ? (
          await runtime.platform.readDealSourceFiles({
            paths: selections.filter(isLocalDealSourceFile).map((file) => file.path),
            rootPath: localDataRoom.rootPath,
          })
        ).map(localFileContentsToFile)
      : selections.filter((file): file is File => file instanceof File);
    const response = await runtime.api.saveDealMetadata(core.deal.dealId, {
      factSheetLink: optionalLink(form.factSheetLink),
      files: uploads,
      rlLink: optionalLink(form.rlLink),
      sharepointLink: optionalLink(form.sharepointLink),
      sowLink: optionalLink(form.sowLink),
    });
    saveDealResult(response, sourceFileName(selectedSourceFiles.sowFile));
    navigate(`/hub/deals/${encodeURIComponent(response.deal.dealId)}`, {
      state: {
        email,
        result: response,
        sowSourceName: sourceFileName(selectedSourceFiles.sowFile),
      } satisfies DealExtractionLocationState,
    });
  }

  function finishWithoutMetadata(core: SaveDealResponse) {
    navigate(`/hub/deals/${encodeURIComponent(core.deal.dealId)}`, {
      state: {
        email,
        result: {
          ...core,
          extraction: { keyQuestions: [] },
          files: [],
        },
      } satisfies DealExtractionLocationState,
    });
  }

  return (
    <Dialog
      onOpenChange={(open) => {
        if (!open && !isSubmitting) onClose();
      }}
      open
    >
      <DialogContent
        className="max-h-[calc(100vh-2rem)] gap-0 overflow-hidden p-0 sm:max-w-[720px]"
        onCloseAutoFocus={(event) => {
          if (returnFocusRef.current?.isConnected) {
            event.preventDefault();
            returnFocusRef.current.focus();
          }
        }}
        onEscapeKeyDown={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        onPointerDownOutside={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        showCloseButton={false}
      >
        <form className="flex max-h-[calc(100vh-2rem)] min-h-0 flex-col" onSubmit={handleSubmit}>
          <DialogHeader className="relative shrink-0 gap-1 border-b border-border px-6 py-5 pr-14">
            <DialogDescription className="order-first text-xs font-medium uppercase tracking-[0.12em]">
              {initialDeal ? "Active Deals" : `Step ${step === "details" ? 1 : step === "companies" ? 2 : 3} of 3`}
            </DialogDescription>
            <DialogTitle ref={titleRef} tabIndex={-1} className="text-xl font-semibold">
              {initialDeal ? "Edit deal" : step === "sources" ? "Add deal metadata" : step === "companies" ? "Assign companies and roles" : "Add deal"}
            </DialogTitle>
          </DialogHeader>

          <div ref={scrollRef} className="min-h-0 overflow-y-auto px-6 py-5">
            <div hidden={step !== "details"}>
              <fieldset className="grid gap-4" disabled={isSubmitting || step !== "details"}>
                {!initialDeal ? <DealTextField id="add-deal-id" label="Deal ID" maxLength={64} onValueChange={(value) => updateField("dealId", value)} placeholder="DEAL-000184" value={form.dealId} /> : null}
                {initialDeal ? <DealFieldsEditor onCompanyFormChange={setCompanyFormOpen} onCompanySaved={saveCompanyResult} userEmail={email ?? ""} fields={form} initialCompanies={initialDeal.parties.map((p) => p.company)} onChange={updateDealFields} /> : <DealDetailsEditor fields={form} onChange={updateDealFields} />}
                {initialDeal && companyFormOpen ? <p className="text-sm text-muted-foreground">Save or cancel the company before continuing.</p> : null}
                {!initialDeal && runtime.target === "desktop" ? <LocalFolderField disabled={isSubmitting} error={fieldError} onChoose={chooseLocalFolder} value={form.localPath} /> : null}
              </fieldset>
            </div>
            {!initialDeal ? <div hidden={step !== "companies"}>
              <fieldset disabled={isSubmitting || step !== "companies"}>
                <DealCompanyRolesEditor onCompanyFormChange={setCompanyFormOpen} onCompanySaved={saveCompanyResult} userEmail={email ?? ""} fields={form} onChange={updateDealFields} />
                {companyFormOpen ? <p className="mt-3 text-sm text-muted-foreground">Save or cancel the company before continuing.</p> : null}
              </fieldset>
            </div> : null}
            {step === "sources" ? (
              <fieldset disabled={isSubmitting}>
                <SourceFilesStep
                  availableFiles={localDataRoom?.files ?? null}
                  form={form}
                  onChange={(field, file) => {
                    setSelectedSourceFiles((current) => ({ ...current, [field]: file }));
                    setSubmitError("");
                  }}
                  onUpdateField={updateField}
                  selected={selectedSourceFiles}
                />
              </fieldset>
            ) : null}

            {submitError ? (
              <p
                className="mt-4 rounded-lg border border-error/25 bg-error/8 px-3 py-2 text-sm font-medium text-error"
                role="alert"
              >
                {submitError}
              </p>
            ) : null}
          </div>

          <DialogFooter className="mx-0 mb-0 shrink-0 rounded-none rounded-b-xl border-border px-6 py-4">
            {step === "details" ? (
              <DialogClose asChild>
                <Button disabled={isSubmitting} type="button" variant="outline">
                  Cancel
                </Button>
              </DialogClose>
            ) : <Button disabled={isSubmitting || companyFormOpen || Boolean(createdDeal)} onClick={() => changeStep(step === "sources" ? "companies" : "details")} type="button" variant="outline">Back</Button>}
            {step === "sources" ? (
              <Button disabled={isSubmitting} onClick={() => void saveDeal(false)} type="button" variant="outline">
                Skip metadata
              </Button>
            ) : null}
            <Button className="min-w-28" disabled={isSubmitting || companyFormOpen} type="submit">
              {isSubmitting ? (
                <span
                  aria-hidden="true"
                  className="size-3.5 rounded-full border-2 border-on-action/30 border-t-on-action motion-safe:animate-spin"
                />
              ) : null}
              {isSubmitting ? "Saving..." : initialDeal ? "Save deal" : step === "sources" ? "Finish deal" : "Next"}
            </Button>
          </DialogFooter>
        </form>
        <DialogClose asChild>
          <Button
            aria-label={initialDeal ? "Close edit deal dialog" : "Close add deal dialog"}
            className="absolute right-4 top-4"
            disabled={isSubmitting}
            size="icon-sm"
            type="button"
            variant="ghost"
          >
            <X aria-hidden="true" />
          </Button>
        </DialogClose>
      </DialogContent>
    </Dialog>
  );
}

type DealTextFieldProps = Omit<ComponentProps<typeof Input>, "id" | "onChange"> & {
  error?: string;
  id: string;
  label: string;
  onValueChange: (value: string) => void;
  optional?: boolean;
};

function DealTextField({
  error,
  id,
  label,
  onValueChange,
  optional = false,
  required = !optional,
  ...props
}: DealTextFieldProps) {
  return (
    <Field data-invalid={Boolean(error)}>
      <div className="flex items-center justify-between">
        <FieldLabel htmlFor={id}>{label}</FieldLabel>
        {optional ? <span className="text-xs text-muted-foreground">Optional</span> : null}
      </div>
      <Input
        aria-invalid={Boolean(error)}
        id={id}
        onChange={(event) => onValueChange(event.currentTarget.value)}
        required={required}
        {...props}
      />
      <FieldError>{error}</FieldError>
    </Field>
  );
}

function LocalFolderField({
  disabled,
  error,
  onChoose,
  value,
}: {
  disabled: boolean;
  error: string;
  onChoose: () => void;
  value: string;
}) {
  return (
    <Field data-invalid={Boolean(error)}>
      <div className="flex items-center justify-between">
        <FieldLabel htmlFor="add-deal-local-path">Local data room folder</FieldLabel>
        <span className="text-xs text-muted-foreground">Optional</span>
      </div>
      <div className="flex gap-3">
        <Input
          aria-invalid={Boolean(error)}
          className="min-w-0 flex-1"
          id="add-deal-local-path"
          placeholder="Choose a folder"
          readOnly
          value={value}
        />
        <Button
          disabled={disabled}
          onClick={onChoose}
          type="button"
          variant="outline"
        >
          <Icon className="h-4 w-4" name="folderOpen" /> Browse
        </Button>
      </div>
      <FieldError>{error}</FieldError>
    </Field>
  );
}

function SourceFilesStep({ availableFiles, form, onChange, onUpdateField, selected }: { availableFiles: LocalDealSourceFile[] | null; form: AddDealFormState; onChange: (field: keyof SelectedSourceFiles, file: SourceFileSelection | null) => void; onUpdateField: (field: keyof AddDealFormState, value: string) => void; selected: SelectedSourceFiles }) {
  return (
    <div className="grid gap-5">
      <p className="rounded-2xl bg-surface-container-low px-4 py-3 text-[13px] leading-5 text-muted">
        Add source links and files if available. All fields are optional; key questions are extracted from the submitted documents.
      </p>
      <div className="grid gap-4 sm:grid-cols-2">
        <DealTextField
          disabled={Boolean(form.localPath)}
          id="add-deal-sharepoint-link"
          label="SharePoint link"
          onValueChange={(value) => onUpdateField("sharepointLink", value)}
          optional
          placeholder="https://company.sharepoint.com/sites/deal-room"
          title={form.localPath ? "A local data room is already selected for this deal." : undefined}
          type="url"
          value={form.sharepointLink}
        />
        <DealTextField
          id="add-deal-sow-link"
          label="SOW link"
          onValueChange={(value) => onUpdateField("sowLink", value)}
          optional
          placeholder="https://example.com/sow"
          type="url"
          value={form.sowLink}
        />
        <DealTextField
          id="add-deal-fact-sheet-link"
          label="Fact sheet link"
          onValueChange={(value) => onUpdateField("factSheetLink", value)}
          optional
          placeholder="https://example.com/fact-sheet"
          type="url"
          value={form.factSheetLink}
        />
        <DealTextField
          id="add-deal-rl-link"
          label="Request list link"
          onValueChange={(value) => onUpdateField("rlLink", value)}
          optional
          placeholder="https://example.com/request-list"
          type="url"
          value={form.rlLink}
        />
      </div>
      <SourceFilePicker availableFiles={availableFiles?.filter((file) => file.matchedOn.includes("SOW")) ?? null} file={selected.sowFile} label="SOW file" onChange={(file) => onChange("sowFile", file)} />
      <SourceFilePicker availableFiles={availableFiles?.filter((file) => file.matchedOn.includes("Project Timeline")) ?? null} file={selected.projectTimelineFile} label="Project timeline" onChange={(file) => onChange("projectTimelineFile", file)} />
    </div>
  );
}

function SourceFilePicker({ availableFiles, file, label, onChange }: { availableFiles: LocalDealSourceFile[] | null; file: SourceFileSelection | null; label: string; onChange: (file: SourceFileSelection | null) => void }) {
  const inputRef = useRef<HTMLInputElement>(null);
  return (
    <section className="grid gap-3">
      <div className="flex items-center justify-between"><h3 className="text-[11px] font-bold uppercase tracking-[0.16em] text-muted">{label}</h3><span className="rounded-full bg-surface-container-low px-3 py-1 text-[11px] font-semibold text-muted">Optional</span></div>
      {availableFiles === null ? <input className="hidden" onChange={(event) => { onChange(event.target.files?.[0] ?? null); event.target.value = ""; }} ref={inputRef} type="file" /> : null}
      {availableFiles !== null ? (
        availableFiles.length ? <div className="grid gap-2">{availableFiles.map((candidate) => <button className={`rounded-xl border p-3 text-left ${isLocalDealSourceFile(file) && file.path === candidate.path ? "border-primary bg-primary/5" : "border-outline-variant"}`} key={candidate.path} onClick={() => onChange(isLocalDealSourceFile(file) && file.path === candidate.path ? null : candidate)} type="button"><SourceFileOption file={candidate} /></button>)}</div> : <p className="rounded-2xl border border-dashed border-outline px-4 py-4 text-[12px] text-muted">No matching {label.toLowerCase()} was found.</p>
      ) : file ? <div className="flex items-center gap-3 rounded-xl border border-primary p-3"><SourceFileOption file={file} /><button className="text-[12px] font-semibold text-muted" onClick={() => onChange(null)} type="button">Remove</button></div> : <button className="rounded-2xl border border-dashed border-outline px-4 py-5 text-[13px] font-semibold text-primary" onClick={() => inputRef.current?.click()} type="button"><Icon className="mr-2 inline h-4 w-4" name="upload" />Choose file</button>}
    </section>
  );
}

function SourceFileOption({ file }: { file: SourceFileSelection }) {
  const name = isLocalDealSourceFile(file) ? file.filename : file.name;
  const size = isLocalDealSourceFile(file) ? file.sizeBytes : file.size;
  return <span className="flex min-w-0 flex-1 items-center gap-3"><Icon className="h-4 w-4 shrink-0 text-primary" name={fileIcon(name)} /><span className="min-w-0 flex-1 truncate text-[13px] font-bold text-text-main">{name}</span><span className="text-[11px] text-muted">{formatFileSize(size)}</span></span>;
}

function editableFields(form: DealFields): DealFields {
  return {
    dealName: form.dealName.trim(), dealCode: form.dealCode?.trim() || null,
    engagementType: form.engagementType, transactionType: form.transactionType, status: form.status,
    startDate: form.startDate, expectedCloseDate: form.expectedCloseDate, actualCloseDate: form.actualCloseDate,
    engagementCompletedDate: form.engagementCompletedDate,
    parties: form.parties.map(({ companyId, role, notes }) => ({ companyId, role, notes })),
    scopes: form.scopes,
  };
}
function buildSaveDealInput(form: AddDealFormState, userEmail: string): SaveDealInput {
  return { ...editableFields(form), dealId: form.dealId.trim(), userEmail,
    localPath: runtime.target === "desktop" ? form.localPath.trim() || null : null, sharepointLink: null };
}

function optionalLink(value: string) {
  return value.trim() || null;
}

function isLocalDealSourceFile(file: SourceFileSelection | null): file is LocalDealSourceFile {
  return file !== null && "relativePath" in file && "mimeType" in file;
}

function sourceFileName(file: SourceFileSelection | null) {
  if (!file) return undefined;
  return isLocalDealSourceFile(file) ? file.filename : file.name;
}

function localFileContentsToFile(file: LocalDealFileContents) {
  const bytes = Uint8Array.from(atob(file.dataBase64), (character) => character.charCodeAt(0));
  return new File([bytes], file.filename, { type: file.mimeType });
}

function fileIcon(filename: string): "doc" | "pdf" | "sheet" {
  const extension = filename.split(".").pop()?.toLowerCase();
  if (extension === "pdf") return "pdf";
  if (extension === "xls" || extension === "xlsx" || extension === "csv") return "sheet";
  return "doc";
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
