import { beginApiRequest, finishApiRequest } from "../../lib/activityLog";
import { sessionStore } from "../../auth/sessionStore";
import { authFetch } from "../authFetch";
import { parseUploadProgress } from "../uploadProgress";
import { apiUrl, BackendApiError } from "./transport";
import type { ProcessFileJobResponse, UploadProgressEvent } from "../../contracts/quarryApi";

export async function uploadDocument(
  dealId: string,
  file: File,
  onProgress?: (event: UploadProgressEvent) => void,
): Promise<ProcessFileJobResponse> {
  const generation = sessionStore.getSnapshot().generation;
  const bytes = await file.arrayBuffer();
  if (generation !== sessionStore.getSnapshot().generation)
    throw new DOMException("Session changed", "AbortError");
  const byteFile = new File([bytes], file.name, {
    lastModified: file.lastModified,
    type: file.type,
  });
  const form = new FormData();
  form.append("files", byteFile, file.name);
  const url = apiUrl(`/api/v1/deals/${encodeURIComponent(dealId)}/documents/upload`);
  const requestId = beginApiRequest({ method: "POST", url });
  const startedAt = performance.now();
  let httpStatus: number | undefined;
  try {
    const response = await authFetch(url, {
      method: "POST", body: form, headers: { Accept: "text/event-stream" },
    });
    httpStatus = response.status;
    if (!response.ok) {
      await response.body?.cancel();
      throw new BackendApiError("Upload admission failed. Retry the file.", response.status);
    }
    const result = await readAdmission(response, onProgress);
    finishApiRequest(requestId, {
      durationMs: performance.now() - startedAt, httpStatus, status: "success",
    });
    return result;
  } catch (error) {
    finishApiRequest(requestId, {
      durationMs: performance.now() - startedAt, httpStatus, status: "error",
      message: error instanceof Error ? error.message : "Upload failed",
    });
    throw error;
  }
}

async function readAdmission(
  response: Response,
  onProgress?: (event: UploadProgressEvent) => void,
): Promise<ProcessFileJobResponse> {
  if (!response.body || !response.headers.get("content-type")?.startsWith("text/event-stream")) {
    await response.body?.cancel();
    throw new Error("Invalid upload progress stream.");
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let buffer = "";
  try {
    while (true) {
      const chunk = await reader.read();
      if (chunk.done) throw new Error("Upload connection interrupted. Retry the same file.");
      buffer += decoder.decode(chunk.value, { stream: true });
      if (buffer.length > 65536) throw new Error("Upload progress event exceeds size limit.");
      let boundary: RegExpExecArray | null;
      while ((boundary = /\r?\n\r?\n/.exec(buffer))) {
        const frame = buffer.slice(0, boundary.index);
        buffer = buffer.slice(boundary.index + boundary[0].length);
        const data = frame.split(/\r?\n/)
          .filter(line => line.startsWith("data:"))
          .map(line => line.slice(5).trimStart()).join("\n");
        if (!data) continue;
        const event = parseUploadProgress(JSON.parse(data));
        const name = frame.split(/\r?\n/).find(line => line.startsWith("event:"))?.slice(6).trim();
        if (name !== event.status) throw new Error("Invalid upload progress event.");
        onProgress?.(event);
        if (event.status === "accepted") return { jobId: event.jobId, filename: event.filename };
        if (event.status === "failed") throw new Error("Document preparation failed. Retry the same file.");
      }
    }
  } finally {
    await reader.cancel().catch(() => undefined);
  }
}
