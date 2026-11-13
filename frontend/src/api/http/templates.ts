import { parseTemplateResponse } from "../templateResponse";
import {
  classifiedTemplateImportPath,
  classifiedTemplateImportResult,
  validateTemplateImportFile,
} from "../classifiedTemplateImport";
import { authFetch } from "../authFetch";
import type {
  PowerPointExport,
  QuarryApi,
  PptxTemplateImportMode,
  PptxTemplateImportResult,
  TemplatePreviewPage,
  TemplateRetrieval,
  TemplateProcessingRetry,
} from "../../contracts/quarryApi";
import { POWERPOINT_CONTENT_TYPE } from "../../contracts/quarryApi";
import type { DiligenceCanvasDocument } from "../../contracts/diligenceCanvas";
import { beginApiRequest, finishApiRequest } from "../../lib/activityLog";

import { get, post, requestJson, apiUrl, BackendApiError } from "./transport";

const MAX_POWERPOINT_EXPORT_BYTES = 64 * 1024 * 1024;

async function exportPowerPoint(
  document: DiligenceCanvasDocument,
): Promise<PowerPointExport> {
  const path = "/api/v1/templates/export";
  const url = apiUrl(path);
  const requestId = beginApiRequest({
    method: "POST",
    request: {
      slideCount: document.presentation.slides.length,
    },
    url,
  });
  const startedAt = performance.now();

  try {
    const response = await authFetch(url, {
      body: JSON.stringify(document),
      headers: { "Content-Type": "application/json" },
      method: "POST",
    });
    if (!response.ok) {
      const bodyText = await response.text();
      let body: { error?: string; message?: string } | undefined;
      try {
        body = JSON.parse(bodyText) as { error?: string; message?: string };
      } catch {
        body = undefined;
      }
      const message =
        body?.error ||
        body?.message ||
        bodyText ||
        response.statusText ||
        `Request failed with status ${response.status}`;
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        httpStatus: response.status,
        message,
        status: "error",
      });
      throw new BackendApiError(message, response.status);
    }

    const mimeType = response.headers
      .get("content-type")
      ?.split(";", 1)[0]
      .trim();
    if (mimeType !== POWERPOINT_CONTENT_TYPE) {
      throw new Error(
        "The export backend returned an invalid PowerPoint content type.",
      );
    }
    const fileName = parsePowerPointFileName(
      response.headers.get("content-disposition"),
    );
    const warningCount = parsePowerPointWarningCount(
      response.headers.get("x-powerpoint-warning-count"),
    );
    const declaredLength = response.headers.get("content-length");
    if (declaredLength) {
      const length = Number(declaredLength);
      if (!/^\d+$/.test(declaredLength) || !Number.isSafeInteger(length)) {
        throw new Error(
          "The export backend returned an invalid PowerPoint size.",
        );
      }
      if (length > MAX_POWERPOINT_EXPORT_BYTES) {
        throw new Error(
          "The export backend returned an oversized PowerPoint file.",
        );
      }
    }
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (
      bytes.byteLength === 0 ||
      bytes.byteLength > MAX_POWERPOINT_EXPORT_BYTES ||
      bytes[0] !== 0x50 ||
      bytes[1] !== 0x4b ||
      bytes[2] !== 0x03 ||
      bytes[3] !== 0x04
    ) {
      throw new Error(
        "The export backend returned invalid or oversized PowerPoint bytes.",
      );
    }
    const dataBase64 = bytesToBase64(bytes);
    finishApiRequest(requestId, {
      details: { byteLength: bytes.byteLength, warningCount },
      durationMs: performance.now() - startedAt,
      httpStatus: response.status,
      status: "success",
    });
    return {
      dataBase64,
      fileName,
      mimeType: POWERPOINT_CONTENT_TYPE,
      warningCount,
    };
  } catch (error) {
    if (!(error instanceof BackendApiError)) {
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        message:
          error instanceof Error ? error.message : "Network request failed",
        status: "error",
      });
    }
    throw error;
  }
}

function parsePowerPointFileName(value: string | null) {
  const match = value?.match(/^attachment; filename="([^"\\/\r\n]+\.pptx)"$/i);
  if (!match || match[1].length > 255 || match[1].trim() !== match[1]) {
    throw new Error(
      "The export backend returned an invalid PowerPoint filename.",
    );
  }
  return match[1];
}

function parsePowerPointWarningCount(value: string | null) {
  if (!value || !/^\d+$/.test(value)) {
    throw new Error("The export backend returned an invalid warning count.");
  }
  const count = Number(value);
  if (!Number.isSafeInteger(count) || count > 10_000) {
    throw new Error("The export backend returned an invalid warning count.");
  }
  return count;
}

function bytesToBase64(bytes: Uint8Array) {
  let binary = "";
  const chunkSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += chunkSize) {
    binary += String.fromCharCode(
      ...bytes.subarray(offset, offset + chunkSize),
    );
  }
  return btoa(binary);
}

function listTemplatePreviews(page: number) {
  return get<TemplatePreviewPage>(
    `/api/v1/templates/previews?page=${encodeURIComponent(String(page))}`,
  );
}

function getTemplateRetrieval(templateId: string, includeEmbeddings = false) {
  return requestJson<TemplateRetrieval>(
    `/api/v1/templates/slide-templates/catalog/${encodeURIComponent(templateId)}/retrieval?metadata=true&embeddings=${includeEmbeddings}`,
    undefined,
    undefined,
    false,
  );
}

function retryTemplateProcessing(templateId: string) {
  return post<TemplateProcessingRetry, Record<string, never>>(
    `/api/v1/templates/slide-templates/catalog/${encodeURIComponent(templateId)}/retrieval/retry`,
    {},
  );
}

async function getTemplate(templateId: string) {
  const value = await requestJson<unknown>(
    `/api/v1/templates/${encodeURIComponent(templateId)}`,
    undefined,
    undefined,
    false,
  );
  return parseTemplateResponse(value);
}

async function importPptxTemplate(
  file: File,
  importMode: PptxTemplateImportMode,
): Promise<PptxTemplateImportResult> {
  validateTemplateImportFile(file);
  let response: Response;
  try {
    response = await authFetch(apiUrl(classifiedTemplateImportPath(importMode)), {
      method: "POST",
      headers: { "Content-Type": POWERPOINT_CONTENT_TYPE },
      body: file,
    });
  } catch {
    throw new Error(
      "Template import could not be confirmed. It may have completed; refresh templates before trying again.",
    );
  }
  try {
    if (!response.ok) {
      const body: unknown = await response.json().catch(() => null);
      const message =
        body && typeof body === "object" && "error" in body && typeof body.error === "string"
          ? body.error
          : "The template could not be imported. Try again.";
      throw new BackendApiError(message, response.status);
    }
    return classifiedTemplateImportResult(
      response.status, response.headers, importMode,
    );
  } finally {
    // The server commits before sending these headers. Release the unused document body.
    await response.body?.cancel().catch(() => undefined);
  }
}

async function deleteTemplate(templateId: string, expectedRevision: number) {
  await requestJson<void>(
    `/api/v1/templates/${encodeURIComponent(templateId)}?expectedRevision=${expectedRevision}`,
    { method: "DELETE" },
  );
}

export const httpTemplatesApi: Pick<
  QuarryApi,
  | "exportPowerPoint"
  | "listTemplatePreviews"
  | "getTemplateRetrieval"
  | "retryTemplateProcessing"
  | "getTemplate"
  | "importPptxTemplate"
  | "deleteTemplate"
> = {
  exportPowerPoint,
  listTemplatePreviews,
  getTemplateRetrieval,
  retryTemplateProcessing,
  getTemplate,
  importPptxTemplate,
  deleteTemplate,
};
