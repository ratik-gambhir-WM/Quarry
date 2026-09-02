import { FormEvent, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import type {
  LocalDealDataRoom,
  LocalDealFileContents,
  LocalDealSourceFile,
  SaveDealInput,
} from "../../../data/dealExtraction";
import { formatFileSize } from "../../../lib/formatters";
import { DialogBackdrop } from "../../ui/DialogBackdrop";
import { DialogHeader } from "../../ui/DialogHeader";
import { Icon } from "../../ui/Icon";
import {
  ModalField,
  ModalInput,
  ModalTextField,
  modalControlClassName,
} from "../../ui/ModalField";
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
  localPath: string;
  primaryBuyer: string;
  sharepointLink: string;
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
  localPath: "",
  primaryBuyer: "",
  sharepointLink: "",
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
  const formRef = useRef<HTMLFormElement>(null);
  const [form, setForm] = useState(emptyForm);
  const [localDataRoom, setLocalDataRoom] = useState<LocalDealDataRoom | null>(null);
  const [createdDealId, setCreatedDealId] = useState<string | null>(null);
  const [selectedSourceFiles, setSelectedSourceFiles] = useState(emptySourceFiles);
  const [step, setStep] = useState<"details" | "sources">("details");
  const [fieldError, setFieldError] = useState("");
  const [submitError, setSubmitError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      const form = formRef.current;
      const firstField = form?.querySelector<HTMLElement>("input:not([type='hidden']), select, textarea");
      (firstField ?? form?.querySelector<HTMLElement>("button"))?.focus();
    });

    return () => window.cancelAnimationFrame(frame);
  }, []);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !isSubmitting) onClose();
    }
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isSubmitting, onClose]);

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
    if (runtime.target === "desktop" && !form.localPath) {
      setFieldError("Choose a local data room folder.");
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
      const response = await runtime.api.saveDealMetadata(createdDealId, uploads);
      navigate(`/hub/deals/${encodeURIComponent(response.deal.dealId)}`, {
        state: { email, result: response },
      });
    } catch (error) {
      setSubmitError(errorMessage(error));
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <DialogBackdrop
      className="z-50 px-6 py-0"
      closeLabel="Close add deal dialog"
      disabled={isSubmitting}
      onClose={onClose}
    >
      <form
        aria-labelledby="add-deal-dialog-title"
        aria-modal="true"
        className="relative z-10 flex max-h-[calc(100vh-3rem)] w-full max-w-[720px] flex-col gap-5 overflow-y-auto rounded-[19px] border border-outline-variant bg-white p-6 shadow-[0_28px_70px_rgba(7,1,84,0.2)]"
        onSubmit={handleSubmit}
        ref={formRef}
        role="dialog"
      >
        <DialogHeader
          className="gap-4"
          closeLabel="Close add deal dialog"
          eyebrow="Active Deals"
          onClose={onClose}
          title={step === "sources" ? "Add deal metadata" : "Add deal"}
          titleId="add-deal-dialog-title"
        />

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
            onChange={(field, file) => {
              setSelectedSourceFiles((current) => ({ ...current, [field]: file }));
              setSubmitError("");
            }}
            selected={selectedSourceFiles}
          />
        )}

        {submitError ? (
          <p className="rounded-2xl border border-error/25 bg-error/8 px-4 py-3 text-[12px] font-medium text-error">
            {submitError}
          </p>
        ) : null}

        <div className="flex items-center justify-end gap-3 pt-2">
          <button
            className="rounded-full px-5 py-3 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:opacity-60"
            disabled={isSubmitting}
            onClick={step === "details" ? onClose : saveSources}
            type="button"
          >
            {step === "details" ? "Cancel" : "Skip files"}
          </button>
          <button
            className="inline-flex min-w-[148px] items-center justify-center gap-2 rounded-full bg-action px-6 py-3 text-[13px] font-semibold text-on-action shadow-[0_10px_30px_rgba(7,1,84,0.18)] transition enabled:hover:bg-action-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-70"
            disabled={isSubmitting}
            type="submit"
          >
            {isSubmitting ? (
              <span className="h-4 w-4 rounded-full border-2 border-on-primary-container/30 border-t-on-primary-container motion-safe:animate-spin" />
            ) : null}
            {isSubmitting ? "Saving..." : step === "details" ? "Next" : "Finish deal"}
          </button>
        </div>
      </form>
    </DialogBackdrop>
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
    <div className="grid gap-4">
      <div className="grid gap-4 sm:grid-cols-2">
        <ModalTextField
          id="add-deal-id"
          label="Deal ID"
          onValueChange={(value) => onUpdateField("dealId", value)}
          placeholder="DEAL-000184"
          value={form.dealId}
        />
        <ModalTextField
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
        <ModalTextField
          id="add-deal-start-date"
          label="Start date"
          onValueChange={(value) => onUpdateField("startDate", value)}
          placeholder="2026-02-14"
          type="date"
          value={form.startDate}
        />
        <ModalTextField
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
        <ModalTextField
          autoComplete="organization"
          id="add-deal-target-company"
          label="Target company"
          onValueChange={(value) => onUpdateField("targetCompany", value)}
          placeholder="Target"
          value={form.targetCompany}
        />
        <ModalTextField
          autoComplete="organization"
          id="add-deal-primary-buyer"
          label="Primary buyer"
          onValueChange={(value) => onUpdateField("primaryBuyer", value)}
          placeholder="CVS"
          value={form.primaryBuyer}
        />
        <ModalTextField
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
      ) : (
        <ModalTextField
          id="add-deal-sharepoint-link"
          label="SharePoint link"
          onValueChange={(value) => onUpdateField("sharepointLink", value)}
          optional
          placeholder="https://company.sharepoint.com/sites/deal-room"
          type="url"
          value={form.sharepointLink}
        />
      )}
    </div>
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
    <ModalField htmlFor={id} label={label}>
      <select
        className={modalControlClassName}
        id={id}
        onChange={(event) => onChange(event.currentTarget.value)}
        required
        value={value}
      >
        {options.map((option) => <option key={option} value={option}>{option}</option>)}
      </select>
    </ModalField>
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
    <ModalField error={error} htmlFor="add-deal-local-path" label="Local data room folder">
      <div className="flex gap-3">
        <ModalInput
          className="min-w-0 flex-1"
          id="add-deal-local-path"
          placeholder="Choose a folder"
          readOnly
          required
          value={value}
        />
        <button
          className="flex items-center gap-2 rounded-2xl border border-outline-variant bg-white px-4 py-3 text-[13px] font-semibold text-primary"
          disabled={disabled}
          onClick={onChoose}
          type="button"
        >
          <Icon className="h-4 w-4" name="folderOpen" /> Browse
        </button>
      </div>
    </ModalField>
  );
}

function SourceFilesStep({ availableFiles, onChange, selected }: { availableFiles: LocalDealSourceFile[] | null; onChange: (field: keyof SelectedSourceFiles, file: SourceFileSelection | null) => void; selected: SelectedSourceFiles }) {
  return (
    <div className="grid gap-5">
      <p className="rounded-2xl bg-surface-container-low px-4 py-3 text-[13px] leading-5 text-muted">
        Add an SOW and project timeline if available. Both are optional; key questions are extracted from the submitted documents.
      </p>
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
  const sharepointLink = form.sharepointLink.trim();
  return {
    closeDate: form.closeDate,
    dealId: form.dealId.trim(),
    dealName: form.dealName.trim(),
    dealSponsor: form.dealSponsor.trim(),
    localPath: runtime.target === "desktop" ? form.localPath.trim() : null,
    primaryBuyer: form.primaryBuyer.trim(),
    sharepointLink: runtime.target === "web" && sharepointLink ? sharepointLink : null,
    startDate: form.startDate,
    status: form.status,
    targetCompany: form.targetCompany.trim(),
    transactionType: form.transactionType,
    userEmail,
  };
}

function isLocalDealSourceFile(file: SourceFileSelection | null): file is LocalDealSourceFile {
  return file !== null && "relativePath" in file && "mimeType" in file;
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
