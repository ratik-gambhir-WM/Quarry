import {
  MAX_PPTX_TEMPLATE_IMPORT_BYTES,
  type PptxTemplateImportMode,
  type PptxTemplateImportResult,
} from "../contracts/quarryApi";

export function classifiedTemplateImportPath(mode: PptxTemplateImportMode) {
  const operation = mode === "single" ? "import" : "batch-import";
  return `/api/v1/templates/slide-templates/${operation}/classified`;
}

export function validateTemplateImportFile(file: File) {
  if (
    !file.name.trim() || file.name.length > 255 ||
    /[/\\\x00-\x1f\x7f]/.test(file.name) ||
    !file.name.toLowerCase().endsWith(".pptx")
  ) {
    throw new Error("Choose a PowerPoint file with a valid .pptx filename.");
  }
  if (file.size === 0 || file.size > MAX_PPTX_TEMPLATE_IMPORT_BYTES) {
    throw new Error("Choose a non-empty PowerPoint file no larger than 25 MB.");
  }
}

/** The save headers confirm persistence without reading slide contents into the client. */
export function classifiedTemplateImportResult(
  status: number,
  headers: Headers,
  importMode: PptxTemplateImportMode,
): PptxTemplateImportResult {
  const uncertain = () => new Error(
    "Template import could not be confirmed. It may have completed; refresh templates before trying again.",
  );
  const count = (name: string, max: number) => {
    const value = headers.get(name);
    if (value === null || !/^\d+$/.test(value)) throw uncertain();
    const parsed = Number(value);
    if (!Number.isSafeInteger(parsed) || parsed > max) throw uncertain();
    return parsed;
  };
  if (
    status !== 201 ||
    headers.get("content-type")?.split(";")[0].trim() !== "application/json"
  ) throw uncertain();
  const warningCount = count("x-powerpoint-warning-count", 10_000);
  if (importMode === "single") {
    if (
      !/^[a-zA-Z0-9_-]{1,128}$/.test(headers.get("x-template-id") ?? "") ||
      headers.get("x-template-preview-status") !== "ready"
    ) throw uncertain();
    return { importMode, importedCount: 1, warningCount };
  }
  const importedCount = count("x-imported-template-count", 100);
  if (importedCount === 0) throw uncertain();
  return { importMode, importedCount, warningCount };
}
