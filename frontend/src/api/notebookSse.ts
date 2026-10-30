import { sessionStore } from "../auth/sessionStore";
import { authFetch } from "./authFetch";
import { notebookEventReceiver, notebookJobPath } from "./notebookApi";
import type { NotebookSaveHandlers } from "../contracts/notebooks";

/** Bounded SSE framing; streaming TextDecoder preserves UTF-8 across network chunks. */
export class NotebookSseParser {
  private buffer = "";
  push(text: string, receive: (data: string) => void) {
    this.buffer += text;
    if (new TextEncoder().encode(this.buffer).byteLength > 65536)
      throw new Error("Notebook event exceeds size limit.");
    this.buffer = this.buffer.replace(/\r\n/g, "\n");
    for (
      let end = this.buffer.indexOf("\n\n");
      end >= 0;
      end = this.buffer.indexOf("\n\n")
    ) {
      const frame = this.buffer.slice(0, end);
      this.buffer = this.buffer.slice(end + 2);
      const data = frame
        .split("\n")
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice(5).replace(/^ /, ""))
        .join("\n");
      if (data) receive(data);
    }
  }
}
export function subscribeNotebookSse(
  baseUrl: string,
  notebookId: string,
  jobId: string,
  handlers: NotebookSaveHandlers,
) {
  const controller = new AbortController();
  const untrack = sessionStore.track(controller);
  const receive = notebookEventReceiver(notebookId, jobId, handlers);
  void (async () => {
    const response = await authFetch(
      `${baseUrl}${notebookJobPath(notebookId, jobId)}/events`,
      { headers: { Accept: "text/event-stream" }, signal: controller.signal },
    );
    if (
      !response.ok ||
      !response.headers.get("content-type")?.startsWith("text/event-stream") ||
      !response.body
    )
      throw new Error(`Notebook save stream failed (${response.status}).`);
    const reader = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const parser = new NotebookSseParser();
    let terminal = false;
    try {
      while (!controller.signal.aborted) {
        const chunk = await reader.read();
        if (chunk.done) {
          if (!terminal)
            throw new Error(
              "Notebook save stream disconnected; check job status.",
            );
          break;
        }
        parser.push(decoder.decode(chunk.value, { stream: true }), (data) => {
          if (!terminal && !controller.signal.aborted) terminal = receive(data);
        });
        if (terminal) break;
      }
    } finally {
      await reader.cancel();
      reader.releaseLock();
    }
  })()
    .catch((error: unknown) => {
      if (!controller.signal.aborted)
        handlers.onConnectionError?.(
          error instanceof Error
            ? error.message
            : "Notebook save stream failed.",
        );
    })
    .finally(untrack);
  return () => controller.abort();
}
