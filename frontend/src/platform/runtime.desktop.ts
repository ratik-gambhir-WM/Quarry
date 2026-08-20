import { invoke } from "@tauri-apps/api/core";
import { httpQuarryApi } from "../api/httpQuarryApi";
import type { QuarryRuntime, SaveFileInput } from "../contracts/quarryApi";
import { beginIpcRequest, finishIpcRequest } from "../lib/activityLog";

async function saveFile(input: SaveFileInput) {
  const activityId = beginIpcRequest("save_text_file", {
    extensions: input.extensions,
    mimeType: input.mimeType,
    suggestedName: input.suggestedName,
    title: input.title,
  });
  const startedAt = performance.now();

  try {
    const saved = await invoke<boolean>("save_text_file", { input });
    finishIpcRequest(activityId, {
      details: { saved },
      durationMs: performance.now() - startedAt,
      status: "success",
    });
    return saved;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    finishIpcRequest(activityId, {
      durationMs: performance.now() - startedAt,
      message,
      status: "error",
    });
    throw new Error(message);
  }
}

export const runtime: QuarryRuntime = {
  api: httpQuarryApi,
  platform: { saveFile },
  target: "desktop",
};
