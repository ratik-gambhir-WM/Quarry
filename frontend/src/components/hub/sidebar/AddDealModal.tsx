import { FormEvent, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { runtime } from "@quarry/runtime";
import type { SaveDealAndExtractInput } from "../../../data/dealExtraction";
import { Icon } from "../../ui/Icon";
import { DealTypePicker } from "./DealTypePicker";
import { ModalTextField } from "./ModalTextField";

type AddDealModalProps = {
  onClose: () => void;
};

type AddDealFormState = {
  buyerOrPlatformCompany: string;
  carveOutBusiness: string;
  dataRoomFolder: string;
  dealName: string;
  dealType: string;
  parentOrSellerCompany: string;
  peFirm: string;
  targetCompany: string;
};

type ModalStep = "deal-details" | "source-files";

type SelectedSourceFiles = {
  projectTimelineFile: File | null;
  sowFile: File | null;
};

const emptyAddDealForm: AddDealFormState = {
  buyerOrPlatformCompany: "",
  carveOutBusiness: "",
  dataRoomFolder: "",
  dealName: "",
  dealType: "",
  parentOrSellerCompany: "",
  peFirm: "",
  targetCompany: "",
};

const emptySelectedSourceFiles: SelectedSourceFiles = {
  projectTimelineFile: null,
  sowFile: null,
};

export function AddDealModal({ onClose }: AddDealModalProps) {
  const navigate = useNavigate();
  const folderInputRef = useRef<HTMLInputElement>(null);
  const [form, setForm] = useState<AddDealFormState>(emptyAddDealForm);
  const [files, setFiles] = useState<File[]>([]);
  const [createdDealId, setCreatedDealId] = useState<number | null>(null);
  const [companyError, setCompanyError] = useState("");
  const [dealTypeError, setDealTypeError] = useState("");
  const [folderError, setFolderError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [modalStep, setModalStep] = useState<ModalStep>("deal-details");
  const [selectedSourceFiles, setSelectedSourceFiles] = useState<SelectedSourceFiles>(emptySelectedSourceFiles);
  const [sowFileError, setSowFileError] = useState("");
  const [submitError, setSubmitError] = useState("");
  const companyFields = getCompanyFieldsForDealType(form.dealType);

  useEffect(() => {
    folderInputRef.current?.setAttribute("directory", "");
    folderInputRef.current?.setAttribute("webkitdirectory", "");

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !isSubmitting) {
        onClose();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isSubmitting, onClose]);

  function updateField(field: keyof AddDealFormState, value: string) {
    setForm((current) => ({ ...current, [field]: value }));
    setCreatedDealId(null);
    setSelectedSourceFiles(emptySelectedSourceFiles);
    setSowFileError("");
    setSubmitError("");
    if (field === "dealType") {
      setDealTypeError("");
      setCompanyError("");
    }

    if (
      field === "targetCompany" ||
      field === "buyerOrPlatformCompany" ||
      field === "parentOrSellerCompany" ||
      field === "carveOutBusiness"
    ) {
      setCompanyError("");
    }
  }

  async function handleChooseFolder() {
    setFolderError("");
    setSubmitError("");
    folderInputRef.current?.click();
  }

  function handleFolderSelection(fileList: FileList | null) {
    const selectedFiles = Array.from(fileList ?? []);
    setFiles(selectedFiles);
    const firstFile = selectedFiles[0] as (File & { webkitRelativePath?: string }) | undefined;
    const rootName = firstFile?.webkitRelativePath?.split("/")[0] ?? "";
    updateField("dataRoomFolder", rootName);
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (modalStep === "source-files") {
      await handleCreateAndExtract();
      return;
    }

    setSubmitError("");

    if (!form.dataRoomFolder) {
      setFolderError("Choose a main data room folder.");
      return;
    }

    if (!form.dealType) {
      setDealTypeError("Select a type of deal.");
      return;
    }

    if (companyFields.some((field) => !form[field.name].trim())) {
      setCompanyError("Complete the company fields for this deal type.");
      return;
    }

    setModalStep("source-files");
  }

  async function handleCreateAndExtract() {
    setSubmitError("");

    if (!selectedSourceFiles.sowFile) {
      setSowFileError("Choose the SOW file before submitting the deal.");
      return;
    }

    setIsSubmitting(true);

    try {
      const dealId = createdDealId ?? (await runtime.api.createDealFromUpload(buildSaveDealAndExtractInput(form), files)).deal.id;
      setCreatedDealId(dealId);
      const selectedUploads = [selectedSourceFiles.sowFile, selectedSourceFiles.projectTimelineFile].filter(
        (file): file is File => file !== null,
      );
      const response = await runtime.api.extractDealQuestionsFromUpload(dealId, selectedUploads);

      navigate(`/hub/deals/${response.deal.id}`, {
        state: {
          result: response,
        },
      });
    } catch (error) {
      setSubmitError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsSubmitting(false);
    }
  }

  function handleBackToDetails() {
    setSubmitError("");
    setSowFileError("");
    setModalStep("deal-details");
  }

  function handleSourceFileChange(field: keyof SelectedSourceFiles, file: File | null) {
    setSelectedSourceFiles((current) => ({ ...current, [field]: file }));
    setSubmitError("");
    if (field === "sowFile") {
      setSowFileError("");
    }
  }

  return (
    <div className="modal-backdrop fixed inset-0 z-50 flex items-center justify-center px-6">
      <button
        aria-label="Close add deal dialog"
        className="absolute inset-0 cursor-default disabled:cursor-wait"
        disabled={isSubmitting}
        onClick={onClose}
        type="button"
      />

      <form
        className="relative z-10 flex max-h-[calc(100vh-3rem)] w-full max-w-[640px] flex-col gap-5 overflow-y-auto rounded-[19px] border border-outline-variant bg-white p-6 shadow-[0_28px_70px_rgba(7,1,84,0.2)]"
        onSubmit={handleSubmit}
      >
        <input
          className="hidden"
          multiple
          onChange={(event) => handleFolderSelection(event.target.files)}
          ref={folderInputRef}
          type="file"
        />
        <div className="flex items-start justify-between gap-4">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-[0.2em] text-muted">Active Deals</p>
            <h2 className="mt-2 text-[2rem] font-bold leading-none text-text-main [font-family:var(--font-heading)]">
              {modalStep === "deal-details" ? "Add deal" : "Upload source files"}
            </h2>
          </div>
          <button
            aria-label="Close add deal dialog"
            className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            disabled={isSubmitting}
            onClick={onClose}
            type="button"
          >
            <Icon className="h-5 w-5 rotate-45" name="plus" />
          </button>
        </div>

        {modalStep === "deal-details" ? (
          <DealDetailsStep
            companyError={companyError}
            companyFields={companyFields}
            dealTypeError={dealTypeError}
            folderError={folderError}
            form={form}
            isSubmitting={isSubmitting}
            onChooseFolder={handleChooseFolder}
            onUpdateField={updateField}
          />
        ) : (
          <SourceFilesStep
            dataRoomFolder={form.dataRoomFolder}
            selectedSourceFiles={selectedSourceFiles}
            sowFileError={sowFileError}
            onFileChange={handleSourceFileChange}
          />
        )}

        {submitError ? (
          <p className="rounded-2xl border border-error/25 bg-error/8 px-4 py-3 text-[12px] font-medium text-error">
            {submitError}
          </p>
        ) : null}

        <div className="flex items-center justify-end gap-3 pt-2">
          {modalStep === "source-files" ? (
            <button
              className="rounded-full px-5 py-3 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-60"
              disabled={isSubmitting}
              onClick={handleBackToDetails}
              type="button"
            >
              Back
            </button>
          ) : (
            <button
              className="rounded-full px-5 py-3 text-[13px] font-semibold text-muted transition hover:bg-surface-container-high hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-60"
              disabled={isSubmitting}
              onClick={onClose}
              type="button"
            >
              Cancel
            </button>
          )}
          <button
            className="inline-flex min-w-[148px] items-center justify-center gap-2 rounded-full bg-primary-container px-6 py-3 text-[13px] font-semibold text-on-primary-container shadow-[0_10px_30px_rgba(7,1,84,0.18)] transition hover:bg-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed disabled:cursor-wait disabled:opacity-70"
            disabled={isSubmitting}
            type="submit"
          >
            {isSubmitting ? (
              <span className="h-4 w-4 rounded-full border-2 border-on-primary-container/30 border-t-on-primary-container motion-safe:animate-spin" />
            ) : null}
            <span>{getSubmitLabel(modalStep, isSubmitting)}</span>
          </button>
        </div>
      </form>
    </div>
  );
}

type DealDetailsStepProps = {
  companyError: string;
  companyFields: CompanyFieldConfig[];
  dealTypeError: string;
  folderError: string;
  form: AddDealFormState;
  isSubmitting: boolean;
  onChooseFolder: () => void;
  onUpdateField: (field: keyof AddDealFormState, value: string) => void;
};

function DealDetailsStep({
  companyError,
  companyFields,
  dealTypeError,
  folderError,
  form,
  isSubmitting,
  onChooseFolder,
  onUpdateField,
}: DealDetailsStepProps) {
  return (
    <div className="grid gap-4">
      <ModalTextField
        autoComplete="off"
        label="Deal name"
        onChange={(value) => onUpdateField("dealName", value)}
        placeholder="Project Gamma"
        value={form.dealName}
      />

      <div className="space-y-2">
        <label className="px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted" htmlFor="add-deal-data-room">
          Main data room folder
        </label>
        <div className="flex gap-3">
          <input
            className="min-w-0 flex-1 rounded-2xl border border-outline-variant bg-surface-container-lowest px-4 py-3 text-[14px] text-text-main outline-none transition placeholder:text-muted/60 focus:border-primary-container focus:ring-4 focus:ring-primary-fixed/40"
            id="add-deal-data-room"
            placeholder="Choose a folder"
            readOnly
            required
            value={form.dataRoomFolder}
          />
          <button
            className="flex shrink-0 items-center gap-2 rounded-2xl border border-outline-variant bg-white px-4 py-3 text-[13px] font-semibold text-primary transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
            disabled={isSubmitting}
            onClick={onChooseFolder}
            type="button"
          >
            <Icon className="h-4 w-4" name="folderOpen" />
            <span>Browse</span>
          </button>
        </div>
        {folderError ? <p className="px-1 text-[12px] font-medium text-error">{folderError}</p> : null}
      </div>

      <div className="space-y-2">
        <DealTypePicker error={dealTypeError} onChange={(value) => onUpdateField("dealType", value)} value={form.dealType} />
      </div>

      {form.dealType ? (
        <div className={`grid gap-4 ${companyFields.length > 1 ? "sm:grid-cols-2" : ""}`}>
          {companyFields.map((field) => (
            <ModalTextField
              autoComplete="organization"
              key={field.name}
              label={field.label}
              onChange={(value) => onUpdateField(field.name, value)}
              placeholder={field.placeholder}
              value={form[field.name]}
            />
          ))}
          {companyError ? <p className="px-1 text-[12px] font-medium text-error sm:col-span-2">{companyError}</p> : null}
        </div>
      ) : null}

      <div className="grid gap-4">
        <ModalTextField
          autoComplete="organization"
          label="PE firm"
          onChange={(value) => onUpdateField("peFirm", value)}
          placeholder="West Monroe Capital"
          value={form.peFirm}
        />
      </div>
    </div>
  );
}

type SourceFilesStepProps = {
  dataRoomFolder: string;
  selectedSourceFiles: SelectedSourceFiles;
  sowFileError: string;
  onFileChange: (field: keyof SelectedSourceFiles, file: File | null) => void;
};

function SourceFilesStep({ dataRoomFolder, selectedSourceFiles, sowFileError, onFileChange }: SourceFilesStepProps) {
  return (
    <div className="grid gap-5">
      <p className="rounded-2xl bg-surface-container-low px-4 py-3 text-[13px] leading-5 text-muted">
        Choose the source files from <span className="font-semibold text-text-main">{dataRoomFolder}</span>. The SOW is
        required; the project timeline can be added if one is available.
      </p>
      <SourceFilePicker
        error={sowFileError}
        file={selectedSourceFiles.sowFile}
        label="SOW file"
        onChange={(file) => onFileChange("sowFile", file)}
      />
      <SourceFilePicker
        file={selectedSourceFiles.projectTimelineFile}
        label="Project timeline"
        optional
        onChange={(file) => onFileChange("projectTimelineFile", file)}
      />
    </div>
  );
}

type SourceFilePickerProps = {
  error?: string;
  file: File | null;
  label: string;
  optional?: boolean;
  onChange: (file: File | null) => void;
};

function SourceFilePicker({ error = "", file, label, optional = false, onChange }: SourceFilePickerProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  return (
    <section className="grid gap-3">
      <div className="flex items-center justify-between gap-3">
        <h3 className="px-1 text-[11px] font-bold uppercase tracking-[0.16em] text-muted">{label}</h3>
        <span className="rounded-full bg-surface-container-low px-3 py-1 text-[11px] font-semibold text-muted">
          {optional ? "Optional" : "Required"}
        </span>
      </div>
      <input
        className="hidden"
        onChange={(event) => {
          onChange(event.target.files?.[0] ?? null);
          event.target.value = "";
        }}
        ref={inputRef}
        type="file"
      />
      {file ? (
        <div className="flex items-start gap-3 rounded-[11px] border border-primary-container bg-primary-fixed/50 p-3">
          <SourceFileOption file={file} />
          <div className="flex shrink-0 items-center gap-1">
            {optional ? (
              <button
                aria-label={`Remove ${label}`}
                className="rounded-full px-3 py-2 text-[12px] font-semibold text-muted transition hover:bg-white/70 hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
                onClick={() => onChange(null)}
                type="button"
              >
                Remove
              </button>
            ) : null}
            <button
              className="rounded-full bg-white px-3 py-2 text-[12px] font-semibold text-primary transition hover:bg-surface-container-high focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
              onClick={() => inputRef.current?.click()}
              type="button"
            >
              Replace
            </button>
          </div>
        </div>
      ) : (
        <button
          className="flex items-center justify-center gap-2 rounded-2xl border border-dashed border-outline bg-surface-container-lowest px-4 py-5 text-[13px] font-semibold text-primary transition hover:border-primary-container hover:bg-surface-container-low focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary-fixed"
          onClick={() => inputRef.current?.click()}
          type="button"
        >
          <Icon className="h-4 w-4" name="folderOpen" />
          <span>Choose {label.toLowerCase()}</span>
        </button>
      )}
      {error ? <p className="px-1 text-[12px] font-medium text-error">{error}</p> : null}
    </section>
  );
}

function SourceFileOption({ file }: { file: File }) {
  return (
    <span className="flex min-w-0 flex-1 items-start gap-3">
      <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-white text-primary">
        <Icon className="h-4 w-4" name={fileIcon(file.name)} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-bold text-text-main">{file.name}</span>
        <span className="mt-2 block text-[11px] font-semibold uppercase tracking-[0.12em] text-muted">
          {formatFileSize(file.size)}
        </span>
      </span>
    </span>
  );
}

function buildSaveDealAndExtractInput(form: AddDealFormState): SaveDealAndExtractInput {
  return {
    buyerOrPlatformCompany: optionalString(form.buyerOrPlatformCompany),
    carveOutBusiness: optionalString(form.carveOutBusiness),
    dealName: form.dealName.trim(),
    dealType: form.dealType.trim(),
    mainDataRoomFolder: form.dataRoomFolder.trim(),
    parentOrSellerCompany: optionalString(form.parentOrSellerCompany),
    peFirm: form.peFirm.trim(),
    targetCompany: optionalString(form.targetCompany),
  };
}

function getSubmitLabel(modalStep: ModalStep, isSubmitting: boolean) {
  if (modalStep === "source-files") {
    return isSubmitting ? "Adding deal" : "Submit deal";
  }

  return "Continue";
}

function optionalString(value: string): string | null {
  const trimmedValue = value.trim();
  return trimmedValue ? trimmedValue : null;
}

function formatFileSize(sizeBytes: number) {
  if (sizeBytes < 1024) {
    return `${sizeBytes} bytes`;
  }

  if (sizeBytes < 1024 * 1024) {
    return `${(sizeBytes / 1024).toFixed(1)} KB`;
  }

  return `${(sizeBytes / (1024 * 1024)).toFixed(1)} MB`;
}

function fileIcon(filename: string): "doc" | "pdf" | "sheet" {
  const extension = filename.split(".").pop()?.toLowerCase();
  if (extension === "pdf") {
    return "pdf";
  }

  if (extension === "xls" || extension === "xlsx" || extension === "csv") {
    return "sheet";
  }

  return "doc";
}

type CompanyFieldConfig = {
  label: string;
  name: Extract<
    keyof AddDealFormState,
    "buyerOrPlatformCompany" | "carveOutBusiness" | "parentOrSellerCompany" | "targetCompany"
  >;
  placeholder: string;
};

function getCompanyFieldsForDealType(dealType: string): CompanyFieldConfig[] {
  switch (dealType) {
    case "Buy-side":
      return [
        {
          label: "Buyer / platform company",
          name: "buyerOrPlatformCompany",
          placeholder: "Platform Co",
        },
        {
          label: "Target company",
          name: "targetCompany",
          placeholder: "Target Co",
        },
      ];
    case "Carve-out":
      return [
        {
          label: "Parent / seller company",
          name: "parentOrSellerCompany",
          placeholder: "Parent Co",
        },
        {
          label: "Carve-out business",
          name: "carveOutBusiness",
          placeholder: "Business Unit",
        },
      ];
    case "Add-on":
      return [
        {
          label: "Platform company",
          name: "buyerOrPlatformCompany",
          placeholder: "Platform Co",
        },
        {
          label: "Add-on target",
          name: "targetCompany",
          placeholder: "Target Co",
        },
      ];
    case "Sell-side":
      return [
        {
          label: "Target company",
          name: "targetCompany",
          placeholder: "Target Co",
        },
      ];
    case "Recapitalization":
    case "Growth equity":
      return [
        {
          label: "Target company",
          name: "targetCompany",
          placeholder: "Target Co",
        },
      ];
    default:
      return [];
  }
}
