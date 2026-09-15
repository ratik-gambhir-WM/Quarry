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
import { Field, FieldError, FieldGroup, FieldLabel } from "../../ui/field";
import { Icon } from "../../ui/Icon";
import { Input } from "../../ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../ui/select";
import { TransactionTypePicker } from "./DealTypePicker";

type AddDealModalProps = {
  email?: string;
  onClose: () => void;
};

type AddDealFormState = {
  closeDate: string;
  dealId: string;
  dealName: string;
  dealSponsor: string;
  factSheetLink: string;
  localPath: string;
  primaryBuyer: string;
  rlLink: string;
  sharepointLink: string;
  sowLink: string;
  startDate: string;
  status: string;
  targetCompany: string;
  transactionType: string;
};

type SourceFileSelection = File | LocalDealSourceFile;

type SelectedSourceFiles = {
  projectTimelineFile: SourceFileSelection | null;
  sowFile: SourceFileSelection | null;
};

const emptyForm: AddDealFormState = {
  closeDate: "",
  dealId: "",
  dealName: "",
  dealSponsor: "",
  factSheetLink: "",
  localPath: "",
  primaryBuyer: "",
  rlLink: "",
  sharepointLink: "",
  sowLink: "",
  startDate: "",
  status: "Active",
  targetCompany: "",
  transactionType: "",
};

const emptySourceFiles: SelectedSourceFiles = {
  projectTimelineFile: null,
  sowFile: null,
};

export function AddDealModal({ email, onClose }: AddDealModalProps) {
  const navigate = useNavigate();
  const [form, setForm] = useState(emptyForm);
  const [localDataRoom, setLocalDataRoom] = useState<LocalDealDataRoom | null>(null);
  const [createdDealId, setCreatedDealId] = useState<string | null>(null);
  const [selectedSourceFiles, setSelectedSourceFiles] = useState(emptySourceFiles);
  const [step, setStep] = useState<"details" | "sources">("details");
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
    if (step === "details") {
      await createCoreDeal();
    } else {
      await saveSources();
    }
  }

  async function createCoreDeal() {
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
    try {
      const response = await runtime.api.createDeal(buildSaveDealInput(form, email));
      setCreatedDealId(response.deal.dealId);
      setStep("sources");
    } catch (error) {
      setSubmitError(errorMessage(error));
    } finally {
      setIsSubmitting(false);
    }
  }

  async function saveSources() {
    if (!createdDealId) return;
    setIsSubmitting(true);
    setSubmitError("");
    try {
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
      const response = await runtime.api.saveDealMetadata(createdDealId, {
        factSheetLink: optionalLink(form.factSheetLink),
        files: uploads,
        rlLink: optionalLink(form.rlLink),
        sharepointLink: optionalLink(form.sharepointLink),
        sowLink: optionalLink(form.sowLink),
      });
      navigate(`/hub/deals/${encodeURIComponent(response.deal.dealId)}`, {
        state: {
          email,
          result: response,
          sowSourceName: sourceFileName(selectedSourceFiles.sowFile),
        } satisfies DealExtractionLocationState,
      });
    } catch (error) {
      setSubmitError(errorMessage(error));
    } finally {
      setIsSubmitting(false);
    }
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
        onEscapeKeyDown={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        onPointerDownOutside={(event) => {
          if (isSubmitting) event.preventDefault();
        }}
        showCloseButton={false}
      >
        <form className="flex min-h-0 flex-col" onSubmit={handleSubmit}>
          <DialogHeader className="relative gap-1 border-b border-border px-6 py-5 pr-14">
            <DialogDescription className="order-first text-xs font-medium uppercase tracking-[0.12em]">
              Active Deals
            </DialogDescription>
            <DialogTitle className="text-xl font-semibold">
              {step === "sources" ? "Add deal metadata" : "Add deal"}
            </DialogTitle>
          </DialogHeader>

          <div className="min-h-0 overflow-y-auto px-6 py-5">
            {step === "details" ? (
              <DealDetailsStep
                error={fieldError}
                form={form}
                isSubmitting={isSubmitting}
                onChooseLocalFolder={chooseLocalFolder}
                onUpdateField={updateField}
              />
            ) : (
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
            )}

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
            ) : (
              <Button disabled={isSubmitting} onClick={saveSources} type="button" variant="outline">
                Skip metadata
              </Button>
            )}
            <Button className="min-w-28" disabled={isSubmitting} type="submit">
              {isSubmitting ? (
                <span
                  aria-hidden="true"
                  className="size-3.5 rounded-full border-2 border-on-action/30 border-t-on-action motion-safe:animate-spin"
                />
              ) : null}
              {isSubmitting ? "Saving..." : step === "details" ? "Next" : "Finish deal"}
            </Button>
          </DialogFooter>
        </form>
        <DialogClose asChild>
          <Button
            aria-label="Close add deal dialog"
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

function DealDetailsStep({
  error,
  form,
  isSubmitting,
  onChooseLocalFolder,
  onUpdateField,
}: {
  error: string;
  form: AddDealFormState;
  isSubmitting: boolean;
  onChooseLocalFolder: () => void;
  onUpdateField: (field: keyof AddDealFormState, value: string) => void;
}) {
  return (
    <FieldGroup className="gap-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <DealTextField
          id="add-deal-id"
          label="Deal ID"
          onValueChange={(value) => onUpdateField("dealId", value)}
          placeholder="DEAL-000184"
          value={form.dealId}
        />
        <DealTextField
          id="add-deal-name"
          label="Deal name"
          onValueChange={(value) => onUpdateField("dealName", value)}
          placeholder="Acme acquisition of WidgetCo"
          value={form.dealName}
        />
      </div>
      <div className="grid gap-4 sm:grid-cols-3">
        <SelectField
          label="Status"
          onChange={(value) => onUpdateField("status", value)}
          options={["Active", "Pipeline", "On Hold", "Closed"]}
          value={form.status}
        />
        <DealTextField
          id="add-deal-start-date"
          label="Start date"
          onValueChange={(value) => onUpdateField("startDate", value)}
          placeholder="2026-02-14"
          type="date"
          value={form.startDate}
        />
        <DealTextField
          id="add-deal-close-date"
          label="Close date"
          onValueChange={(value) => onUpdateField("closeDate", value)}
          placeholder="2026-05-01"
          type="date"
          value={form.closeDate}
        />
      </div>
      <TransactionTypePicker
        error={!form.transactionType ? error : ""}
        onChange={(value) => onUpdateField("transactionType", value)}
        value={form.transactionType}
      />
      <div className="grid gap-4 sm:grid-cols-3">
        <DealTextField
          autoComplete="organization"
          id="add-deal-target-company"
          label="Target company"
          onValueChange={(value) => onUpdateField("targetCompany", value)}
          placeholder="Target"
          value={form.targetCompany}
        />
        <DealTextField
          autoComplete="organization"
          id="add-deal-primary-buyer"
          label="Primary buyer"
          onValueChange={(value) => onUpdateField("primaryBuyer", value)}
          placeholder="CVS"
          value={form.primaryBuyer}
        />
        <DealTextField
          autoComplete="organization"
          id="add-deal-sponsor"
          label="Deal sponsor"
          onValueChange={(value) => onUpdateField("dealSponsor", value)}
          placeholder="Thoma Bravo"
          value={form.dealSponsor}
        />
      </div>
      {runtime.target === "desktop" ? (
        <LocalFolderField disabled={isSubmitting} error={error} onChoose={onChooseLocalFolder} value={form.localPath} />
      ) : null}
    </FieldGroup>
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

function SelectField({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  const id = `add-deal-${label.toLowerCase().replace(/\s+/g, "-")}`;
  return (
    <Field>
      <FieldLabel htmlFor={id}>{label}</FieldLabel>
      <Select onValueChange={onChange} required value={value}>
        <SelectTrigger className="w-full" id={id}>
          <SelectValue />
        </SelectTrigger>
        <SelectContent position="popper">
          {options.map((option) => (
            <SelectItem key={option} value={option}>
              {option}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
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
          label="RL link"
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

function buildSaveDealInput(form: AddDealFormState, userEmail: string): SaveDealInput {
  return {
    closeDate: form.closeDate,
    dealId: form.dealId.trim(),
    dealName: form.dealName.trim(),
    dealSponsor: form.dealSponsor.trim(),
    localPath: runtime.target === "desktop" ? form.localPath.trim() : null,
    primaryBuyer: form.primaryBuyer.trim(),
    sharepointLink: null,
    startDate: form.startDate,
    status: form.status,
    targetCompany: form.targetCompany.trim(),
    transactionType: form.transactionType,
    userEmail,
  };
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
