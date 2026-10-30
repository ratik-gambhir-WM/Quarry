import { credentials, sessionStore } from "../auth/sessionStore";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  createTauriQuarryApi,
  type TauriMultipartRequest,
  type TauriQueryPayload,
  type TauriQueryRequest,
} from "../api/tauriQuarryApi";
import type {
  PowerPointExport,
  QuarryRuntime,
  SaveFileInput,
  SavePowerPointInput,
} from "../contracts/quarryApi";
import type {
  LocalDealDataRoom,
  LocalDealFileContents,
  ReadDealSourceFilesInput,
} from "../data/dealExtraction";
import { beginIpcRequest, finishIpcRequest } from "../lib/activityLog";

async function invokeApi<TResult>(
  command: string,
  args?: Record<string, unknown>,
) {
  if (
    !command.startsWith("quarry_api_") &&
    command !== "send_query_stream" &&
    command !== "subscribe_document_job" &&
    command !== "subscribe_notebook_job"
  )
    return invoke<TResult>(command, args);
  const snapshot = credentials();
  const path = typeof args?.path === "string" ? args.path : "";
  const publicRequest = /\/auth\/(login|register)$/.test(path);
  try {
    const result = await invoke<TResult>(command, {
      ...args,
      auth: publicRequest ? null : snapshot.auth,
    });
    if (sessionStore.getSnapshot().generation !== snapshot.generation)
      throw new DOMException("Session changed", "AbortError");
    return result;
  } catch (error) {
    if (
      typeof error === "object" &&
      error &&
      "status" in error &&
      error.status === 401 &&
      !publicRequest
    )
      sessionStore.clearIfCurrent(
        snapshot.generation,
        "Your session expired. Sign in again.",
      );
    throw error;
  }
}

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

async function savePowerPoint(input: SavePowerPointInput) {
  const activityId = beginIpcRequest("save_powerpoint_file", {
    suggestedName: input.suggestedName,
    title: input.title,
  });
  const startedAt = performance.now();

  try {
    const saved = await invoke<boolean>("save_powerpoint_file", { input });
    finishIpcRequest(activityId, {
      details: { saved },
      durationMs: performance.now() - startedAt,
      status: "success",
    });
    return saved;
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

async function selectDealDataRoom() {
  return invokeWithActivity<LocalDealDataRoom | null>("select_deal_data_room");
}

async function readDealSourceFiles(input: ReadDealSourceFilesInput) {
  return invokeWithActivity<LocalDealFileContents[]>("read_deal_source_files", {
    input,
  });
}

async function invokeWithActivity<TResult>(
  command: string,
  args?: Record<string, unknown>,
) {
  const activityId = beginIpcRequest(
    command,
    command.startsWith("quarry_api_") ? { operation: command } : args,
  );
  const startedAt = performance.now();
  try {
    const result = await invokeApi<TResult>(command, args);
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
    const wrapped = new Error(message);
    if (
      typeof error === "object" &&
      error &&
      "status" in error &&
      typeof error.status === "number"
    )
      Object.assign(wrapped, { status: error.status });
    throw wrapped;
  }
}

async function invokePowerPointExport(path: string, body: unknown) {
  const activityId = beginIpcRequest("quarry_api_post_powerpoint", { path });
  const startedAt = performance.now();
  try {
    const result = await invokeApi<PowerPointExport>(
      "quarry_api_post_powerpoint",
      { body, path },
    );
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
  delete: (path: string) =>
    invokeWithActivity<void>("quarry_api_delete", { path }),
  get: <TResult>(path: string) =>
    invokeWithActivity<TResult>("quarry_api_get", { path }),
  getPdf: (path: string) =>
    invokeWithActivity<ArrayBuffer>("quarry_api_get_pdf", { path }),
  post: <TResult>(path: string, body: unknown) =>
    invokeWithActivity<TResult>("quarry_api_post", { body, path }),
  patch: <TResult>(path: string, body: unknown) =>
    invokeWithActivity<TResult>("quarry_api_patch", { body, path }),
  async subscribeNotebookJob(notebookId, jobId, onData, onError) {
    const generation = sessionStore.getSnapshot().generation;
    const subscriptionId = crypto.randomUUID();
    const unlisten = await listen<{ data: string; subscriptionId: string }>(
      "quarry-notebook-job-event",
      ({ payload }) => {
        if (
          payload.subscriptionId === subscriptionId &&
          sessionStore.getSnapshot().generation === generation
        )
          onData(payload.data);
      },
    );
    if (sessionStore.getSnapshot().generation !== generation) {
      unlisten();
      throw new DOMException("Session changed", "AbortError");
    }
    const controller = new AbortController();
    const untrack = sessionStore.track(controller);
    let stopped = false;
    const stop = () => {
      if (stopped) return;
      stopped = true;
      untrack();
      unlisten();
      void invoke<void>("cancel_query_stream", { subscriptionId }).catch(
        () => undefined,
      );
    };
    controller.signal.addEventListener("abort", stop, { once: true });
    void invokeApi<void>("subscribe_notebook_job", {
      notebookId,
      jobId,
      subscriptionId,
    })
      .catch(() => {
        if (!stopped) onError();
      })
      .finally(stop);
    return stop;
  },
  postPowerPoint: (path: string, body: unknown) =>
    invokePowerPointExport(path, body),
  postMultipart: <TResult>(request: TauriMultipartRequest) => {
    const { sessionGeneration, ...payload } = request;
    if (
      sessionGeneration !== undefined &&
      sessionGeneration !== sessionStore.getSnapshot().generation
    )
      return Promise.reject(new DOMException("Session changed", "AbortError"));
    return invokeWithActivity<TResult>("quarry_api_post_multipart", {
      request: payload,
    });
  },
  async startQuery(
    request: TauriQueryRequest,
    onPayload: (payload: TauriQueryPayload) => void,
  ) {
    if (
      request.sessionGeneration !== undefined &&
      request.sessionGeneration !== sessionStore.getSnapshot().generation
    )
      throw new DOMException("Session changed", "AbortError");
    const { sessionGeneration: _generation, ...payloadRequest } = request;
    const subscriptionId = crypto.randomUUID();
    const activityId = beginIpcRequest("send_query_stream", {
      contextMessageCount: request.context?.length ?? 0,
      fileBytes: request.files.reduce(
        (total, file) => total + Math.floor(file.dataBase64.length * 0.75),
        0,
      ),
      fileCount: request.files.length,
      model: request.model,
    });
    const startedAt = performance.now();
    let stopped = false;
    const snapshot = credentials();
    const unlisten = await listen<TauriQueryPayload>(
      "quarry-query-event",
      ({ payload }) => {
        if (!stopped && payload.subscriptionId === subscriptionId) {
          if (payload.kind === "connectionError" && payload.status === 401)
            sessionStore.clearIfCurrent(
              snapshot.generation,
              "Your session expired. Sign in again.",
            );
          onPayload(payload);
        }
      },
    );
    try {
      if (snapshot.generation !== sessionStore.getSnapshot().generation)
        throw new DOMException("Session changed", "AbortError");
      await invokeApi<void>("send_query_stream", {
        request: payloadRequest,
        subscriptionId,
      });
      finishIpcRequest(activityId, {
        durationMs: performance.now() - startedAt,
        status: "success",
      });
    } catch (error) {
      unlisten();
      const message = ipcErrorMessage(error);
      finishIpcRequest(activityId, {
        durationMs: performance.now() - startedAt,
        message,
        status: "error",
      });
      throw new Error(message);
    }
    const controller = new AbortController();
    const untrack = sessionStore.track(controller);
    const stop = () => {
      if (stopped) return;
      stopped = true;
      untrack();
      unlisten();
      void invoke<void>("cancel_query_stream", { subscriptionId }).catch(
        () => undefined,
      );
    };
    controller.signal.addEventListener("abort", stop, { once: true });
    return stop;
  },
  async subscribeJob(jobId, onEvent, onError) {
    const subscriptionId = crypto.randomUUID();
    const unlisten = await listen<{
      data: string;
      eventName: string;
      subscriptionId: string;
    }>("quarry-document-job-event", ({ payload }) => {
      if (payload.subscriptionId === subscriptionId)
        onEvent(payload.eventName, payload.data);
    });
    void invokeWithActivity<void>("subscribe_document_job", {
      jobId,
      subscriptionId,
    }).catch(onError);
    const controller = new AbortController();
    const untrack = sessionStore.track(controller);
    const stop = () => {
      untrack();
      unlisten();
      void invoke<void>("cancel_query_stream", { subscriptionId }).catch(
        () => undefined,
      );
    };
    controller.signal.addEventListener("abort", stop, { once: true });
    return stop;
  },
});

function ipcErrorMessage(error: unknown) {
  if (typeof error === "object" && error && "message" in error)
    return String(error.message);
  return error instanceof Error ? error.message : String(error);
}

export const runtime: QuarryRuntime = {
  api: tauriQuarryApi,
  platform: {
    readDealSourceFiles,
    saveFile,
    savePowerPoint,
    selectDealDataRoom,
  },
  target: "desktop",
};
