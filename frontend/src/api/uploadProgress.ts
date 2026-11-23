import type { UploadProgressEvent } from "../contracts/quarryApi";

export function parseUploadProgress(value: unknown): UploadProgressEvent {
  if (typeof value !== "object" || value === null ||
      !("jobId" in value) || typeof value.jobId !== "string" || !value.jobId ||
      !("filename" in value) || typeof value.filename !== "string" ||
      !("status" in value) || typeof value.status !== "string" ||
      !["parsing", "classifying", "accepted", "failed"].includes(value.status)) {
    throw new Error("Invalid upload progress event.");
  }
  return value as UploadProgressEvent;
}
