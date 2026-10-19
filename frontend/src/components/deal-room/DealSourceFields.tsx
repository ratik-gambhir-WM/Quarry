import { type ComponentProps, useRef } from "react";
import type { LocalDealSourceFile } from "../../data/dealExtraction";
import { formatFileSize } from "../../lib/formatters";
import { Field, FieldError, FieldLabel } from "../ui/field";
import { Input } from "../ui/input";
import { Icon } from "../ui/Icon";

export type DealSourceLinks = {
  factSheetLink: string; localPath: string; rlLink: string; sharepointLink: string; sowLink: string;
};
export type SourceFileSelection = File | LocalDealSourceFile;
export type SelectedSourceFiles = {
  projectTimelineFile: SourceFileSelection | null;
  sowFile: SourceFileSelection | null;
};

type DealTextFieldProps = Omit<ComponentProps<typeof Input>, "id" | "onChange"> & {
  error?: string;
  id: string;
  label: string;
  onValueChange: (value: string) => void;
  optional?: boolean;
};

export function DealTextField({
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

export function SourceFilesStep({ availableFiles, form, onChange, onUpdateField, selected }: { availableFiles: LocalDealSourceFile[] | null; form: DealSourceLinks; onChange: (field: keyof SelectedSourceFiles, file: SourceFileSelection | null) => void; onUpdateField: (field: keyof DealSourceLinks, value: string) => void; selected: SelectedSourceFiles }) {
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
      {availableFiles === null ? <input aria-label={label} className="hidden" onChange={(event) => { onChange(event.target.files?.[0] ?? null); event.target.value = ""; }} ref={inputRef} type="file" /> : null}
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

export function isLocalDealSourceFile(file: SourceFileSelection | null): file is LocalDealSourceFile {
  return file !== null && "relativePath" in file && "mimeType" in file;
}

function fileIcon(filename: string): "doc" | "pdf" | "sheet" {
  const extension = filename.split(".").pop()?.toLowerCase();
  if (extension === "pdf") return "pdf";
  if (extension === "xls" || extension === "xlsx" || extension === "csv") return "sheet";
  return "doc";
}

