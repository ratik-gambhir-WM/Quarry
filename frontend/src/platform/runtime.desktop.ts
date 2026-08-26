import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { createTauriQuarryApi, type TauriMultipartRequest } from "../api/tauriQuarryApi";
import type { QuarryRuntime, SaveFileInput } from "../contracts/quarryApi";
import type {
  LocalDealDataRoom,
  LocalDealFileContents,
  ReadDealSourceFilesInput,
} from "../data/dealExtraction";
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

async function selectDealDataRoom() {
  return invokeWithActivity<LocalDealDataRoom | null>("select_deal_data_room");
}

async function readDealSourceFiles(input: ReadDealSourceFilesInput) {
  return invokeWithActivity<LocalDealFileContents[]>("read_deal_source_files", { input });
}

async function invokeWithActivity<TResult>(command: string, args?: Record<string, unknown>) {
  const activityId = beginIpcRequest(command, args);
  const startedAt = performance.now();
  try {
    const result = await invoke<TResult>(command, args);
    finishIpcRequest(activityId, {
      durationMs: performance.now() - startedAt,
      status: "success",
    });
    return result;
  } catch (error) {
    const message = ipcErrorMessage(error);
    finishIpcRequest(activityId, {
      durationMs: performance.now() - startedAt,
      message,
      status: "error",
    });
    throw new Error(message);
  }
}

const tauriQuarryApi = createTauriQuarryApi({
  get: <TResult>(path: string) => invokeWithActivity<TResult>("quarry_api_get", { path }),
  getPdf: (path: string) => invokeWithActivity<ArrayBuffer>("quarry_api_get_pdf", { path }),
  post: <TResult>(path: string, body: unknown) =>
    invokeWithActivity<TResult>("quarry_api_post", { body, path }),
  postMultipart: <TResult>(request: TauriMultipartRequest) =>
    invokeWithActivity<TResult>("quarry_api_post_multipart", { request }),
  async subscribeJob(jobId, onEvent, onError) {
    const subscriptionId = crypto.randomUUID();
    const unlisten = await listen<{
      data: string;
      eventName: string;
      subscriptionId: string;
    }>("quarry-document-job-event", ({ payload }) => {
      if (payload.subscriptionId === subscriptionId) onEvent(payload.eventName, payload.data);
    });
    void invokeWithActivity<void>("subscribe_document_job", { jobId, subscriptionId }).catch(
      onError,
    );
    return unlisten;
  },
});

function ipcErrorMessage(error: unknown) {
  if (typeof error === "object" && error && "message" in error) return String(error.message);
  return error instanceof Error ? error.message : String(error);
}

export const runtime: QuarryRuntime = {
  api: tauriQuarryApi,
  platform: { readDealSourceFiles, saveFile, selectDealDataRoom },
  target: "desktop",
};
