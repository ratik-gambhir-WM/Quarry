import { authFetch } from "../authFetch";
import type { AssistantThreadListOptions } from "../../contracts/quarryApi";
import type {
  AssistantThread,
  AssistantThreadDetail,
  AssistantThreadPage,
  QueryModelInput,
  QuarryApi,
  RunAssistantThreadInput,
  SendQueryEventHandlers,
} from "../../contracts/quarryApi";
import { QuerySseParser } from "../querySse";
import {
  beginApiRequest,
  finishApiRequest,
  logSseEvent,
} from "../../lib/activityLog";

import { get, post, requestJson, apiUrl } from "./transport";

function queryModel(input: QueryModelInput, handlers: SendQueryEventHandlers) {
  const form = new FormData();
  form.append("prompt", input.prompt);
  form.append("context", JSON.stringify(input.context));
  if (input.model !== undefined) form.append("model", input.model);
  if (input.systemInstructions !== undefined) {
    form.append("systemInstructions", input.systemInstructions);
  }
  for (const file of input.files) form.append("files", file, file.name);
  return streamQuery(
    "/api/v1/query_model",
    form,
    {
      contextMessageCount: input.context.length,
      fileCount: input.files.length,
      fileBytes: input.files.reduce((total, file) => total + file.size, 0),
      model: input.model,
    },
    handlers,
  );
}

function runAssistantThread(
  input: RunAssistantThreadInput,
  handlers: SendQueryEventHandlers,
) {
  const form = new FormData();
  form.append("prompt", input.prompt);
  form.append("userMessageId", input.userMessageId);
  form.append("assistantMessageId", input.assistantMessageId);
  form.append("requestId", input.requestId);
  if (input.parentMessageId !== undefined)
    form.append("parentMessageId", input.parentMessageId);
  if (input.model !== undefined) form.append("model", input.model);
  if (input.systemInstructions !== undefined) {
    form.append("systemInstructions", input.systemInstructions);
  }
  for (const file of input.files) form.append("files", file, file.name);
  return streamQuery(
    `/api/v1/assistant/threads/${encodeURIComponent(input.threadId)}/runs`,
    form,
    {
      fileCount: input.files.length,
      fileBytes: input.files.reduce((total, file) => total + file.size, 0),
      model: input.model,
      threadId: input.threadId,
    },
    handlers,
  );
}

function streamQuery(
  path: string,
  form: FormData,
  requestDetails: Record<string, unknown>,
  { onConnectionError, onEvent }: SendQueryEventHandlers,
) {
  const url = apiUrl(path);
  const controller = new AbortController();
  const startedAt = performance.now();
  const requestId = beginApiRequest({
    method: "POST",
    request: requestDetails,
    url,
  });
  console.info("[chat] http.stream.started", {
    path,
    requestId,
    requestDetails,
  });
  let active = true;
  let settled = false;
  let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;

  const stopStream = () => {
    controller.abort();
    void reader?.cancel().catch(() => undefined);
  };

  const failConnection = (message: string, httpStatus?: number) => {
    if (!active || settled) return;
    settled = true;
    active = false;
    stopStream();
    console.error("[chat] http.stream.failed", {
      httpStatus,
      message,
      path,
      requestId,
    });
    finishApiRequest(requestId, {
      durationMs: performance.now() - startedAt,
      httpStatus,
      message,
      status: "error",
    });
    onConnectionError?.(message);
  };

  void (async () => {
    try {
      const response = await authFetch(url, {
        body: form,
        headers: { Accept: "text/event-stream" },
        method: "POST",
        signal: controller.signal,
      });
      if (!response.ok) {
        failConnection(
          `The query request failed with status ${response.status}.`,
          response.status,
        );
        return;
      }
      const contentType = response.headers
        .get("content-type")
        ?.split(";", 1)[0]
        .trim();
      console.info("[chat] http.stream.connected", {
        contentType,
        path,
        requestId,
        status: response.status,
      });
      if (contentType !== "text/event-stream" || !response.body) {
        failConnection(
          "The query backend returned an invalid streaming response.",
          response.status,
        );
        return;
      }
      const parser = new QuerySseParser((event) => {
        if (!active || settled) return;
        logSseEvent({
          data:
            event.type === "delta"
              ? { characterCount: [...event.delta].length }
              : { type: event.type },
          eventName: event.type,
          status: event.type === "failed" ? "error" : "success",
          title: `Query ${event.type} event`,
          url,
        });
        console.info("[chat] http.sse.event", {
          event: event.type,
          path,
          requestId,
          valueCharacters:
            event.type === "delta"
              ? [...event.delta].length
              : event.type === "completed"
                ? [...event.response].length
                : undefined,
        });
        onEvent(event);
        if (event.type === "completed" || event.type === "failed") {
          settled = true;
          active = false;
          stopStream();
          finishApiRequest(requestId, {
            durationMs: performance.now() - startedAt,
            httpStatus: response.status,
            status: event.type === "completed" ? "success" : "error",
          });
        }
      });
      reader = response.body.getReader();
      while (active) {
        const { done, value } = await reader.read();
        if (done) break;
        parser.push(value);
      }
      if (active) parser.finish();
    } catch (error) {
      console.error("[chat] http.stream.exception", { error, path, requestId });
      if (
        !active &&
        error instanceof DOMException &&
        error.name === "AbortError"
      )
        return;
      failConnection(
        error instanceof Error ? error.message : "The query connection failed.",
      );
    }
  })();

  return () => {
    if (!active) return;
    active = false;
    stopStream();
    if (!settled) {
      settled = true;
      finishApiRequest(requestId, {
        durationMs: performance.now() - startedAt,
        message: "Query cancelled",
        status: "success",
      });
    }
  };
}

function assistantThreadPath(threadId: string, suffix = "") {
  return `/api/v1/assistant/threads/${encodeURIComponent(threadId)}${suffix}`;
}

function listAssistantThreads(options: AssistantThreadListOptions = {}) {
  const parameters = new URLSearchParams();
  if (options.after) parameters.set("before", options.after);
  if (options.archived) parameters.set("archived", "true");
  if (options.scope) parameters.set("scope", options.scope);
  if (options.dealId) parameters.set("dealId", options.dealId);
  return get<AssistantThreadPage>(
    `/api/v1/assistant/threads?${parameters.toString()}`,
  );
}

function createAssistantThread(threadId?: string, dealId?: string | null) {
  return post<AssistantThread, { threadId?: string; dealId?: string | null }>(
    "/api/v1/assistant/threads",
    {
      ...(threadId ? { threadId } : {}),
      ...(dealId !== undefined ? { dealId } : {}),
    },
  );
}

function getAssistantThread(threadId: string) {
  return get<AssistantThreadDetail>(`${assistantThreadPath(threadId)}`);
}

function renameAssistantThread(threadId: string, title: string) {
  return post<void, { title: string }>(
    assistantThreadPath(threadId, "/rename"),
    { title },
  );
}

function setAssistantThreadArchived(threadId: string, archived: boolean) {
  return post<void, Record<string, never>>(
    assistantThreadPath(threadId, archived ? "/archive" : "/unarchive"),
    {},
  );
}

function deleteAssistantThread(threadId: string) {
  return requestJson<void>(`${assistantThreadPath(threadId)}`, {
    method: "DELETE",
  });
}

export const httpAssistantApi: Pick<
  QuarryApi,
  | "queryModel"
  | "runAssistantThread"
  | "listAssistantThreads"
  | "createAssistantThread"
  | "getAssistantThread"
  | "renameAssistantThread"
  | "deleteAssistantThread"
  | "archiveAssistantThread"
  | "unarchiveAssistantThread"
> = {
  queryModel,
  runAssistantThread,
  listAssistantThreads,
  createAssistantThread,
  getAssistantThread,
  renameAssistantThread,
  deleteAssistantThread,
  archiveAssistantThread: (threadId) =>
    setAssistantThreadArchived(threadId, true),
  unarchiveAssistantThread: (threadId) =>
    setAssistantThreadArchived(threadId, false),
};
